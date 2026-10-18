use super::Command;
use crate::{Readiness, unix};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    os::unix::{io::AsRawFd, net::UnixStream},
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
pub(super) enum Message {
    Hello { secret: String, pid: u32 },
    Start(Command),
    Ready(u32),
    Stop,
    Exit(i32),
    Failure(String),
}
pub(super) struct Control {
    pub stream: UnixStream,
    buffer: Vec<u8>,
}
impl Control {
    pub fn new(stream: UnixStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
        })
    }
    pub fn send(&mut self, message: Message) -> Result<()> {
        let mut bytes = serde_json::to_vec(&message)?;
        ensure!(bytes.len() < 65536, "guardian message too large");
        bytes.push(b'\n');
        let end = Instant::now() + Duration::from_millis(200);
        let mut offset = 0;
        while offset < bytes.len() {
            match unix::ready(self.stream.write(&bytes[offset..]))? {
                Some(0) => bail!("guardian control write closed"),
                Some(n) => offset += n,
                None => wait(|readiness| readiness.writable(self.stream.as_raw_fd()), end)?,
            }
        }
        Ok(())
    }
    pub fn receive(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
                ensure!(end < 65536, "guardian frame exceeds limit");
                let message = serde_json::from_slice(&self.buffer[..end])?;
                self.buffer.drain(..=end);
                return Ok(Some(message));
            }
            ensure!(self.buffer.len() < 65536, "guardian frame exceeds limit");
            let mut bytes = [0; 4096];
            match unix::ready(self.stream.read(&mut bytes))? {
                Some(0) => bail!("guardian control EOF"),
                Some(n) => self.buffer.extend_from_slice(&bytes[..n]),
                None => return Ok(None),
            }
        }
    }
    pub fn until(&mut self, end: Instant) -> Result<Message> {
        loop {
            if let Some(message) = self.receive()? {
                return Ok(message);
            }
            wait(|readiness| readiness.readable(self.stream.as_raw_fd()), end)?;
        }
    }
}
pub(super) fn wait(watch: impl FnOnce(&mut Readiness), end: Instant) -> Result<()> {
    ensure!(Instant::now() < end, "guardian deadline exceeded");
    let mut readiness = Readiness::default();
    watch(&mut readiness);
    let timeout = end
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(5));
    Ok(readiness.wait(Some(timeout))?)
}

#[cfg(test)]
mod tests;
