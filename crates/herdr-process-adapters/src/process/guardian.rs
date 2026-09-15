use super::{
    control::{Control, Message},
    launch::Launch,
    native::{TerminalJobs, reap},
};
use anyhow::{Result, bail, ensure};
use std::{
    os::unix::{io::AsRawFd, net::UnixStream},
    time::{Duration, Instant},
};

pub fn supervise(args: &[String]) -> Result<i32> {
    ensure!(
        args.len() == 2,
        "supervise requires private socket and capability"
    );
    let stream = UnixStream::connect(&args[0])?;
    let directory = super::directory::Directory::connected(std::path::Path::new(&args[0]))?;
    let mut control = Control::new(stream)?;
    control.send(Message::Hello {
        secret: args[1].clone(),
        pid: std::process::id(),
    })?;
    let Message::Start(spec) = control.until(Instant::now() + Duration::from_millis(300))? else {
        bail!("expected guardian start");
    };
    // This entry point must run before the helper creates any threads. Do not call
    // it inside the manager. The CLI dispatches directly to it in a fresh process.
    for (name, _) in std::env::vars_os() {
        if super::startup::private_variable(&name) {
            unsafe { std::env::remove_var(name) };
        }
    }
    unsafe { std::env::set_var("TERM", "xterm-256color") };
    unsafe {
        for signal in [
            libc::SIGHUP,
            libc::SIGINT,
            libc::SIGQUIT,
            libc::SIGTERM,
            libc::SIGTTOU,
            libc::SIGTTIN,
            libc::SIGPIPE,
        ] {
            libc::signal(signal, libc::SIG_IGN);
        }
    }
    let jobs = TerminalJobs::current()?;
    let mut child = Launch::fork(&spec, control.stream.as_raw_fd())?;
    let mut guard = Cleanup {
        jobs,
        pid: child.pid,
        status: None,
        attempted: false,
    };
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
        child.release()?;
        control.send(Message::Ready(child.pid as u32))?;
        loop {
            reap(child.pid, &mut guard.status)?;
            if guard.status.is_some() {
                return Ok(());
            }
            match control.receive() {
                Ok(Some(Message::Stop)) => return Ok(()),
                Ok(None) => (),
                Ok(Some(_)) => bail!("unexpected guardian request"),
                Err(_) => return Ok(()), // EOF and malformed control both revoke ownership input.
            }
            super::control::wait(
                control.stream.as_raw_fd(),
                libc::POLLIN,
                Instant::now() + Duration::from_millis(10),
            )?;
        }
    }))
    .unwrap_or_else(|_| Err(anyhow::anyhow!("guardian panicked")));
    let cleanup = guard.finish().and(directory.remove());
    if let Err(error) = cleanup.and(run) {
        let _ = control.send(Message::Failure(format!("{error:#}")));
        return Err(error);
    }
    let status = guard
        .status
        .ok_or_else(|| anyhow::anyhow!("configured command was not reaped"))?;
    let _ = control.send(Message::Exit(status));
    Ok(0)
}
struct Cleanup {
    jobs: TerminalJobs,
    pid: i32,
    status: Option<i32>,
    attempted: bool,
}
impl Cleanup {
    fn finish(&mut self) -> Result<()> {
        self.attempted = true;
        self.jobs.cleanup(self.pid, &mut self.status)
    }
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        if !self.attempted
            && let Err(e) = self.finish()
        {
            eprintln!("guardian cleanup incomplete: {e:#}");
        }
    }
}
