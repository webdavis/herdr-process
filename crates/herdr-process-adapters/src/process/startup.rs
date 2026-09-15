use super::{
    ProcessSpec,
    control::{Control, Message},
    directory::Directory,
};
use anyhow::{Result, bail, ensure};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    ffi::OsStr,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, net::UnixListener},
    },
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn private_variable(name: &OsStr) -> bool {
    let name = name.as_bytes();
    [b"HERDR_".as_slice(), b"GUARDIAN_", b"ATTACHMENT_"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}
pub(super) struct Startup {
    started: bool,
    pub child: Box<dyn Child + Send + Sync>,
    pub master: Box<dyn MasterPty + Send>,
    pub reader: Box<dyn Read + Send>,
    pub tail: Vec<u8>,
    pub writer: Box<dyn Write + Send>,
    pub control: Option<Control>,
    pub _directory: Directory,
}
impl Startup {
    pub fn create(binary: &Path, size: PtySize) -> Result<Self> {
        let directory = Directory::create()?;
        let socket = directory.0.join("s");
        let listener = UnixListener::bind(&socket)?;
        listener.set_nonblocking(true)?;
        let mut random = [0u8; 16];
        unsafe { libc::arc4random_buf(random.as_mut_ptr().cast(), random.len()) };
        let secret: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let pair = native_pty_system().openpty(size)?;
        let fd = pair
            .master
            .as_raw_fd()
            .ok_or_else(|| anyhow::anyhow!("native PTY descriptor unavailable"))?;
        // Cloned portable-pty descriptors share this nonblocking file description.
        ensure!(
            unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } == 0,
            "nonblocking PTY: {}",
            std::io::Error::last_os_error()
        );
        let reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let mut command = CommandBuilder::new(binary);
        command.arg("supervise");
        command.arg(&socket);
        command.arg(&secret);
        command.env("TERM", "xterm-256color");
        for (name, _) in std::env::vars_os() {
            if private_variable(&name) {
                command.env_remove(name);
            }
        }
        let child = pair.slave.spawn_command(command)?;
        drop(pair.slave);
        let mut startup = Self {
            started: false,
            child,
            master: pair.master,
            reader,
            tail: Vec::new(),
            writer,
            control: None,
            _directory: directory,
        };
        let end = Instant::now() + Duration::from_millis(300);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    let mut control = Control::new(stream)?;
                    let hello = control.until(end)?;
                    ensure!(
                        matches!(hello,Message::Hello{secret:ref offered,pid} if offered==&secret && Some(pid)==startup.child.process_id()),
                        "guardian authentication failed"
                    );
                    startup.control = Some(control);
                    return Ok(startup);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    ensure!(
                        startup.child.try_wait()?.is_none(),
                        "guardian exited before authentication"
                    );
                    super::control::wait(listener.as_raw_fd(), libc::POLLIN, end)?;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    pub fn start(&mut self, spec: &ProcessSpec) -> Result<u32> {
        let control = self
            .control
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("missing guardian control"))?;
        self.started = true;
        control.send(Message::Start(spec.clone()))?;
        match control.until(Instant::now() + Duration::from_millis(400))? {
            Message::Ready(pid) => Ok(pid),
            Message::Failure(message) => bail!(message),
            _ => bail!("expected configured command readiness"),
        }
    }
    pub fn reap(&mut self, end: Instant) -> Result<portable_pty::ExitStatus> {
        loop {
            let mut bytes = [0; 8192];
            match self.reader.read(&mut bytes) {
                Ok(n) => {
                    ensure!(
                        self.tail.len() + n <= 1048576,
                        "final terminal output exceeds retained capacity"
                    );
                    self.tail.extend_from_slice(&bytes[..n]);
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(e) => return Err(e.into()),
            }
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            super::control::wait(-1, 0, end).map_err(|e| anyhow::anyhow!("guardian reap: {e}"))?;
        }
    }
}
impl Drop for Startup {
    fn drop(&mut self) {
        self.control.take();
        if !self.started
            && let Err(e) = super::native::abort_unstarted(self.child.as_mut())
        {
            eprintln!("unstarted guardian cleanup incomplete: {e:#}");
        }
        // EOF takes the guardian's normal terminal cleanup path.
        if let Err(e) = self.reap(Instant::now() + Duration::from_millis(600)) {
            eprintln!("guardian reaping incomplete: {e:#}");
        }
    }
}
