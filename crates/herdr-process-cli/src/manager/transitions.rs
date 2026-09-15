use super::*;
use adapters::{HostCall, HostReply, OpenView};
use std::io::Read;
mod actions;
pub(super) struct Command {
    pub reply: Option<usize>,
    pub profile: String,
    pub action: Action,
    pub target: Target,
}
pub(super) struct Pending {
    pub command: Command,
    pub ticket: String,
    pub candidate: Option<usize>,
    pub ready: bool,
    pub dimensions: Option<(u16, u16)>,
    pub call: Option<HostCall>,
    pub opened: Option<Placement>,
    pub intent: ViewIntent,
    pub deadline: Instant,
    pub waiting: Option<usize>,
    pub retry: Instant,
}
impl Manager {
    pub(super) fn poll_transition(&mut self) {
        let Some(mut pending) = self.pending.take() else {
            return;
        };
        let result = self.advance(&mut pending);
        match result {
            Ok(false) => self.pending = Some(pending),
            Ok(true) => self.finish(&pending.command, Ok(())),
            Err(error) => {
                if let Some(id) = pending.candidate {
                    self.retire(id);
                }
                if let Some(session) = self.sessions.get(&pending.command.profile)
                    && let Some((id, _)) = session.view
                    && let Some(client) = self.clients.get_mut(&id)
                {
                    client.screen = Some(session.terminal.snapshot());
                }
                self.finish(&pending.command, Err(error));
            }
        }
    }
    fn advance(&mut self, pending: &mut Pending) -> anyhow::Result<bool> {
        anyhow::ensure!(
            self.sessions.contains_key(&pending.command.profile),
            "session exited during view replacement; retry to start again"
        );
        anyhow::ensure!(
            Instant::now() < pending.deadline,
            "view replacement timed out; retry"
        );
        if let Some(id) = pending.candidate {
            anyhow::ensure!(
                self.clients
                    .get(&id)
                    .is_some_and(|c| matches!(c.role, Role::Candidate(_))),
                "candidate disconnected; retry"
            );
        }
        if let Some(id) = pending.waiting {
            if self.clients.contains_key(&id) {
                return Ok(false);
            }
            anyhow::bail!("retired attachment did not disconnect; retry");
        }
        if pending.call.is_none() && pending.opened.is_none() {
            if Instant::now() < pending.retry {
                return Ok(false);
            }
            let profile = &self.configuration.profiles()[&pending.command.profile];
            let split = if let ViewIntent::OpenSplit(direction) = pending.intent {
                Some((pending.command.target.clone(), direction))
            } else {
                None
            };
            pending.call = Some(self.host.open(&OpenView {
                profile: pending.command.profile.clone(),
                ticket: pending.ticket.clone(),
                manager_socket: self.socket.clone(),
                cwd: profile.cwd.clone(),
                width: profile.width,
                height: profile.height,
                split,
            })?);
        }
        if let Some(call) = &mut pending.call {
            match call.poll() {
                Ok(None) => return Ok(false),
                Ok(Some(HostReply::Focused)) => return Ok(true),
                Ok(Some(HostReply::Opened(pane))) => {
                    pending.opened = Some(match pending.intent {
                        ViewIntent::OpenSplit(direction) => Placement::Docked {
                            target: pending.command.target.clone(),
                            direction,
                            pane: pane
                                .ok_or_else(|| anyhow::anyhow!("split did not return a pane"))?,
                        },
                        _ => Placement::Floating,
                    });
                    pending.call = None;
                }
                Err(error) if error.is_busy() && pending.intent == ViewIntent::OpenFloat => {
                    if let Some(id) = pending.candidate.take() {
                        self.retire(id);
                    }
                    pending.ready = false;
                    pending.ticket = ticket()?;
                    pending.call = None;
                    pending.retry = Instant::now() + Duration::from_millis(10);
                    return Ok(false);
                }
                Err(error) => return Err(error.into()),
            }
        }
        if pending.ready
            && let Some(mut placement) = pending.opened.take()
        {
            let id = pending
                .candidate
                .ok_or_else(|| anyhow::anyhow!("candidate missing"))?;
            let session = self.sessions.get_mut(&pending.command.profile).unwrap();
            let (rows, cols) = pending
                .dimensions
                .ok_or_else(|| anyhow::anyhow!("candidate size missing"))?;
            session.resize(rows, cols)?;
            let screen = session.terminal.snapshot();
            session.context = session.context_for(&pending.command.target);
            if let Placement::Docked { target, .. } = &mut placement {
                *target = session.context.clone();
            }
            let old = session.view.replace((id, placement));
            let client = self.clients.get_mut(&id).unwrap();
            client.role = Role::Active(pending.command.profile.clone());
            client.screen = None;
            self.send(id, Response::Screen { bytes: screen });
            self.send(id, Response::Ack {});
            if let Some((old, _)) = old {
                self.retire(old);
            }
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) fn finish(&mut self, command: &Command, result: anyhow::Result<()>) {
        self.completed = true;
        let response = match result {
            Ok(()) => Response::Ack {},
            Err(error) => Response::Error {
                message: format!("{} {}: {error}", command.profile, command.action),
            },
        };
        if let Some(id) = command.reply {
            self.send(id, response);
        } else if let Response::Error { message } = response {
            eprintln!("{message}");
        }
    }
}
fn ticket() -> anyhow::Result<String> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
