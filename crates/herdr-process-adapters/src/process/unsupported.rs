use super::ProcessSpec;
use anyhow::{Result, bail};
use portable_pty::PtySize;
use std::path::Path;
pub struct SupervisedProcess {
    _private: (),
}
impl SupervisedProcess {
    pub fn spawn(_: &Path, _: &ProcessSpec, _: PtySize) -> Result<Self> {
        bail!("process guardian requires macOS")
    }
    pub fn pid(&self) -> u32 {
        unreachable!("unsupported platform cannot construct a process guardian")
    }
    pub fn resize(&mut self, _: PtySize) -> Result<()> {
        bail!("process guardian requires macOS")
    }
    pub fn write(&mut self, _: &[u8]) -> Result<usize> {
        bail!("process guardian requires macOS")
    }
    pub fn read_available(&mut self) -> Result<Vec<u8>> {
        bail!("process guardian requires macOS")
    }
    pub fn poll_exit(&mut self) -> Result<Option<i32>> {
        bail!("process guardian requires macOS")
    }
    pub fn terminate(&mut self) -> Result<()> {
        bail!("process guardian requires macOS")
    }
}
pub fn supervise(_: &[String]) -> Result<i32> {
    bail!("process guardian requires macOS")
}
