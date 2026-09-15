use super::*;
impl Manager {
    pub(in crate::manager) fn execute(&mut self, command: Command) {
        match self.start(&command) {
            Ok(Some(pending)) => self.pending = Some(Pending { command, ..pending }),
            Ok(None) => self.finish(&command, Ok(())),
            Err(error) => self.finish(&command, Err(error)),
        }
    }
    fn start(&mut self, command: &Command) -> anyhow::Result<Option<Pending>> {
        let current = self
            .sessions
            .get(&command.profile)
            .and_then(|s| s.view.as_ref().map(|v| &v.1));
        let intent = view_intent(command.action, current, &command.target);
        match intent {
            ViewIntent::Kill => {
                if let Some(mut session) = self.sessions.remove(&command.profile) {
                    if let Some((id, _)) = session.view.take() {
                        self.retire(id);
                    }
                    session.process.terminate()?;
                }
                return Ok(None);
            }
            ViewIntent::Hide => {
                if let Some(session) = self.sessions.get_mut(&command.profile)
                    && let Some((id, _)) = session.view.take()
                {
                    self.retire(id);
                }
                return Ok(None);
            }
            ViewIntent::OpenSplit(_) => anyhow::ensure!(
                !command.target.workspace.is_empty() && !command.target.pane.is_empty(),
                "split needs a workspace and pane; invoke from a current pane"
            ),
            _ => {}
        }
        if !self.sessions.contains_key(&command.profile) {
            let profile = self
                .configuration
                .profiles()
                .get(&command.profile)
                .ok_or_else(|| anyhow::anyhow!("unknown profile"))?;
            self.sessions.insert(
                command.profile.clone(),
                Session::spawn(&self.supervisor_binary, profile)?,
            );
        }
        let mut pending = Pending {
            command: Command {
                reply: command.reply,
                profile: command.profile.clone(),
                action: command.action,
                target: command.target.clone(),
            },
            ticket: ticket()?,
            candidate: None,
            ready: false,
            dimensions: None,
            call: None,
            opened: None,
            intent,
            deadline: Instant::now() + Duration::from_millis(600),
            waiting: None,
            retry: Instant::now(),
        };
        if intent == ViewIntent::Focus {
            if let Some((_, Placement::Docked { pane, .. })) = &self.sessions[&command.profile].view
            {
                pending.call = Some(self.host.focus(pane)?);
            }
        } else if intent == ViewIntent::OpenFloat {
            let old = self.sessions.values_mut().find_map(|session| {
                if session
                    .view
                    .as_ref()
                    .is_some_and(|v| v.1 == Placement::Floating)
                {
                    session.view.take().map(|v| v.0)
                } else {
                    None
                }
            });
            if let Some(id) = old {
                self.retire(id);
                pending.waiting = Some(id);
            }
        }
        Ok(Some(pending))
    }
}
