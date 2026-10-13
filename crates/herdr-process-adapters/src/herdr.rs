use herdr_process_domain::{Direction, Target};
use std::{ffi::OsString, path::PathBuf, process::Command, time::Duration};
mod arguments;
mod call;
mod reply;
pub use call::HostCall;
use reply::Expected;
pub use reply::{HostFailure, HostFailureCode, HostReply};

const HOST_DEADLINE: Duration = Duration::from_millis(500);

pub struct Herdr {
    pub binary: PathBuf,
    pub socket: PathBuf,
}

pub struct OpenView {
    pub profile: String,
    pub ticket: String,
    pub manager_socket: PathBuf,
    pub cwd: PathBuf,
    pub width: u8,
    pub height: u8,
    pub split: Option<(Target, Direction)>,
}

impl Herdr {
    pub fn open(&self, view: &OpenView) -> anyhow::Result<HostCall> {
        let (arguments, expected) = arguments::open_arguments(view)?;
        self.call(arguments, expected)
    }

    pub fn focus(&self, pane: &str) -> anyhow::Result<HostCall> {
        let (arguments, expected) = arguments::focus_arguments(pane);
        self.call(arguments, expected)
    }

    fn call(&self, arguments: Vec<OsString>, expected: Expected) -> anyhow::Result<HostCall> {
        HostCall::spawn(self.command(arguments), expected, HOST_DEADLINE).map_err(Into::into)
    }

    fn command(&self, arguments: Vec<OsString>) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .env("HERDR_SOCKET_PATH", &self.socket)
            .args(arguments);
        command
    }
}

#[cfg(test)]
mod tests;
