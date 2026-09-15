use herdr_process_adapters as adapters;
use herdr_process_application::{Effect, InputRouter, ViewIntent, view_intent};
use herdr_process_domain::{Action, Placement, Target};
use herdr_process_protocol::{Request, Response};
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};
mod clients;
mod input;
mod requests;
use input::Work;
mod sessions;
mod transitions;
use clients::{Client, Role};
use sessions::Session;
use transitions::{Command, Pending};

struct Manager {
    configuration: adapters::Configuration,
    host: adapters::Herdr,
    supervisor_binary: PathBuf,
    socket: PathBuf,
    clients: BTreeMap<usize, Client>,
    sessions: BTreeMap<String, Session>,
    queue: VecDeque<Work>,
    pending: Option<Pending>,
    epoch: Instant,
    completed: bool,
}

pub fn run(
    listener: adapters::Listener,
    configuration: adapters::Configuration,
    host: adapters::Herdr,
    supervisor_binary: PathBuf,
    socket: PathBuf,
) -> anyhow::Result<()> {
    let mut manager = Manager {
        configuration,
        host,
        supervisor_binary,
        socket,
        clients: BTreeMap::new(),
        sessions: BTreeMap::new(),
        queue: VecDeque::new(),
        pending: None,
        epoch: Instant::now(),
        completed: false,
    };
    let mut next = 0usize;
    loop {
        if let Some(peer) = listener.accept()? {
            next = next
                .checked_add(1)
                .ok_or_else(|| anyhow::anyhow!("connection identity exhausted"))?;
            manager
                .clients
                .insert(next, Client::new(peer, &manager.configuration)?);
        }
        manager.poll_clients();
        manager.poll_sessions()?;
        manager.poll_work();
        manager.poll_transition();
        if manager.completed
            && manager.clients.is_empty()
            && manager.sessions.is_empty()
            && manager.pending.is_none()
            && manager.queue.is_empty()
        {
            return Ok(());
        }
        std::thread::yield_now();
    }
}
