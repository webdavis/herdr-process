use super::*;
const SESSION_EXITED: &str = "session exited during view replacement; retry to start again";
impl<P: Ports> Manager<P> {
    pub(super) fn advance(&mut self, transition: &mut Transition<P>) -> anyhow::Result<bool> {
        anyhow::ensure!(
            self.sessions.contains_key(&transition.command.profile),
            SESSION_EXITED
        );
        anyhow::ensure!(
            self.now < transition.deadline,
            "view replacement timed out; retry"
        );
        match &mut transition.step {
            Step::Focusing(call) => Ok(call.poll()?.is_some()),
            Step::Opening(opening) => self.open(&transition.command, opening),
        }
    }
    fn open(&mut self, command: &Command, opening: &mut Opening<P>) -> anyhow::Result<bool> {
        if let Some(candidate) = opening.candidate() {
            anyhow::ensure!(
                self.clients
                    .get(&candidate.id)
                    .is_some_and(|c| matches!(c.role, Role::Candidate(_))),
                "candidate disconnected; retry"
            );
        }
        match &opening.stage {
            Stage::Retiring(id) if self.clients.contains_key(id) => return Ok(false),
            Stage::Retiring(_) => anyhow::bail!("retired attachment did not disconnect; retry"),
            Stage::RetryAt(at) if self.now < *at => return Ok(false),
            Stage::RetryAt(_) => {
                let call = self.host.open(&self.view(command, opening))?;
                opening.stage = Stage::Calling {
                    call,
                    candidate: None,
                };
            }
            _ => {}
        }
        if let Stage::Calling { call, candidate } = &mut opening.stage {
            match call.poll() {
                Ok(None) => return Ok(false),
                Ok(Some(HostReply::Focused)) => return Ok(true),
                Ok(Some(HostReply::Opened(pane))) => {
                    let placement = match opening.view {
                        Open::Float => Placement::Floating,
                        Open::Split(direction) => Placement::Docked {
                            target: command.target.clone(),
                            direction,
                            pane: pane
                                .ok_or_else(|| anyhow::anyhow!("split did not return a pane"))?,
                        },
                    };
                    opening.stage = Stage::Opened {
                        placement,
                        candidate: candidate.take(),
                    };
                }
                Err(error) if error.is_busy() && opening.view == Open::Float => {
                    if let Some(candidate) = candidate.take() {
                        self.retire(candidate.id);
                    }
                    opening.ticket = self.tickets.issue();
                    opening.stage = Stage::RetryAt(self.now + Duration::from_millis(10));
                    return Ok(false);
                }
                Err(error) => return Err(error.into()),
            }
        }
        if let Stage::Opened {
            placement,
            candidate: Some(candidate),
        } = &opening.stage
            && candidate.ready
        {
            self.commit(command, *candidate, placement.clone())?;
            return Ok(true);
        }
        Ok(false)
    }
    fn view(&self, command: &Command, opening: &Opening<P>) -> OpenView {
        let profile = &self.settings.profiles[&command.profile];
        OpenView {
            profile: command.profile.clone(),
            ticket: opening.ticket.clone(),
            manager_socket: self.socket.clone(),
            cwd: profile.command.cwd.clone(),
            width: profile.width,
            height: profile.height,
            split: match opening.view {
                Open::Float => None,
                Open::Split(direction) => Some((command.target.clone(), direction)),
            },
        }
    }
    fn commit(
        &mut self,
        command: &Command,
        candidate: Candidate,
        mut placement: Placement,
    ) -> anyhow::Result<()> {
        let session = self
            .sessions
            .get_mut(&command.profile)
            .ok_or_else(|| anyhow::anyhow!(SESSION_EXITED))?;
        session.resize(candidate.rows, candidate.cols)?;
        let screen = session.terminal.snapshot();
        session.context = session.context_for(&command.target);
        if let Placement::Docked { target, .. } = &mut placement {
            *target = session.context.clone();
        }
        let old = session.view.replace((candidate.id, placement));
        if let Some(client) = self.clients.get_mut(&candidate.id) {
            client.role = Role::Active(command.profile.clone());
            client.screen = None;
        }
        self.send(candidate.id, Response::Screen { bytes: screen });
        self.send(candidate.id, Response::Ack {});
        if let Some((old, _)) = old {
            self.retire(old);
        }
        Ok(())
    }
}
