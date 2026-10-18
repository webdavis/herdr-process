use crate::{Readiness, unix};
use herdr_process_application::Connection;
use herdr_process_protocol::{Decoder, MAX_FRAME, Request, Response, encode};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
};

pub struct Peer {
    stream: UnixStream,
    decoder: Decoder,
    outgoing: VecDeque<Vec<u8>>,
    offset: usize,
    queued: usize,
}

impl Peer {
    pub fn new(stream: UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            decoder: Decoder::default(),
            outgoing: VecDeque::new(),
            offset: 0,
            queued: 0,
        })
    }

    pub fn read<T: DeserializeOwned>(&mut self) -> io::Result<Option<Vec<T>>> {
        let mut bytes = [0; 8192];
        match unix::ready(self.stream.read(&mut bytes))? {
            Some(0) => {
                self.decoder.finish()?;
                Ok(None)
            }
            Some(count) => self.decoder.feed(&bytes[..count]).map(Some),
            None => Ok(Some(Vec::new())),
        }
    }

    pub fn send<T: Serialize>(&mut self, message: &T) -> io::Result<()> {
        let bytes = encode(message)?;
        if self.queued + bytes.len() > 2 * MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "connection output queue is full",
            ));
        }
        self.queued += bytes.len();
        self.outgoing.push_back(bytes);
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<bool> {
        let mut budget = 256 * 1024;
        while let Some(front) = self.outgoing.front() {
            if budget == 0 {
                break;
            }
            let end = front.len().min(self.offset + budget);
            match unix::ready(self.stream.write(&front[self.offset..end]))? {
                Some(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "connection stopped accepting output",
                    ));
                }
                Some(count) => {
                    self.offset += count;
                    self.queued -= count;
                    budget -= count;
                    if self.offset == front.len() {
                        self.outgoing.pop_front();
                        self.offset = 0;
                    }
                }
                None => break,
            }
        }
        Ok(self.is_idle())
    }

    pub fn is_idle(&self) -> bool {
        self.outgoing.is_empty()
    }

    pub fn watch(&self, readiness: &mut Readiness) {
        readiness.readable(self.stream.as_raw_fd());
        if !self.is_idle() {
            self.watch_writable(readiness);
        }
    }

    pub fn watch_writable(&self, readiness: &mut Readiness) {
        readiness.writable(self.stream.as_raw_fd());
    }
}

#[cfg(test)]
mod tests;

impl Connection for Peer {
    fn read(&mut self) -> io::Result<Option<Vec<Request>>> {
        Peer::read(self)
    }
    fn send(&mut self, response: &Response) -> io::Result<()> {
        Peer::send(self, response)
    }
    fn flush(&mut self) -> io::Result<bool> {
        Peer::flush(self)
    }
    fn is_idle(&self) -> bool {
        Peer::is_idle(self)
    }
}
