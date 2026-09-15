use super::*;
pub(super) enum Work {
    Command(Command),
    Effect { source: usize, effect: Effect },
}
impl Manager {
    pub(super) fn poll_work(&mut self) {
        for _ in 0..128 {
            let input_can_progress = match self.queue.front() {
                Some(Work::Effect {
                    effect: Effect::Forward(_),
                    ..
                }) => true,
                Some(Work::Effect {
                    source,
                    effect: Effect::Interrupt,
                }) => self.docked(*source),
                _ => false,
            };
            if self.pending.is_some() && !input_can_progress {
                return;
            }
            let Some(work) = self.queue.pop_front() else {
                return;
            };
            self.execute_work(work);
            if !input_can_progress {
                return;
            }
        }
    }
    fn docked(&self, id: usize) -> bool {
        let Some(Role::Active(profile)) = self.clients.get(&id).map(|c| &c.role) else {
            return false;
        };
        self.sessions
            .get(profile)
            .and_then(|s| s.view.as_ref())
            .is_some_and(|v| matches!(v.1, Placement::Docked { .. }))
    }

    pub(super) fn effects(&mut self, id: usize, effects: Vec<Effect>) {
        if self.queue.len() + effects.len() > 128 {
            self.send(
                id,
                Response::Error {
                    message: "input work queue is full; reconnect".into(),
                },
            );
            self.retire(id);
            return;
        }
        self.queue.extend(
            effects
                .into_iter()
                .map(|effect| Work::Effect { source: id, effect }),
        );
    }
    pub(super) fn execute_work(&mut self, work: Work) {
        let (id, effect) = match work {
            Work::Command(command) => {
                self.execute(command);
                return;
            }
            Work::Effect { source, effect } => (source, effect),
        };
        let Some(Role::Active(profile)) = self.clients.get(&id).map(|c| c.role.clone()) else {
            return;
        };
        let Some(session) = self.sessions.get_mut(&profile) else {
            return;
        };
        let target = match session.view.as_ref().map(|v| &v.1) {
            Some(Placement::Docked { target, pane, .. }) => Target {
                workspace: target.workspace.clone(),
                pane: pane.clone(),
            },
            _ => session.context.clone(),
        };
        let result = match effect {
            Effect::Forward(bytes) => session.input(&bytes),
            Effect::Interrupt
                if session
                    .view
                    .as_ref()
                    .is_some_and(|v| v.1 == Placement::Floating) =>
            {
                let action =
                    if self.configuration.profiles()[&profile].ctrl_c == adapters::CtrlC::Hide {
                        Action::ToggleFloat
                    } else {
                        Action::Kill
                    };
                self.execute(Command {
                    reply: None,
                    profile,
                    action,
                    target,
                });
                Ok(())
            }
            Effect::Interrupt => session.input(&[3]),
            Effect::Invoke { profile, action } => {
                self.execute(Command {
                    reply: None,
                    profile,
                    action,
                    target,
                });
                Ok(())
            }
        };
        if let Err(error) = result {
            self.send(
                id,
                Response::Error {
                    message: error.to_string(),
                },
            );
            self.retire(id);
        }
    }
}
