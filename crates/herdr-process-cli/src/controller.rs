use crate::{arguments::Options, environment};
use anyhow::{Result, bail, ensure};
use herdr_process_adapters::{Endpoint, Peer};
use herdr_process_domain::Action;
use herdr_process_protocol::{Request, Response};
use std::time::{Duration, Instant};
mod startup;

pub fn run(options: &Options, profile: &str, action: Action) -> Result<()> {
    run_in(
        options,
        profile,
        action,
        |name| std::env::var_os(name),
        environment::runtime,
    )
}

fn run_in(
    options: &Options,
    profile: &str,
    action: Action,
    get: impl Fn(&str) -> Option<std::ffi::OsString>,
    runtime: impl Fn(
        &herdr_process_adapters::Herdr,
        &herdr_process_adapters::ConfigurationPaths,
    ) -> std::path::PathBuf,
) -> Result<()> {
    let environment = environment::Environment::read(options, &get)?;
    let configuration = environment.load()?;
    ensure!(
        configuration.profiles().contains_key(profile),
        "unknown profile: {profile}"
    );
    ensure!(
        get("HERDR_ENV").is_some_and(|value| value == "1"),
        "action requires HERDR_ENV=1"
    );
    let host = environment::host(&get)?;
    let target = environment::target(&get)?;
    if matches!(action, Action::SplitRight | Action::SplitBelow) {
        ensure!(
            !target.workspace.is_empty() && !target.pane.is_empty(),
            "split requires a workspace and target pane"
        );
    }
    let runtime = runtime(&host, &environment.paths);
    let endpoint = Endpoint::new(&runtime)?;
    let mut command = startup::command(&environment, &host, &runtime)?;
    let mut connection = startup::connect(&endpoint, &mut command, Duration::from_secs(2))?;
    connection.request_started();
    exchange(
        &mut connection.peer,
        &Request::Action {
            profile: profile.to_owned(),
            action: action.to_string(),
            configuration: configuration.identity().to_owned(),
            target,
        },
        Duration::from_secs(2),
    )
}

pub(crate) fn manager_started(status: &str) {
    if std::env::var_os("HERDR_PROCESS_STARTUP").is_some_and(|value| value == "1") {
        eprintln!("herdr-process:{status}");
    }
}

fn exchange(peer: &mut Peer, request: &Request, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    peer.send(request)?;
    loop {
        ensure!(
            Instant::now() < deadline,
            "manager acknowledgement timed out; action result is unknown"
        );
        peer.flush()?;
        let messages = peer
            .read::<Response>()?
            .ok_or_else(|| anyhow::anyhow!("manager disconnected; action result is unknown"))?;
        if let Some(message) = messages.into_iter().next() {
            match message {
                Response::Ack {} if peer.is_idle() => return Ok(()),
                Response::Error { message } => bail!("{message}"),
                _ => bail!("unexpected manager response; action result is unknown"),
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[cfg(test)]
mod tests;
