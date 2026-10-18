use herdr_process_domain::{Direction, Target};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenView {
    pub profile: String,
    pub ticket: String,
    pub manager_socket: PathBuf,
    pub cwd: PathBuf,
    pub width: u8,
    pub height: u8,
    pub split: Option<(Target, Direction)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostReply {
    Opened(Option<String>),
    Focused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostFailureCode {
    UiBusy,
    Rejected,
    Spawn,
    Io,
    Exit,
    Malformed,
    Oversized,
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostFailure {
    pub code: HostFailureCode,
}

impl HostFailure {
    pub fn is_busy(&self) -> bool {
        self.code == HostFailureCode::UiBusy
    }
    pub fn new(code: HostFailureCode) -> Self {
        Self { code }
    }
}

impl std::fmt::Display for HostFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.code {
            HostFailureCode::UiBusy => "herdr is busy, try again",
            HostFailureCode::Rejected => "herdr refused the request",
            HostFailureCode::Spawn => "could not start herdr",
            HostFailureCode::Io => "could not talk to herdr",
            HostFailureCode::Exit => "herdr exited with an error",
            HostFailureCode::Malformed => "herdr sent a reply that could not be read",
            HostFailureCode::Oversized => "herdr sent a reply that is too large",
            HostFailureCode::Timeout => "herdr did not answer in time",
        })
    }
}

impl std::error::Error for HostFailure {}
