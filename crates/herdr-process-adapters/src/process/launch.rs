use super::{ProcessSpec, control};
use anyhow::{Result, ensure};
use std::{
    ffi::CString,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
    time::{Duration, Instant},
};

pub(super) struct Launch {
    pub pid: i32,
    barrier: std::fs::File,
    errors: std::fs::File,
}
fn pipe() -> Result<[OwnedFd; 2]> {
    let mut fds = [-1; 2];
    // Both descriptors become owned immediately after successful pipe creation.
    ensure!(
        unsafe { libc::pipe(fds.as_mut_ptr()) } == 0,
        "pipe: {}",
        std::io::Error::last_os_error()
    );
    let pair = unsafe { [OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])] };
    for fd in &pair {
        ensure!(
            unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
            "pipe close-on-exec"
        );
    }
    Ok(pair)
}
impl Launch {
    pub fn fork(spec: &ProcessSpec, control_fd: i32) -> Result<Self> {
        let argv: Vec<CString> = std::iter::once(&spec.program)
            .chain(spec.args.iter())
            .map(|s| CString::new(s.as_bytes()))
            .collect::<std::result::Result<_, _>>()?;
        let mut pointers: Vec<*const libc::c_char> = argv.iter().map(|s| s.as_ptr()).collect();
        pointers.push(std::ptr::null());
        let cwd = CString::new(spec.cwd.as_os_str().as_bytes())?;
        let [barrier_read, barrier_write] = pipe()?;
        let [error_read, error_write] = pipe()?;
        // supervise is an executable-only, single-threaded entry point. Child uses
        // only native operations and _exit, never Rust unwinding after this fork.
        let pid = unsafe { libc::fork() };
        ensure!(pid >= 0, "fork: {}", std::io::Error::last_os_error());
        if pid == 0 {
            unsafe {
                libc::close(control_fd);
                libc::close(barrier_write.as_raw_fd());
                libc::close(error_read.as_raw_fd());
                let mut ready = 0u8;
                if libc::setpgid(0, 0) < 0
                    || libc::read(barrier_read.as_raw_fd(), (&mut ready as *mut u8).cast(), 1) != 1
                {
                    libc::_exit(125);
                }
                libc::close(barrier_read.as_raw_fd());
                for signal in 1..32 {
                    if signal != libc::SIGKILL && signal != libc::SIGSTOP {
                        libc::signal(signal, libc::SIG_DFL);
                    }
                }
                let empty: libc::sigset_t = std::mem::zeroed();
                libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
                if libc::chdir(cwd.as_ptr()) == 0 {
                    libc::execvp(argv[0].as_ptr(), pointers.as_ptr());
                }
                let errno = *libc::__error();
                libc::write(error_write.as_raw_fd(), (&errno as *const i32).cast(), 4);
                libc::_exit(127);
            }
        }
        drop(barrier_read);
        drop(error_write);
        Ok(Self {
            pid,
            barrier: barrier_write.into(),
            errors: error_read.into(),
        })
    }
    pub fn release(&mut self) -> Result<()> {
        // The child is blocked before exec, so its group cannot disappear here.
        ensure!(
            unsafe { libc::setpgid(self.pid, self.pid) } == 0,
            "configured command group setup"
        );
        ensure!(
            unsafe { libc::tcsetpgrp(0, self.pid) } == 0,
            "configured command foreground setup"
        );
        self.barrier.write_all(&[1])?;
        let fd = self.errors.as_raw_fd();
        ensure!(
            unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } == 0,
            "exec pipe nonblocking"
        );
        let end = Instant::now() + Duration::from_millis(200);
        let mut bytes = Vec::new();
        loop {
            let mut b = [0; 4];
            match self.errors.read(&mut b) {
                Ok(0) => {
                    ensure!(
                        bytes.is_empty(),
                        "configured command exec failed: {bytes:?}"
                    );
                    return Ok(());
                }
                Ok(n) => {
                    bytes.extend_from_slice(&b[..n]);
                    ensure!(
                        bytes.len() < 4,
                        "configured command exec failed: {}",
                        std::io::Error::from_raw_os_error(i32::from_ne_bytes(
                            bytes[..4].try_into()?
                        ))
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    control::wait(fd, libc::POLLIN, end)?
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}
