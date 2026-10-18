use herdr_process_application::{Process, Processes};
use herdr_process_domain::Command;
use portable_pty::PtySize;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[path = "process/control.rs"]
mod control;
#[cfg(target_os = "macos")]
#[path = "process/directory.rs"]
mod directory;
#[cfg(target_os = "macos")]
#[path = "process/guardian.rs"]
mod guardian;
#[cfg(target_os = "macos")]
#[path = "process/launch.rs"]
mod launch;
#[cfg(target_os = "macos")]
#[path = "process/native.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "process/owner.rs"]
mod owner;
#[cfg(target_os = "macos")]
#[path = "process/startup.rs"]
mod startup;
#[cfg(target_os = "macos")]
pub use {guardian::supervise, owner::SupervisedProcess};

#[cfg(not(target_os = "macos"))]
#[path = "process/unsupported.rs"]
mod unsupported;
#[cfg(not(target_os = "macos"))]
pub use unsupported::{SupervisedProcess, supervise};

pub struct Supervisor {
    pub binary: PathBuf,
}

impl Processes for Supervisor {
    type Process = SupervisedProcess;
    fn spawn(&self, command: &Command, rows: u16, cols: u16) -> anyhow::Result<SupervisedProcess> {
        SupervisedProcess::spawn(&self.binary, command, size(rows, cols))
    }
}

impl Process for SupervisedProcess {
    fn write(&mut self, bytes: &[u8]) -> anyhow::Result<usize> {
        SupervisedProcess::write(self, bytes)
    }
    fn read_available(&mut self) -> anyhow::Result<Vec<u8>> {
        SupervisedProcess::read_available(self)
    }
    fn poll_exit(&mut self) -> anyhow::Result<Option<i32>> {
        SupervisedProcess::poll_exit(self)
    }
    fn resize(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        SupervisedProcess::resize(self, size(rows, cols))
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        SupervisedProcess::terminate(self)
    }
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}
