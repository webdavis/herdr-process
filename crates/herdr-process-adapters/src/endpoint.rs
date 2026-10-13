use crate::{Peer, Readiness, unix};
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
        if !unix::private_directory(root)? {
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
        if !metadata.is_file() || !unix::owned(&metadata) || metadata.mode() & 0o777 != 0o600 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe endpoint lock",
            ));
        }
        if unix::ready(try_lock(&lock))?.is_none() {
            return Ok(None);
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
    pub fn watch(&self, readiness: &mut Readiness) {
        readiness.readable(self.listener.as_raw_fd());
    }

    pub fn accept(&self) -> io::Result<Option<Peer>> {
        unix::ready(self.listener.accept())?
            .map(|(stream, _)| Peer::new(stream))
            .transpose()
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
    if !metadata.file_type().is_socket() || !unix::owned(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe endpoint socket",
        ));
    }
    Ok(metadata)
}

fn try_lock(file: &File) -> io::Result<()> {
    match unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } {
        0 => Ok(()),
        _ => Err(io::Error::last_os_error()),
    }
}

#[cfg(test)]
mod tests;
