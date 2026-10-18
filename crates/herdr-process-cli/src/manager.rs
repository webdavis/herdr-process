use herdr_process_adapters as adapters;
use herdr_process_application::{Manager, Ports, Settings};
use std::{path::PathBuf, time::Instant};

struct Adapters;

impl Ports for Adapters {
    type Host = adapters::Herdr;
    type Call = adapters::HostCall;
    type Processes = adapters::Supervisor;
    type Process = adapters::SupervisedProcess;
    type Tickets = adapters::RandomTickets;
    type Screen = adapters::Terminal;
    type Connection = adapters::Peer;
}

pub fn run(
    listener: adapters::Listener,
    configuration: adapters::Configuration,
    host: adapters::Herdr,
    supervisor_binary: PathBuf,
    socket: PathBuf,
) -> anyhow::Result<()> {
    let settings = Settings {
        identity: configuration.identity().to_owned(),
        keymap: configuration.keymap().clone(),
        profiles: configuration.profiles().clone(),
    };
    let processes = adapters::Supervisor {
        binary: supervisor_binary,
    };
    let mut manager = Manager::<Adapters>::new(
        settings,
        socket,
        host,
        processes,
        adapters::RandomTickets,
        Instant::now(),
    );
    let mut next = 0usize;
    loop {
        if let Some(peer) = listener.accept()? {
            next = next
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("connection identity exhausted"))?;
            manager.connect(next, peer);
        }
        manager.tick(Instant::now())?;
        if manager.finished() {
            return Ok(());
        }
        wait(&manager, &listener)?;
    }
}

fn wait(manager: &Manager<Adapters>, listener: &adapters::Listener) -> std::io::Result<()> {
    let mut readiness = adapters::Readiness::default();
    listener.watch(&mut readiness);
    for peer in manager.connections() {
        peer.watch(&mut readiness);
    }
    for (process, writing) in manager.processes() {
        process.watch(&mut readiness);
        if writing {
            process.watch_writable(&mut readiness);
        }
    }
    readiness.wait(manager.timeout())
}
