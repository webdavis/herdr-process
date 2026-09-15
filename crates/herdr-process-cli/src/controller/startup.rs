use crate::environment::Environment;
use anyhow::{Context, Result, bail, ensure};
use herdr_process_adapters::{Endpoint, Herdr, Peer};
use std::{
    fs::{File, OpenOptions},
    io,
    os::unix::{
        fs::{FileExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub(super) struct Launch {
    command: Command,
    diagnostic: File,
}
pub(super) struct Connection {
    pub peer: Peer,
    candidate: Option<Candidate>,
}
struct Candidate {
    child: Child,
    unstarted: bool,
}
impl Connection {
    pub fn request_started(&mut self) {
        if let Some(candidate) = &mut self.candidate {
            candidate.unstarted = false;
        }
    }
}
impl Drop for Candidate {
    fn drop(&mut self) {
        if self.unstarted {
            let _ = self.child.kill();
            let _ = self.child.wait();
        } else {
            let _ = self.child.try_wait();
        }
    }
}

pub(super) fn command(environment: &Environment, host: &Herdr, runtime: &Path) -> Result<Launch> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let diagnostic = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(runtime.join(format!("startup-{}-{stamp}.log", std::process::id())))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("manager")
        .arg("--runtime-dir")
        .arg(runtime)
        .arg("--profiles")
        .arg(&environment.paths.profiles)
        .arg("--herdr-config")
        .arg(&environment.paths.herdr)
        .env("HOME", &environment.home)
        .env("HERDR_BIN_PATH", &host.binary)
        .env("HERDR_SOCKET_PATH", &host.socket)
        .env("HERDR_PROCESS_STARTUP", "1")
        .env_remove("HERDR_PROCESS_TICKET")
        .env_remove("HERDR_PROCESS_PROFILE")
        .env_remove("HERDR_PROCESS_SOCKET")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(diagnostic.try_clone()?);
    Ok(Launch {
        command,
        diagnostic,
    })
}

pub(super) fn connect(
    endpoint: &Endpoint,
    launch: &mut Launch,
    timeout: Duration,
) -> Result<Connection> {
    if let Some(peer) = available(endpoint)? {
        return Ok(Connection {
            peer,
            candidate: None,
        });
    }
    let deadline = Instant::now() + timeout;
    let child = launch
        .command
        .process_group(0)
        .spawn()
        .context("manager startup failed")?;
    let mut candidate = Some(Candidate {
        child,
        unstarted: true,
    });
    let mut ready = false;
    loop {
        ensure!(Instant::now() < deadline, "manager startup timed out");
        let mut bytes = [0; 8192];
        let length = launch.diagnostic.read_at(&mut bytes, 0)?;
        let log = &bytes[..length];
        ensure!(
            length < bytes.len(),
            "manager startup diagnostics exceeded limit"
        );
        if log.starts_with(b"herdr-process:ready\n") {
            ready = true;
        }
        if let Some(owner) = candidate.as_mut()
            && let Some(status) = owner.child.try_wait()?
        {
            ensure!(
                status.success() && log.starts_with(b"herdr-process:duplicate\n"),
                "manager startup failed"
            );
            candidate = None;
            ready = true;
        }
        if ready && let Some(peer) = available(endpoint)? {
            return Ok(Connection { peer, candidate });
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn available(endpoint: &Endpoint) -> Result<Option<Peer>> {
    match endpoint.connect() {
        Ok(peer) => Ok(Some(peer)),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound
                    | io::ErrorKind::ConnectionRefused
                    | io::ErrorKind::WouldBlock
            ) =>
        {
            Ok(None)
        }
        Err(error) => bail!("cannot connect manager: {error}"),
    }
}
#[cfg(test)]
mod tests;
