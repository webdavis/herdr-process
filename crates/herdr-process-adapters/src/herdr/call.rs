use super::reply::{Expected, HostFailure, HostFailureCode, HostReply};
use std::{
    io::{self, Read},
    os::fd::AsRawFd,
    process::{Child, ChildStderr, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};
/// How long the host is given to answer before the call is refused. It is a
/// PRODUCT decision: somebody is waiting on a pane to open. `pub(super)` so a
/// test can assert a freshly opened call still carries this value rather than
/// one an edit silently swapped in underneath it.
pub(super) const DEADLINE: Duration = Duration::from_millis(500);
const OUTPUT_LIMIT: usize = 65536;
const READ_BUDGET: usize = 8192;

pub struct HostCall {
    child: Child,
    stdout: Option<ChildStdout>,
    stderr: Option<ChildStderr>,
    output: Vec<u8>,
    errors: Vec<u8>,
    started: Instant,
    /// This call's own copy of `DEADLINE`. A TEST OWNS ITS OWN BOUND: the
    /// suite's doubles are spawned shells, and on a loaded machine the spawn
    /// alone can outlast the product's half second, which turned a test about
    /// a malformed reply into a test about how fast this machine forks.
    deadline: Duration,
    expected: Expected,
    outcome: Option<Result<HostReply, HostFailure>>,
}

impl HostCall {
    pub(super) fn spawn(mut command: Command, expected: Expected) -> Result<Self, HostFailure> {
        let started = Instant::now();
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| HostFailure::new(HostFailureCode::Spawn))?;
        let mut call = Self {
            child,
            stdout: None,
            stderr: None,
            output: Vec::new(),
            errors: Vec::new(),
            started,
            deadline: DEADLINE,
            expected,
            outcome: None,
        };
        call.stdout = call.child.stdout.take();
        call.stderr = call.child.stderr.take();
        let stdout = call
            .stdout
            .as_ref()
            .ok_or_else(|| HostFailure::new(HostFailureCode::Io))?;
        let stderr = call
            .stderr
            .as_ref()
            .ok_or_else(|| HostFailure::new(HostFailureCode::Io))?;
        nonblocking(stdout)
            .and_then(|()| nonblocking(stderr))
            .map_err(|_| HostFailure::new(HostFailureCode::Io))?;
        Ok(call)
    }

    pub fn poll(&mut self) -> Result<Option<HostReply>, HostFailure> {
        if let Some(outcome) = &self.outcome {
            return outcome.clone().map(Some);
        }
        let result = self.advance();
        if !matches!(result, Ok(None)) {
            self.outcome = Some(match &result {
                Ok(Some(reply)) => Ok(reply.clone()),
                Err(error) => Err(error.clone()),
                Ok(None) => unreachable!(),
            });
            // Error outcomes stop the exact child immediately; Drop performs the reap.
            if result.is_err() {
                let _ = self.child.kill();
            }
            self.stdout = None;
            self.stderr = None;
            self.output.clear();
            self.errors.clear();
        }
        result
    }

    /// Move this call's bound, so a test can wait on the signal its double
    /// actually raises rather than race the product's deadline.
    #[cfg(test)]
    pub(super) fn bound(&mut self, deadline: Duration) {
        self.deadline = deadline;
    }

    /// This call's current bound. TEST-ONLY: lets a test pin the initializer
    /// to `DEADLINE` without reading a wall clock.
    #[cfg(test)]
    pub(super) fn deadline(&self) -> Duration {
        self.deadline
    }

    fn advance(&mut self) -> Result<Option<HostReply>, HostFailure> {
        if self.started.elapsed() >= self.deadline {
            return Err(HostFailure::new(HostFailureCode::Timeout));
        }
        read_pipe(&mut self.stdout, &mut self.output, self.errors.len())?;
        read_pipe(&mut self.stderr, &mut self.errors, self.output.len())?;
        let status = self
            .child
            .try_wait()
            .map_err(|_| HostFailure::new(HostFailureCode::Io))?;
        let Some(status) = status else {
            return Ok(None);
        };
        if self.stdout.is_some() || self.stderr.is_some() {
            return Ok(None);
        }
        if !status.success() || (self.output.is_empty() && !self.errors.is_empty()) {
            return Err(HostFailure::new(HostFailureCode::Exit));
        }
        // A structured API failure is authoritative even if stderr also contains diagnostics.
        let reply = self.expected.decode(&self.output)?;
        if !self.errors.is_empty() {
            return Err(HostFailure::new(HostFailureCode::Exit));
        }
        Ok(Some(reply))
    }
}

impl Drop for HostCall {
    fn drop(&mut self) {
        // Child caches a reaped status, so kill cannot target a reused process ID.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    // SAFETY: the owned pipe remains alive throughout both descriptor operations.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: preserve the existing flags and only add nonblocking mode on this pipe.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn read_pipe(
    pipe: &mut Option<impl Read>,
    bytes: &mut Vec<u8>,
    other: usize,
) -> Result<(), HostFailure> {
    let Some(reader) = pipe.as_mut() else {
        return Ok(());
    };
    let mut buffer = [0; READ_BUDGET];
    let remaining = OUTPUT_LIMIT.saturating_sub(bytes.len() + other);
    let size = buffer.len().min(remaining + 1);
    match reader.read(&mut buffer[..size]) {
        Ok(0) => *pipe = None,
        Ok(count) => {
            if count > remaining {
                return Err(HostFailure::new(HostFailureCode::Oversized));
            }
            bytes.extend_from_slice(&buffer[..count]);
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) => {}
        Err(_) => return Err(HostFailure::new(HostFailureCode::Io)),
    }
    Ok(())
}
