use super::*;
use adapters::{ProcessSpec, PtySize, SupervisedProcess, Terminal};
pub(super) struct Session {
    pub process: SupervisedProcess,
    pub terminal: Terminal,
    pub view: Option<(usize, Placement)>,
    pub context: Target,
    input: VecDeque<u8>,
}
impl Session {
    pub fn spawn(binary: &std::path::Path, profile: &adapters::Profile) -> anyhow::Result<Self> {
        let process = SupervisedProcess::spawn(
            binary,
            &ProcessSpec {
                program: profile.program.clone(),
                args: profile.args.clone(),
                cwd: profile.cwd.clone(),
            },
            size(24, 80),
        )?;
        Ok(Self {
            process,
            terminal: Terminal::new(24, 80),
            view: None,
            context: Target {
                workspace: String::new(),
                pane: String::new(),
            },
            input: VecDeque::new(),
        })
    }
    pub fn context_for(&self, target: &Target) -> Target {
        if let Some((
            _,
            Placement::Docked {
                target: anchor,
                pane,
                ..
            },
        )) = &self.view
            && target.workspace == anchor.workspace
            && target.pane == *pane
        {
            return anchor.clone();
        }
        if target.workspace.is_empty() || target.pane.is_empty() {
            self.context.clone()
        } else {
            target.clone()
        }
    }
    pub fn preview(&self, rows: u16, cols: u16) -> Vec<u8> {
        let mut preview = Terminal::new(rows, cols);
        preview.feed(&self.terminal.snapshot());
        preview.snapshot()
    }
    pub fn resize(&mut self, rows: u16, cols: u16) -> anyhow::Result<()> {
        validate_size(rows, cols)?;
        self.process.resize(size(rows, cols))?;
        self.terminal.resize(rows, cols);
        Ok(())
    }
    pub fn input(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.input.len() + bytes.len() <= 1024 * 1024,
            "child input queue is full"
        );
        self.input.extend(bytes);
        Ok(())
    }
    fn drain(&mut self) -> anyhow::Result<(bool, bool)> {
        let bytes = self.process.read_available()?;
        let mut changed = !bytes.is_empty();
        self.terminal.feed(&bytes);
        let exited = self.process.poll_exit()?.is_some();
        if exited {
            let tail = self.process.read_available()?;
            changed |= !tail.is_empty();
            self.terminal.feed(&tail);
        } else {
            let responses = self.terminal.take_responses();
            self.input(&responses)?;
            if !self.input.is_empty() {
                match self.process.write(self.input.make_contiguous()) {
                    Ok(count) => {
                        self.input.drain(..count);
                    }
                    Err(error)
                        if error.downcast_ref::<std::io::Error>().is_some_and(|e| {
                            matches!(
                                e.kind(),
                                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                            )
                        }) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Ok((changed, exited))
    }
}
fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}
impl Manager {
    pub(super) fn poll_sessions(&mut self) -> anyhow::Result<()> {
        let mut exited = Vec::new();
        for (name, session) in &mut self.sessions {
            let (changed, done) = session.drain()?;
            if changed {
                for client in self.clients.values_mut() {
                    if client.role == Role::Active(name.clone()) {
                        client.screen = Some(session.terminal.snapshot());
                    } else if client.role == Role::Candidate(name.clone())
                        && let Some((rows, cols)) = self.pending.as_ref().and_then(|p| p.dimensions)
                    {
                        client.screen = Some(session.preview(rows, cols));
                    }
                }
            }
            if done {
                exited.push(name.clone());
            }
        }
        for name in exited {
            if let Some(session) = self.sessions.remove(&name)
                && let Some((id, _)) = session.view
            {
                self.retire(id);
            }
        }
        Ok(())
    }
}

pub(super) fn validate_size(rows: u16, cols: u16) -> anyhow::Result<()> {
    anyhow::ensure!(
        rows > 0 && cols > 0 && u32::from(rows) * u32::from(cols) <= 262144,
        "invalid terminal size"
    );
    Ok(())
}
