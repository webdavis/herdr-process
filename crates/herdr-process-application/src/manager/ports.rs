use super::{HostFailure, HostReply, OpenView};
use herdr_process_domain::Command;
use herdr_process_protocol::{Request, Response};
use std::io;

pub trait Ports {
    type Host: Host<Call = Self::Call>;
    type Call: Call;
    type Processes: Processes<Process = Self::Process>;
    type Process: Process;
    type Tickets: Tickets;
    type Screen: Screen;
    type Connection: Connection;
}

pub trait Host {
    type Call: Call;
    fn open(&self, view: &OpenView) -> anyhow::Result<Self::Call>;
    fn focus(&self, pane: &str) -> anyhow::Result<Self::Call>;
}

pub trait Call {
    fn poll(&mut self) -> Result<Option<HostReply>, HostFailure>;
}

pub trait Processes {
    type Process: Process;
    fn spawn(&self, command: &Command, rows: u16, cols: u16) -> anyhow::Result<Self::Process>;
}

pub trait Process {
    fn write(&mut self, bytes: &[u8]) -> anyhow::Result<usize>;
    fn read_available(&mut self) -> anyhow::Result<Vec<u8>>;
    fn poll_exit(&mut self) -> anyhow::Result<Option<i32>>;
    fn resize(&mut self, rows: u16, cols: u16) -> anyhow::Result<()>;
    fn terminate(&mut self) -> anyhow::Result<()>;
}

pub trait Tickets {
    fn issue(&self) -> String;
}

pub trait Screen {
    fn new(rows: u16, cols: u16) -> Self;
    fn feed(&mut self, bytes: &[u8]);
    fn resize(&mut self, rows: u16, cols: u16);
    fn snapshot(&self) -> Vec<u8>;
    fn take_responses(&mut self) -> Vec<u8>;
}

pub trait Connection {
    fn read(&mut self) -> io::Result<Option<Vec<Request>>>;
    fn send(&mut self, response: &Response) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<bool>;
    fn is_idle(&self) -> bool;
}
