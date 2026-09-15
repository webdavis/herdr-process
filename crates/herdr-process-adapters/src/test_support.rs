use std::{
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// A private temporary directory, unique per run and removed when it drops.
///
/// A name keyed only on the process identifier collides with a directory
/// leaked by an earlier run, because the operating system reuses identifiers:
/// the second run then finds a stale socket, lock or log where it expects
/// none. The nanosecond stamp makes the name unique and the removal on drop
/// stops the leak that makes collisions possible at all.
pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        // Deliberately short and under /tmp: these roots hold Unix sockets,
        // whose absolute path must fit SUN_LEN, and the per-user temporary
        // directory on macOS already spends most of that budget.
        let path = PathBuf::from(format!(
            "/tmp/hp-{}-{stamp:x}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Default for TempRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
