use crate::Peer;
use std::{
    fs::{self, File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
};

pub struct Endpoint {
    root: PathBuf,
}

pub struct Listener {
    listener: UnixListener,
    _lock: File,
    socket: PathBuf,
    identity: (u64, u64),
}

impl Endpoint {
    pub fn new(root: &Path) -> io::Result<Self> {
        match fs::DirBuilder::new().mode(0o700).create(root) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = fs::symlink_metadata(root)?;
        // The directory is the trust boundary for every endpoint entry.
        if !metadata.is_dir()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o777 != 0o700
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "endpoint directory must be owned and private",
            ));
        }
        Ok(Self {
            root: root.to_owned(),
        })
    }

    pub fn socket(&self) -> PathBuf {
        self.root.join("session.sock")
    }

    pub fn bind(&self) -> io::Result<Option<Listener>> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.root.join("owner.lock"))?;
        let metadata = lock.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o777 != 0o600
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe endpoint lock",
            ));
        }
        // The retained descriptor owns this advisory lock until Listener drops.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::WouldBlock {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let socket = self.socket();
        match socket_metadata(&socket) {
            Ok(_) => fs::remove_file(&socket)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(&socket)?;
        let metadata = fs::symlink_metadata(&socket)?;
        let owner = Listener {
            listener,
            _lock: lock,
            socket,
            identity: (metadata.dev(), metadata.ino()),
        };
        fs::set_permissions(&owner.socket, fs::Permissions::from_mode(0o600))?;
        owner.listener.set_nonblocking(true)?;
        Ok(Some(owner))
    }

    pub fn connect(&self) -> io::Result<Peer> {
        socket_metadata(&self.socket())?;
        Peer::new(UnixStream::connect(self.socket())?)
    }
}

impl Listener {
    pub fn accept(&self) -> io::Result<Option<Peer>> {
        match self.listener.accept() {
            Ok((stream, _)) => Peer::new(stream).map(Some),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Ok(metadata) = socket_metadata(&self.socket)
            && (metadata.dev(), metadata.ino()) == self.identity
        {
            let _ = fs::remove_file(&self.socket);
        }
    }
}

fn socket_metadata(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe endpoint socket",
        ));
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests;
