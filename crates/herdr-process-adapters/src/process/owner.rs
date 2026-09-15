use super::{ProcessSpec, control::Message, startup::Startup};
use anyhow::{Result, bail, ensure};
use portable_pty::PtySize;
use std::{
    io::{Read, Write},
    path::Path,
    time::{Duration, Instant},
};

pub struct SupervisedProcess {
    startup: Startup,
    pid: u32,
    state: State,
}
enum State {
    Running,
    Stopping,
    Exited(i32),
    Failed(String),
}
impl SupervisedProcess {
    pub fn spawn(supervisor_binary: &Path, spec: &ProcessSpec, size: PtySize) -> Result<Self> {
        let mut startup = Startup::create(supervisor_binary, size)?;
        let pid = startup.start(spec)?;
        Ok(Self {
            startup,
            pid,
            state: State::Running,
        })
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn resize(&mut self, size: PtySize) -> Result<()> {
        ensure!(
            matches!(self.state, State::Running),
            "configured command is stopping or exited"
        );
        self.startup.master.resize(size)
    }
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        ensure!(
            matches!(self.state, State::Running),
            "configured command input revoked"
        );
        // One nonblocking syscall, bounded chunk; caller retains unsent bytes.
        Ok(self
            .startup
            .writer
            .write(&bytes[..bytes.len().min(16384)])?)
    }
    pub fn read_available(&mut self) -> Result<Vec<u8>> {
        let mut output = std::mem::take(&mut self.startup.tail);
        let mut bytes = [0; 8192];
        while output.len() < 65536 {
            match self.startup.reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(n) => output.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => break,
                Err(e) => return Err(e.into()),
            }
        }
        Ok(output)
    }
    pub fn poll_exit(&mut self) -> Result<Option<i32>> {
        match &self.state {
            State::Exited(code) => return Ok(Some(*code)),
            State::Failed(message) => bail!(message.clone()),
            _ => (),
        }
        let result = self.receive_exit();
        match result {
            Ok(Some(code)) => {
                self.state = State::Exited(code);
                Ok(Some(code))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                self.state = State::Failed(format!("{error:#}"));
                Err(error)
            }
        }
    }
    fn receive_exit(&mut self) -> Result<Option<i32>> {
        let control = self
            .startup
            .control
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("guardian control missing"))?;
        match control.receive()? {
            Some(Message::Exit(code)) => {
                let status = self
                    .startup
                    .reap(Instant::now() + Duration::from_millis(100))?;
                ensure!(
                    status.success(),
                    "guardian exited with {}",
                    status.exit_code()
                );
                Ok(Some(code))
            }
            Some(Message::Failure(message)) => bail!("guardian cleanup failed: {message}"),
            Some(_) => bail!("unexpected guardian response"),
            None => Ok(None),
        }
    }
    pub fn terminate(&mut self) -> Result<()> {
        match &self.state {
            State::Exited(_) => return Ok(()),
            State::Failed(message) => bail!(message.clone()),
            State::Stopping => (),
            State::Running => {
                self.state = State::Stopping;
                if let Some(control) = self.startup.control.as_mut() {
                    control.send(Message::Stop)?;
                }
            }
        }
        let end = Instant::now() + Duration::from_millis(400);
        loop {
            if self.poll_exit()?.is_some() {
                return Ok(());
            }
            super::control::wait(-1, 0, end)?;
        }
    }
}
impl Drop for SupervisedProcess {
    fn drop(&mut self) {
        if let Err(error) = self.terminate() {
            eprintln!("process termination incomplete: {error:#}");
        }
        // Startup closes control and waits even if terminate returned an error.
    }
}
