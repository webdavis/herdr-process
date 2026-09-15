use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, BorrowedFd},
    time::{Duration, Instant},
};

const RESTORE: &[u8] = b"\x1b[0m\x1b[?25h\x1b[?1l\x1b>\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1005l\x1b[?1006l\x1b[?2004l\x1b[?1049l";

pub struct AttachmentTerminal {
    input: File,
    output: File,
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
        let mut terminal = Self {
            input,
            output,
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
        Ok(terminal)
    }

    pub fn size(&self) -> io::Result<(u16, u16)> {
        let mut size: libc::winsize = unsafe { std::mem::zeroed() };
        check(unsafe { libc::ioctl(self.input.as_raw_fd(), libc::TIOCGWINSZ, &mut size) })?;
        Ok((size.ws_row.max(1), size.ws_col.max(1)))
    }
    pub fn read(&mut self) -> io::Result<Option<Vec<u8>>> {
        let mut buffer = [0; 8192];
        match self.input.read(&mut buffer) {
            Ok(0) => Ok(None),
            Ok(count) => Ok(Some(buffer[..count].to_vec())),
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                Ok(Some(Vec::new()))
            }
            Err(error) => Err(error),
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
            match self.output.write(bytes) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "terminal output closed",
                    ));
                }
                Ok(count) => bytes = &bytes[count..],
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    let mut poll = libc::pollfd {
                        fd: self.output.as_raw_fd(),
                        events: libc::POLLOUT,
                        revents: 0,
                    };
                    let result =
                        unsafe { libc::poll(&mut poll, 1, remaining.as_millis().max(1) as i32) };
                    if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted
                    {
                        return Err(io::Error::last_os_error());
                    }
                }
                Err(error) => return Err(error),
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

fn check(result: i32) -> io::Result<i32> {
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
