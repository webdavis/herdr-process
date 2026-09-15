use super::*;
impl Manager {
    pub(super) fn request(&mut self, id: usize, request: Request) {
        if !self.clients.contains_key(&id) {
            return;
        }
        if let Err(error) = self.receive(id, request) {
            self.send(
                id,
                Response::Error {
                    message: error.to_string(),
                },
            );
            self.completed = true;
        }
    }
    fn receive(&mut self, id: usize, request: Request) -> anyhow::Result<()> {
        let role = self.clients[&id].role.clone();
        match request {
            Request::Action {
                profile,
                action,
                configuration,
                target,
            } => {
                anyhow::ensure!(role == Role::Fresh, "connection already assigned");
                anyhow::ensure!(
                    configuration == self.configuration.identity(),
                    "configuration changed; retry with the current configuration"
                );
                let action = action.parse()?;
                anyhow::ensure!(
                    self.configuration.profiles().contains_key(&profile),
                    "unknown profile"
                );
                anyhow::ensure!(self.queue.len() < 128, "request queue full; retry");
                self.clients.get_mut(&id).unwrap().role = Role::Controller;
                self.queue.push_back(Work::Command(Command {
                    reply: Some(id),
                    profile,
                    action,
                    target: Target {
                        workspace: target.workspace,
                        pane: target.pane,
                    },
                }));
            }
            Request::Attach {
                profile,
                ticket,
                rows,
                cols,
            } => {
                anyhow::ensure!(role == Role::Fresh, "connection already assigned");
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("stale attachment"))?;
                anyhow::ensure!(
                    pending.command.profile == profile
                        && pending.ticket == ticket
                        && pending.candidate.is_none(),
                    "invalid attachment capability"
                );
                let session = self
                    .sessions
                    .get_mut(&profile)
                    .ok_or_else(|| anyhow::anyhow!("session exited"))?;
                sessions::validate_size(rows, cols)?;
                pending.dimensions = Some((rows, cols));
                pending.candidate = Some(id);
                let screen = session.preview(rows, cols);
                self.clients.get_mut(&id).unwrap().role = Role::Candidate(profile);
                self.send(id, Response::Screen { bytes: screen });
                self.send(id, Response::Attached {});
            }
            Request::Ready {} => {
                anyhow::ensure!(
                    matches!(role, Role::Candidate(_)),
                    "attachment is not awaiting readiness"
                );
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("stale attachment"))?;
                anyhow::ensure!(
                    pending.candidate == Some(id) && !pending.ready,
                    "duplicate readiness"
                );
                pending.ready = true;
            }
            Request::Input { bytes } => {
                anyhow::ensure!(
                    matches!(role, Role::Active(_)),
                    "input requires committed attachment"
                );
                let effects = self
                    .clients
                    .get_mut(&id)
                    .unwrap()
                    .router
                    .feed(&bytes, self.epoch.elapsed());
                self.effects(id, effects);
            }
            Request::Resize { rows, cols } => {
                let Role::Active(profile) = role else {
                    anyhow::bail!("resize requires committed attachment")
                };
                self.sessions
                    .get_mut(&profile)
                    .ok_or_else(|| anyhow::anyhow!("session exited"))?
                    .resize(rows, cols)?;
                let screen = self.sessions[&profile].terminal.snapshot();
                self.clients.get_mut(&id).unwrap().screen = Some(screen);
            }
            Request::Detach {} => {
                anyhow::ensure!(
                    matches!(role, Role::Active(_) | Role::Candidate(_)),
                    "detach requires attachment"
                );
                self.retire(id);
                for session in self.sessions.values_mut() {
                    if session.view.as_ref().is_some_and(|v| v.0 == id) {
                        session.view = None;
                    }
                }
            }
        }
        Ok(())
    }
}
