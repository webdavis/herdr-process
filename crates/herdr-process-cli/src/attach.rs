use anyhow::{Context, Result, ensure};
use herdr_process_adapters::{AttachmentTerminal, Endpoint};
use herdr_process_protocol::Request;
use std::{
    os::fd::AsFd,
    path::PathBuf,
    time::{Duration, Instant},
};
mod session;

pub fn run() -> Result<()> {
    let socket = PathBuf::from(
        std::env::var_os("HERDR_PROCESS_SOCKET").context("attachment socket is missing")?,
    );
    let profile =
        std::env::var("HERDR_PROCESS_PROFILE").context("attachment profile is missing")?;
    let ticket = std::env::var_os("HERDR_PROCESS_TICKET")
        .context("attachment ticket is missing")?
        .into_string()
        .map_err(|_| anyhow::anyhow!("invalid attachment ticket encoding"))?;
    ensure!(
        !profile.is_empty() && !ticket.is_empty(),
        "attachment credentials are empty"
    );
    let endpoint = Endpoint::new(socket.parent().context("invalid attachment socket")?)?;
    ensure!(
        endpoint.socket() == socket,
        "invalid attachment socket name"
    );
    let mut peer = endpoint.connect().context("cannot connect attachment")?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut terminal = AttachmentTerminal::enter(stdin.as_fd(), stdout.as_fd())?;
    let result = session::run(
        &mut peer,
        &mut terminal,
        profile,
        ticket,
        Duration::from_secs(2),
    );
    let restored = terminal.finish();
    if peer.send(&Request::Detach {}).is_ok() {
        let deadline = Instant::now() + Duration::from_millis(50);
        while Instant::now() < deadline {
            match peer.flush() {
                Ok(false) => std::thread::sleep(Duration::from_millis(1)),
                _ => break,
            }
        }
    }
    result.and(restored.map_err(Into::into))
}
