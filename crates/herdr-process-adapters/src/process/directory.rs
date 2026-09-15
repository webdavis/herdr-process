use anyhow::{Result, ensure};
use std::{
    ffi::{CString, OsString},
    os::unix::{
        ffi::{OsStrExt, OsStringExt},
        fs::MetadataExt,
    },
    path::{Path, PathBuf},
};

pub(super) struct Directory(pub PathBuf);
impl Directory {
    pub fn create() -> Result<Self> {
        let path = std::env::temp_dir().join("g-XXXXXX");
        let mut bytes = CString::new(path.as_os_str().as_bytes())?.into_bytes_with_nul();
        // mkdtemp atomically creates a new directory with mode 0700.
        ensure!(
            !unsafe { libc::mkdtemp(bytes.as_mut_ptr().cast()) }.is_null(),
            "private guardian directory: {}",
            std::io::Error::last_os_error()
        );
        bytes.pop();
        Ok(Self(PathBuf::from(OsString::from_vec(bytes))))
    }
    pub fn connected(socket: &Path) -> Result<Self> {
        ensure!(
            socket.file_name() == Some(std::ffi::OsStr::new("s")),
            "invalid guardian socket name"
        );
        let parent = socket
            .parent()
            .ok_or_else(|| anyhow::anyhow!("missing guardian directory"))?;
        let metadata = std::fs::symlink_metadata(parent)?;
        ensure!(
            metadata.is_dir()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o777 == 0o700,
            "guardian directory must be private and owned"
        );
        Ok(Self(parent.to_owned()))
    }
    pub fn remove(&self) -> Result<()> {
        for result in [
            std::fs::remove_file(self.0.join("s")),
            std::fs::remove_dir(&self.0),
        ] {
            if let Err(e) = result
                && e.kind() != std::io::ErrorKind::NotFound
            {
                return Err(e.into());
            }
        }
        Ok(())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            eprintln!("guardian directory cleanup incomplete: {error:#}");
        }
    }
}
