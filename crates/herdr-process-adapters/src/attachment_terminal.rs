use crate::{Readiness, unix};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, BorrowedFd, FromRawFd},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant},
};

const RESTORE: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1l\x1b>\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?2004l\x1b[?1049l";

static RESIZE_SIGNAL: AtomicI32 = AtomicI32::new(-1);

extern "C" fn on_resize(_: libc::c_int) {
    let fd = RESIZE_SIGNAL.load(Ordering::Relaxed);
    if fd >= 0 {
        unsafe { libc::write(fd, [0u8].as_ptr().cast(), 1) };
    }
}

pub struct AttachmentTerminal {
    input: File,
    output: File,
    resizes: File,
    _resize_signal: File,
    original: libc::termios,
    input_flags: i32,
    output_flags: i32,
    active: bool,
}

impl AttachmentTerminal {
    pub fn enter(input: BorrowedFd<'_>, output: BorrowedFd<'_>) -> io::Result<Self> {
        let input = File::from(input.try_clone_to_owned()?);
        let output = File::from(output.try_clone_to_owned()?);
        let mut original = unsafe { std::mem::zeroed() };
        // Both descriptors remain owned for every native call and restoration.
        check(unsafe { libc::tcgetattr(input.as_raw_fd(), &mut original) })?;
        let input_flags = check(unsafe { libc::fcntl(input.as_raw_fd(), libc::F_GETFL) })?;
        let output_flags = check(unsafe { libc::fcntl(output.as_raw_fd(), libc::F_GETFL) })?;
        let (resizes, resize_signal) = resize_pipe()?;
        let mut terminal = Self {
            input,
            output,
            resizes,
            _resize_signal: resize_signal,
            original,
            input_flags,
            output_flags,
            active: true,
        };
        let mut raw = original;
        unsafe { libc::cfmakeraw(&mut raw) };
        check(unsafe { libc::tcsetattr(terminal.input.as_raw_fd(), libc::TCSANOW, &raw) })?;
        terminal.set_flags(
            input_flags | libc::O_NONBLOCK,
            output_flags | libc::O_NONBLOCK,
        )?;
        terminal.control(b"\x1b[?1049h")?;
        RESIZE_SIGNAL.store(terminal._resize_signal.as_raw_fd(), Ordering::Relaxed);
        unsafe { libc::signal(libc::SIGWINCH, on_resize as *const () as libc::sighandler_t) };
        Ok(terminal)
    }

    pub fn watch(&self, readiness: &mut Readiness, input: bool, output: bool) {
        if input {
            readiness.readable(self.input.as_raw_fd());
            readiness.readable(self.resizes.as_raw_fd());
        }
        if output {
            readiness.writable(self.output.as_raw_fd());
        }
    }

    pub fn size(&self) -> io::Result<(u16, u16)> {
        let mut signals = [0; 64];
        while unix::ready((&self.resizes).read(&mut signals))?.is_some_and(|n| n == signals.len()) {
        }
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        check(unsafe { libc::ioctl(self.input.as_raw_fd(), libc::TIOCGWINSZ, &mut size) })?;
        Ok((size.ws_row.max(1), size.ws_col.max(1)))
    }
    pub fn read(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut buffer = [0; 8192];
        match unix::ready(self.input.read(&mut buffer))? {
            Some(0) => Ok(None),
            Some(count) => Ok(Some(buffer[..count].to_vec())),
            None => Ok(Some(Vec::new())),
        }
    }
    pub fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.output.write(bytes)
    }
    pub fn finish(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        unsafe { libc::signal(libc::SIGWINCH, libc::SIG_DFL) };
        RESIZE_SIGNAL.store(-1, Ordering::Relaxed);
        let screen = self.control(RESTORE);
        let attributes = check(unsafe {
            libc::tcsetattr(self.input.as_raw_fd(), libc::TCSANOW, &self.original)
        });
        let flags = self.set_flags(self.input_flags, self.output_flags);
        screen.and(attributes).and(flags)
    }
    fn set_flags(&self, input: i32, output: i32) -> io::Result<()> {
        let first = check(unsafe { libc::fcntl(self.input.as_raw_fd(), libc::F_SETFL, input) });
        let second = check(unsafe { libc::fcntl(self.output.as_raw_fd(), libc::F_SETFL, output) });
        first.and(second).map(|_| ())
    }
    fn control(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_millis(50);
        while !bytes.is_empty() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "terminal restoration output timed out",
                ));
            }
            match unix::ready(self.output.write(bytes))? {
                Some(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "terminal output closed",
                    ));
                }
                Some(count) => bytes = &bytes[count..],
                None => {
                    let mut readiness = Readiness::default();
                    readiness.writable(self.output.as_raw_fd());
                    readiness.wait(Some(remaining))?;
                }
            }
        }
        Ok(())
    }
}

impl Drop for AttachmentTerminal {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn resize_pipe() -> io::Result<(File, File)> {
    let mut fds = [0; 2];
    check(unsafe { libc::pipe(fds.as_mut_ptr()) })?;
    let pipe = unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) };
    for end in [&pipe.0, &pipe.1] {
        let flags = check(unsafe { libc::fcntl(end.as_raw_fd(), libc::F_GETFL) })?;
        check(unsafe { libc::fcntl(end.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) })?;
        check(unsafe { libc::fcntl(end.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) })?;
    }
    Ok(pipe)
}

fn check(result: i32) -> io::Result<i32> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
