use herdr_process_application::{Process, Processes};
use herdr_process_domain::Command;
use portable_pty::PtySize;
use std::path::PathBuf;

mod control;
mod directory;
mod guardian;
mod launch;
mod native;
mod owner;
mod startup;
pub use {guardian::supervise, owner::SupervisedProcess};

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
