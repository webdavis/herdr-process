use herdr_process_domain::{Direction, Target};
use std::{path::PathBuf, process::Command};
mod call;
mod reply;
pub use call::HostCall;
pub use reply::{HostFailure, HostFailureCode, HostReply};

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
        let mut command = self.command();
        command.args([
            "open",
            "--plugin",
            "herdr-process",
            "--entrypoint",
            "attach",
        ]);
        let expected = if let Some((target, direction)) = &view.split {
            command.args([
                "--placement",
                "split",
                "--workspace",
                &target.workspace,
                "--target-pane",
                &target.pane,
                "--direction",
                match direction {
                    Direction::Right => "right",
                    Direction::Below => "down",
                },
            ]);
            reply::Expected::Split
        } else {
            anyhow::ensure!(
                (1..=100).contains(&view.width) && (1..=100).contains(&view.height),
                "invalid popup dimensions"
            );
            command.args([
                "--placement",
                "popup",
                "--width",
                &format!("{}%", view.width),
                "--height",
                &format!("{}%", view.height),
            ]);
            reply::Expected::Popup
        };
        command.arg("--cwd").arg(&view.cwd);
        let mut socket = std::ffi::OsString::from("HERDR_PROCESS_SOCKET=");
        socket.push(&view.manager_socket);
        command
            .arg("--env")
            .arg(socket)
            .arg("--env")
            .arg(format!("HERDR_PROCESS_PROFILE={}", view.profile))
            .arg("--env")
            .arg(format!("HERDR_PROCESS_TICKET={}", view.ticket));
        HostCall::spawn(command, expected).map_err(Into::into)
    }

    pub fn focus(&self, pane: &str) -> anyhow::Result<HostCall> {
        let mut command = self.command();
        command.args(["focus", pane]);
        HostCall::spawn(command, reply::Expected::Focus(pane.to_owned())).map_err(Into::into)
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .env("HERDR_SOCKET_PATH", &self.socket)
            .args(["plugin", "pane"]);
        command
    }
}

#[cfg(test)]
mod tests;
