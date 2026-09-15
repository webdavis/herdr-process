use super::*;
use std::{
    io::{Read, Write},
    os::fd::{AsFd, AsRawFd, FromRawFd},
    time::{Duration, Instant},
};

struct Pty {
    master: File,
    slave: File,
}
impl Pty {
    fn new() -> Self {
        let mut master = -1;
        let mut slave = -1;
        let mut size = libc::winsize {
            ws_row: 33,
            ws_col: 91,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size,
                )
            },
            0
        );
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) },
            0
        );
        Self { master, slave }
    }
    fn attributes(&self) -> libc::termios {
        let mut value = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { libc::tcgetattr(self.slave.as_raw_fd(), &mut value) },
            0
        );
        value
    }
    fn output(&mut self) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            match self.master.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => bytes.extend(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("{error}"),
            }
        }
        bytes
    }
}

fn assert_restored(expected: &libc::termios, actual: &libc::termios) {
    assert_eq!(actual.c_iflag, expected.c_iflag);
    assert_eq!(actual.c_oflag, expected.c_oflag);
    assert_eq!(actual.c_cflag, expected.c_cflag);
    // Darwin sets this transient state when restoring canonical input (xnu tty.c,
    // TIOCSETA). Compare the configured flags without consuming pending input.
    assert_eq!(
        actual.c_lflag & !libc::PENDIN,
        expected.c_lflag & !libc::PENDIN
    );
    assert_eq!(actual.c_cc, expected.c_cc);
}

#[test]
fn raw_attachment_preserves_input_bytes_size_and_output_then_restores_terminal() {
    let start = Instant::now();
    let mut pty = Pty::new();
    let before = pty.attributes();
    let flags = unsafe { libc::fcntl(pty.slave.as_raw_fd(), libc::F_GETFL) };
    let mut terminal = AttachmentTerminal::enter(pty.slave.as_fd(), pty.slave.as_fd()).unwrap();
    assert_eq!(terminal.size().unwrap(), (33, 91));
    let input = [3, 27, 0, 255, b'\r'];
    pty.master.write_all(&input).unwrap();
    let mut received = Vec::new();
    while received.len() < input.len() {
        received.extend(terminal.read().unwrap().unwrap());
        assert!(start.elapsed() < Duration::from_millis(100));
    }
    assert_eq!(received, input);
    assert_eq!(terminal.write(b"draft\n").unwrap(), 6);
    terminal.finish().unwrap();
    assert_restored(&before, &pty.attributes());
    // Darwin also returns its read-only FWASWRITTEN history after output.
    let restorable = libc::O_NONBLOCK
        | libc::O_APPEND
        | libc::O_ASYNC
        | libc::O_SYNC
        | libc::O_DSYNC
        | libc::O_ACCMODE;
    assert_eq!(
        unsafe { libc::fcntl(pty.slave.as_raw_fd(), libc::F_GETFL) } & restorable,
        flags & restorable
    );
    let output = pty.output();
    assert!(output.starts_with(b"\x1b[?1049h"));
    assert!(output.windows(6).any(|bytes| bytes == b"draft\n"));
    assert!(output.ends_with(b"\x1b[?1049l"));
    drop(terminal);
    assert!(
        pty.output().is_empty(),
        "finish and Drop must not restore twice"
    );
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn panic_and_failed_entry_preserve_original_terminal_state() {
    let mut pty = Pty::new();
    let before = pty.attributes();
    let result = std::panic::catch_unwind(|| {
        let _terminal = AttachmentTerminal::enter(pty.slave.as_fd(), pty.slave.as_fd()).unwrap();
        panic!("private attachment failure");
    });
    assert!(result.is_err());
    assert_restored(&before, &pty.attributes());
    let output = pty.output();
    assert!(output.starts_with(b"\x1b[?1049h"));
    assert!(output.ends_with(b"\x1b[?1049l"));
    let invalid = File::open("/dev/null").unwrap();
    assert!(AttachmentTerminal::enter(invalid.as_fd(), pty.slave.as_fd()).is_err());
    assert_restored(&before, &pty.attributes());
    assert!(pty.output().is_empty());
}
