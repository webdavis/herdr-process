use super::*;
mod actions;
mod opening;
#[derive(Clone, Copy)]
pub(super) enum Origin {
    Controller(usize),
    Attachment(usize),
}
pub(super) struct Command {
    pub origin: Origin,
    pub profile: String,
    pub action: Action,
    pub target: Target,
}
pub(super) struct Transition<P: Ports> {
    pub command: Command,
    pub deadline: Instant,
    pub step: Step<P>,
}
pub(super) enum Step<P: Ports> {
    Focusing(P::Call),
    Opening(Opening<P>),
}
pub(super) struct Opening<P: Ports> {
    pub view: Open,
    pub ticket: String,
    pub stage: Stage<P>,
}
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Open {
    Float,
    Split(Direction),
}
pub(super) enum Stage<P: Ports> {
    Retiring(usize),
    RetryAt(Instant),
    Calling {
        call: P::Call,
        candidate: Option<Candidate>,
    },
    Opened {
        placement: Placement,
        candidate: Option<Candidate>,
    },
}
#[derive(Clone, Copy)]
pub(super) struct Candidate {
    pub id: usize,
    pub rows: u16,
    pub cols: u16,
    pub ready: bool,
}
impl<P: Ports> Opening<P> {
    pub fn candidate(&self) -> Option<&Candidate> {
        match &self.stage {
            Stage::Calling { candidate, .. } | Stage::Opened { candidate, .. } => {
                candidate.as_ref()
            }
            _ => None,
        }
    }
    pub fn candidate_slot(&mut self) -> Option<&mut Option<Candidate>> {
        match &mut self.stage {
            Stage::Calling { candidate, .. } | Stage::Opened { candidate, .. } => Some(candidate),
            _ => None,
        }
    }
}
impl<P: Ports> Transition<P> {
    pub fn candidate(&self) -> Option<&Candidate> {
        match &self.step {
            Step::Opening(opening) => opening.candidate(),
            Step::Focusing(_) => None,
        }
    }
    pub fn candidate_mut(&mut self) -> Option<&mut Candidate> {
        match &mut self.step {
            Step::Opening(opening) => opening.candidate_slot()?.as_mut(),
            Step::Focusing(_) => None,
        }
    }
}
impl<P: Ports> Manager<P> {
    pub(super) fn poll_transition(&mut self) {
        let Some(mut transition) = self.pending.take() else {
            return;
        };
        match self.advance(&mut transition) {
            Ok(false) => self.pending = Some(transition),
            Ok(true) => self.finish(&transition.command, Ok(())),
            Err(error) => {
                if let Some(id) = transition.candidate().map(|c| c.id) {
                    self.retire(id);
                }
                if let Some(session) = self.sessions.get(&transition.command.profile)
                    && let Some((id, _)) = session.view
                    && let Some(client) = self.clients.get_mut(&id)
                {
                    client.screen = Some(session.terminal.snapshot());
                }
                self.finish(&transition.command, Err(error));
            }
        }
    }
    pub(super) fn finish(&mut self, command: &Command, result: anyhow::Result<()>) {
        self.completed = true;
        let error = |error| Response::Error {
            message: format!("{} {}: {error}", command.profile, command.action),
        };
        match (command.origin, result) {
            (Origin::Controller(id), Ok(())) => self.send(id, Response::Ack {}),
            (Origin::Controller(id), Err(e)) | (Origin::Attachment(id), Err(e)) => {
                self.send(id, error(e))
            }
            (Origin::Attachment(_), Ok(())) => {}
        }
    }
}
