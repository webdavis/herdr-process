use std::{
    fs::{self, Metadata},
    io,
    os::unix::fs::MetadataExt,
    path::Path,
};

pub(crate) fn ready<T>(result: io::Result<T>) -> io::Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
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

pub(crate) fn owned(metadata: &Metadata) -> bool {
    metadata.uid() == effective_uid()
}

pub(crate) fn private_directory(path: &Path) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    Ok(metadata.is_dir() && owned(&metadata) && metadata.mode() & 0o777 == 0o700)
}

pub(crate) fn random_hex(bytes: usize) -> String {
    let mut random = vec![0u8; bytes];
    unsafe { libc::arc4random_buf(random.as_mut_ptr().cast(), random.len()) };
    random.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn effective_uid() -> u32 {
    unsafe { libc::geteuid() }
}
