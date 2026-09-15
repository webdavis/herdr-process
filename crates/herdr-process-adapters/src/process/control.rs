use super::ProcessSpec;
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
    Start(ProcessSpec),
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
            match self.stream.write(&bytes[offset..]) {
                Ok(0) => bail!("guardian control write closed"),
                Ok(n) => offset += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    wait(self.stream.as_raw_fd(), libc::POLLOUT, end)?
                }
                Err(e) => return Err(e.into()),
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
            match self.stream.read(&mut bytes) {
                Ok(0) => bail!("guardian control EOF"),
                Ok(n) => self.buffer.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(None),
                Err(e) => return Err(e.into()),
            }
        }
    }
    pub fn until(&mut self, end: Instant) -> Result<Message> {
        loop {
            if let Some(message) = self.receive()? {
                return Ok(message);
            }
            wait(self.stream.as_raw_fd(), libc::POLLIN, end)?;
        }
    }
}
pub(super) fn wait(fd: i32, events: i16, end: Instant) -> Result<()> {
    ensure!(Instant::now() < end, "guardian deadline exceeded");
    let timeout = end
        .saturating_duration_since(Instant::now())
        .as_millis()
        .min(5) as i32;
    let mut p = libc::pollfd {
        fd,
        events,
        revents: 0,
    };
    // The descriptor is borrowed from an owner that remains alive throughout poll.
    let n = unsafe { libc::poll(&mut p, 1, timeout) };
    if n < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
        bail!(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/control.rs"]
mod tests;
