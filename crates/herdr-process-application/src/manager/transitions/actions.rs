use super::*;
impl<P: Ports> Manager<P> {
    pub(in crate::manager) fn execute(&mut self, command: Command) {
        match self.start(&command) {
            Ok(Some(step)) => {
                self.pending = Some(Transition {
                    command,
                    deadline: self.now + Duration::from_millis(600),
                    step,
                })
            }
            Ok(None) => self.finish(&command, Ok(())),
            Err(error) => self.finish(&command, Err(error)),
        }
    }
    fn start(&mut self, command: &Command) -> anyhow::Result<Option<Step<P>>> {
        let current = self
            .sessions
            .get(&command.profile)
            .and_then(|s| s.view.as_ref().map(|v| &v.1));
        let view = match view_intent(command.action, current, &command.target) {
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
            ViewIntent::Focus => {
                let Some(Placement::Docked { pane, .. }) = current else {
                    anyhow::bail!("no docked view to focus");
                };
                return Ok(Some(Step::Focusing(self.host.focus(pane)?)));
            }
            ViewIntent::OpenSplit(direction) => {
                anyhow::ensure!(
                    !command.target.workspace.is_empty() && !command.target.pane.is_empty(),
                    "split needs a workspace and pane; invoke from a current pane"
                );
                Open::Split(direction)
            }
            ViewIntent::OpenFloat => Open::Float,
        };
        if !self.sessions.contains_key(&command.profile) {
            let profile = self
                .settings
                .profiles
                .get(&command.profile)
                .ok_or_else(|| anyhow::anyhow!("unknown profile"))?;
            self.sessions.insert(
                command.profile.clone(),
                Session::spawn(&self.processes, profile)?,
            );
        }
        let stage = match view {
            Open::Float => self
                .retire_floating_view()
                .map_or(Stage::RetryAt(self.now), Stage::Retiring),
            Open::Split(_) => Stage::RetryAt(self.now),
        };
        Ok(Some(Step::Opening(Opening {
            view,
            ticket: self.tickets.issue(),
            stage,
        })))
    }
    fn retire_floating_view(&mut self) -> Option<usize> {
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
        })?;
        self.retire(old);
        Some(old)
    }
}
