use super::*;
use fakes::FakeTickets;
use herdr_process_domain::{Command, CtrlC};

pub fn target(workspace: &str, pane: &str) -> Target {
    Target {
        workspace: workspace.into(),
        pane: pane.into(),
    }
}

fn profile(ctrl_c: CtrlC) -> Profile {
    Profile {
        command: Command {
            program: "/bin/sh".into(),
            args: vec!["-l".into()],
            cwd: "/home".into(),
        },
        width: 80,
        height: 70,
        ctrl_c,
    }
}

pub struct World {
    pub manager: Manager<Fakes>,
    pub host: FakeHost,
    pub processes: FakeProcesses,
    pub now: Instant,
    next: usize,
}

impl World {
    pub fn new() -> Self {
        let host = FakeHost::default();
        let processes = FakeProcesses::default();
        let now = Instant::now();
        let settings = Settings {
            identity: "config-1".into(),
            keymap: crate::Keymap::new(vec![1], Vec::new()).unwrap(),
            profiles: BTreeMap::from([
                ("shell".to_owned(), profile(CtrlC::Hide)),
                ("quit".to_owned(), profile(CtrlC::Quit)),
            ]),
        };
        let manager = Manager::new(
            settings,
            "/run/manager.sock".into(),
            host.clone(),
            processes.clone(),
            FakeTickets::default(),
            now,
        );
        Self {
            manager,
            host,
            processes,
            now,
            next: 0,
        }
    }

    pub fn tick(&mut self) {
        self.manager.tick(self.now).unwrap();
    }

    pub fn advance(&mut self, millis: u64) {
        self.now += Duration::from_millis(millis);
        self.tick();
    }

    pub fn connect(&mut self, requests: Vec<Request>) -> FakeConnection {
        let connection = FakeConnection::default();
        for request in requests {
            connection.push(request);
        }
        self.next += 1;
        self.manager.connect(self.next, connection.clone());
        connection
    }

    pub fn action(&mut self, profile: &str, action: Action, target: Target) -> FakeConnection {
        self.connect(vec![Request::Action {
            profile: profile.into(),
            action,
            configuration: "config-1".into(),
            target,
        }])
    }

    pub fn attach(&mut self, profile: &str, ticket: &str) -> FakeConnection {
        self.connect(vec![
            Request::Attach {
                profile: profile.into(),
                ticket: ticket.into(),
                rows: 24,
                cols: 80,
            },
            Request::Ready {},
        ])
    }

    pub fn open_float(&mut self, profile: &str, ticket: &str) -> (FakeConnection, FakeConnection) {
        self.host.reply(Ok(HostReply::Opened(None)));
        let controller = self.action(profile, Action::ToggleFloat, target("w1", "p1"));
        self.tick();
        let attached = self.attach(profile, ticket);
        self.tick();
        (controller, attached)
    }

    pub fn open_split(&mut self, profile: &str, pane: &str) -> (FakeConnection, FakeConnection) {
        self.host.reply(Ok(HostReply::Opened(Some(pane.into()))));
        let controller = self.action(profile, Action::SplitRight, target("w1", "p1"));
        self.tick();
        let attached = self.attach(profile, "ticket-1");
        self.tick();
        (controller, attached)
    }
}
