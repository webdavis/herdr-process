use anyhow::{Result, bail, ensure};
use herdr_process_adapters::{AttachmentTerminal, Peer};
use herdr_process_protocol::{MAX_FRAME, Request, Response};
use std::{
    collections::VecDeque,
    io,
    time::{Duration, Instant},
};

#[derive(PartialEq)]
enum Phase {
    Screen,
    Replay,
    Acknowledgement,
    Active,
}

pub(super) fn run(
    peer: &mut Peer,
    terminal: &mut AttachmentTerminal,
    profile: String,
    ticket: String,
    timeout: Duration,
) -> Result<()> {
    let mut size = terminal.size()?;
    peer.send(&Request::Attach {
        profile,
        ticket,
        rows: size.0,
        cols: size.1,
    })?;
    let deadline = Instant::now() + timeout;
    let mut phase = Phase::Screen;
    let mut screens = Screens::default();
    let mut saw_screen = false;
    loop {
        ensure!(
            phase == Phase::Active || Instant::now() < deadline,
            "attachment handshake timed out"
        );
        peer.flush()?;
        let responses = peer
            .read::<Response>()?
            .ok_or_else(|| anyhow::anyhow!("attachment manager disconnected"))?;
        for response in responses {
            match response {
                Response::Screen { bytes } => {
                    saw_screen = true;
                    screens.push(bytes)?;
                }
                Response::Attached {} if phase == Phase::Screen && saw_screen => {
                    phase = Phase::Replay
                }
                Response::Ack {} if phase == Phase::Acknowledgement && peer.is_idle() => {
                    phase = Phase::Active
                }
                Response::Retire {} => return Ok(()),
                Response::Error { message } => bail!("{message}"),
                _ => bail!("unexpected attachment handshake response"),
            }
        }
        screens.write(terminal)?;
        if phase == Phase::Replay && screens.is_empty() {
            peer.send(&Request::Ready {})?;
            phase = Phase::Acknowledgement;
        }
        if phase == Phase::Active && peer.is_idle() {
            let current = terminal.size()?;
            if current != size {
                peer.send(&Request::Resize {
                    rows: current.0,
                    cols: current.1,
                })?;
                size = current;
            }
            match terminal.read()? {
                None => return Ok(()),
                Some(bytes) if !bytes.is_empty() => peer.send(&Request::Input { bytes })?,
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[derive(Default)]
struct Screens {
    pending: VecDeque<Vec<u8>>,
    offset: usize,
    bytes: usize,
}
impl Screens {
    fn push(&mut self, bytes: Vec<u8>) -> Result<()> {
        ensure!(
            self.bytes + bytes.len() <= 2 * MAX_FRAME,
            "attachment screen queue is full"
        );
        if !bytes.is_empty() {
            self.bytes += bytes.len();
            self.pending.push_back(bytes);
        }
        Ok(())
    }
    fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
    fn write(&mut self, terminal: &mut AttachmentTerminal) -> Result<()> {
        let mut budget = 65536;
        while let Some(front) = self.pending.front() {
            if budget == 0 {
                break;
            }
            let end = front.len().min(self.offset + budget);
            match terminal.write(&front[self.offset..end]) {
                Ok(0) => bail!("attachment terminal output closed"),
                Ok(count) => {
                    self.offset += count;
                    self.bytes -= count;
                    budget -= count;
                    if self.offset == front.len() {
                        self.pending.pop_front();
                        self.offset = 0;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
