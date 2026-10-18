use herdr_process_domain::{Direction, Target};
use std::path::PathBuf;

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
    pub message: &'static str,
}

impl HostFailure {
    pub fn is_busy(&self) -> bool {
        self.code == HostFailureCode::UiBusy
    }
    pub fn new(code: HostFailureCode) -> Self {
        Self {
            code,
            message: "Herdr host call failed",
        }
    }
}

impl std::fmt::Display for HostFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({:?})", self.message, self.code)
    }
}

impl std::error::Error for HostFailure {}
