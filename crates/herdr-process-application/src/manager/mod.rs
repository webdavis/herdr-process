mod host;
pub use host::{HostFailure, HostFailureCode, HostReply, OpenView};
mod ports;
pub use ports::{Call, Connection, Host, Ports, Process, Processes, Screen, Tickets};
mod clients;
mod input;
mod requests;
mod sessions;
mod transitions;
use crate::{Effect, InputRouter, ViewIntent, view_intent};
use clients::{Client, Role};
use herdr_process_domain::{Action, Direction, Placement, Profile, Target};
use herdr_process_protocol::{Request, Response};
use input::Work;
use sessions::Session;
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};
use transitions::{Candidate, Command, Origin, Stage, Step, Transition};

const TICK: Duration = Duration::from_millis(10);
const RETIRE_GRACE: Duration = Duration::from_millis(500);

pub struct Settings {
    pub identity: String,
    pub keymap: crate::Keymap,
    pub profiles: BTreeMap<String, Profile>,
}

pub struct Manager<P: Ports> {
    settings: Settings,
    host: P::Host,
    processes: P::Processes,
    tickets: P::Tickets,
    socket: PathBuf,
    clients: BTreeMap<usize, Client<P>>,
    sessions: BTreeMap<String, Session<P>>,
    queue: VecDeque<Work>,
    pending: Option<Transition<P>>,
    epoch: Instant,
    now: Instant,
    completed: bool,
}

impl<P: Ports> Manager<P> {
    pub fn new(
        settings: Settings,
        socket: PathBuf,
        host: P::Host,
        processes: P::Processes,
        tickets: P::Tickets,
        now: Instant,
    ) -> Self {
        Self {
            settings,
            host,
            processes,
            tickets,
            socket,
            clients: BTreeMap::new(),
            sessions: BTreeMap::new(),
            queue: VecDeque::new(),
            pending: None,
            epoch: now,
            now,
            completed: false,
        }
    }

    pub fn connect(&mut self, id: usize, connection: P::Connection) {
        self.clients
            .insert(id, Client::new(connection, &self.settings.keymap));
    }

    pub fn tick(&mut self, now: Instant) -> anyhow::Result<()> {
        self.now = now;
        self.poll_clients();
        self.poll_sessions()?;
        self.poll_work();
        self.poll_transition();
        Ok(())
    }

    pub fn finished(&self) -> bool {
        self.completed
            && self.clients.is_empty()
            && self.sessions.is_empty()
            && self.pending.is_none()
            && self.queue.is_empty()
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.next_deadline()
            .map(|deadline| deadline.saturating_duration_since(self.now))
    }

    fn next_deadline(&self) -> Option<Instant> {
        let host_call = self.pending.as_ref().map(|_| self.now + TICK);
        let blocked_work = (self.pending.is_none() && !self.queue.is_empty()).then_some(self.now);
        let clients = self
            .clients
            .values()
            .filter_map(|client| match &client.role {
                Role::Retiring(since) => Some(*since + RETIRE_GRACE),
                Role::Active(_) => client.router.deadline().map(|at| self.epoch + at),
                _ => None,
            });
        host_call
            .into_iter()
            .chain(blocked_work)
            .chain(clients)
            .min()
    }

    pub fn connections(&self) -> impl Iterator<Item = &P::Connection> {
        self.clients.values().map(|client| &client.peer)
    }

    pub fn processes(&self) -> impl Iterator<Item = (&P::Process, bool)> {
        self.sessions
            .values()
            .map(|session| (&session.process, session.writing()))
    }

    fn elapsed(&self) -> Duration {
        self.now.saturating_duration_since(self.epoch)
    }
}

#[cfg(test)]
mod tests;
