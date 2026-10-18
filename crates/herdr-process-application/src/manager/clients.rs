use super::*;

#[derive(Clone, PartialEq)]
pub(super) enum Role {
    Fresh,
    Controller,
    Candidate(String),
    Active(String),
    Retiring(Instant),
}
pub(super) struct Client<P: Ports> {
    pub peer: P::Connection,
    pub router: InputRouter,
    pub role: Role,
    pub screen: Option<Vec<u8>>,
    pub failed: bool,
}
impl<P: Ports> Client<P> {
    pub fn new(peer: P::Connection, keymap: &crate::Keymap) -> Self {
        Self {
            peer,
            router: InputRouter::new(keymap),
            role: Role::Fresh,
            screen: None,
            failed: false,
        }
    }
    pub fn send(&mut self, response: Response) {
        if self.peer.send(&response).is_err() {
            self.failed = true;
        }
    }
    pub fn retire(&mut self, now: Instant) {
        self.role = Role::Retiring(now);
        if let Some(bytes) = self.screen.take() {
            self.send(Response::Screen { bytes });
        }
        self.send(Response::Retire {});
    }
}
impl<P: Ports> Manager<P> {
    pub(super) fn send(&mut self, id: usize, response: Response) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.send(response);
        }
    }
    pub(super) fn retire(&mut self, id: usize) {
        if let Some(client) = self.clients.get_mut(&id) {
            client.retire(self.now);
        }
    }
    pub(super) fn poll_clients(&mut self) {
        let elapsed = self.elapsed();
        let ids: Vec<_> = self.clients.keys().copied().collect();
        for id in ids {
            if matches!(self.clients[&id].role, Role::Retiring(since) if self.now.saturating_duration_since(since) >= Duration::from_millis(500))
            {
                self.remove_client(id);
                continue;
            }
            let requests = self.clients.get_mut(&id).unwrap().peer.read();
            match requests {
                Ok(Some(requests)) => {
                    for request in requests {
                        self.request(id, request);
                    }
                }
                _ => {
                    self.disconnect(id);
                    continue;
                }
            }
            if let Some(client) = self.clients.get_mut(&id)
                && let Role::Active(_) = client.role
            {
                let effects = client.router.expire(elapsed);
                self.effects(id, effects);
            }
            if let Some(client) = self.clients.get_mut(&id) {
                if client.peer.is_idle()
                    && let Some(bytes) = client.screen.take()
                {
                    client.send(Response::Screen { bytes });
                }
                if client.failed || client.peer.flush().is_err() {
                    self.disconnect(id);
                }
            }
        }
    }
    pub(super) fn disconnect(&mut self, id: usize) {
        if let Some(Transition {
            step: Step::Opening(opening),
            ..
        }) = &mut self.pending
            && matches!(opening.stage, Stage::Retiring(waiting) if waiting == id)
        {
            opening.stage = Stage::RetryAt(self.now);
        }
        self.remove_client(id);
    }
    fn remove_client(&mut self, id: usize) {
        self.clients.remove(&id);
        for session in self.sessions.values_mut() {
            if session.view.as_ref().is_some_and(|view| view.0 == id) {
                session.view = None;
            }
        }
    }
}
