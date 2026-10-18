use super::*;
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsFd, AsRawFd, FromRawFd},
        unix::net::UnixStream,
    },
};

#[test]
fn stalled_handshake_is_bounded_and_does_not_forward_input() {
    let start = Instant::now();
    let mut master = -1;
    let mut slave = -1;
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    let mut original: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut original) },
        0
    );
    let (client, mut server) = UnixStream::pair().unwrap();
    let mut terminal = AttachmentTerminal::enter(slave.as_fd(), slave.as_fd()).unwrap();
    master.write_all(b"unfinished\x03").unwrap();
    let worker = std::thread::spawn(move || {
        server
            .set_read_timeout(Some(Duration::from_millis(120)))
            .unwrap();
        let mut size = [0; 4];
        server.read_exact(&mut size).unwrap();
        let mut frame = vec![0; u32::from_be_bytes(size) as usize];
        server.read_exact(&mut frame).unwrap();
        let message: serde_json::Value = serde_json::from_slice(&frame).unwrap();
        assert_eq!(message["message"]["type"], "attach");
        let mut unexpected = [0; 1];
        assert!(
            server.read(&mut unexpected).is_err(),
            "input escaped before Ack"
        );
    });
    let mut peer = Peer::new(client).unwrap();
    let error = run(
        &mut peer,
        &mut terminal,
        "test".into(),
        "ticket".into(),
        Duration::from_millis(30),
    )
    .unwrap_err();
    let elapsed = start.elapsed();
    drop(terminal);
    let mut restored: libc::termios = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), &mut restored) },
        0
    );
    worker.join().unwrap();
    assert!(error.to_string().contains("timed out"));
    assert_eq!(
        original.c_lflag & !libc::PENDIN,
        restored.c_lflag & !libc::PENDIN
    );
    assert!(elapsed < Duration::from_millis(90), "{elapsed:?}");
    assert!(start.elapsed() < Duration::from_millis(300));
}
#[test]
fn screen_queue_rejects_unbounded_pending_memory() {
    let start = Instant::now();
    let mut screens = Screens::default();
    screens.push(vec![b'x'; 2 * MAX_FRAME]).unwrap();
    assert!(screens.push(vec![b'y']).is_err());
    assert!(start.elapsed() < Duration::from_millis(200));
}

#[test]
fn an_error_while_attached_is_shown_on_the_bottom_row_and_the_session_continues() {
    let (mut master, slave) = pty(24, 80);
    let (client, mut server) = UnixStream::pair().unwrap();
    let mut terminal = AttachmentTerminal::enter(slave.as_fd(), slave.as_fd()).unwrap();
    let worker = std::thread::spawn(move || {
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        assert_eq!(frame(&mut server)["message"]["type"], "attach");
        send(
            &mut server,
            &Response::Screen {
                bytes: b"$ ".to_vec(),
            },
        );
        send(&mut server, &Response::Attached {});
        assert_eq!(frame(&mut server)["message"]["type"], "ready");
        send(&mut server, &Response::Ack {});
        send(
            &mut server,
            &Response::Error {
                message: "shell split-right: split needs a workspace and pane; invoke from a current pane".into(),
            },
        );
        std::thread::sleep(Duration::from_millis(50));
        send(
            &mut server,
            &Response::Screen {
                bytes: b"$ ls".to_vec(),
            },
        );
        std::thread::sleep(Duration::from_millis(50));
        send(&mut server, &Response::Retire {});
    });
    let mut peer = Peer::new(client).unwrap();
    run(
        &mut peer,
        &mut terminal,
        "test".into(),
        "ticket".into(),
        Duration::from_secs(2),
    )
    .unwrap();
    drop(terminal);
    worker.join().unwrap();
    let mut output = Vec::new();
    master.read_to_end(&mut output).ok();
    let output = String::from_utf8_lossy(&output);
    let shown = "\x1b7\x1b[24;1H\x1b[2K\x1b[38;2;243;139;168mshell split-right: split needs a workspace and pane; invoke from a current pane\x1b[0m\x1b8";
    assert_eq!(output.matches(shown).count(), 2, "{output:?}");
    assert!(output.contains(&format!("$ ls{shown}")), "{output:?}");
}

fn pty(rows: u16, cols: u16) -> (File, File) {
    let mut master = -1;
    let mut slave = -1;
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };
    size.ws_row = rows;
    size.ws_col = cols;
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
    assert_eq!(
        unsafe { libc::fcntl(master, libc::F_SETFL, libc::O_NONBLOCK) },
        0
    );
    unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) }
}

fn frame(server: &mut UnixStream) -> serde_json::Value {
    let mut size = [0; 4];
    server.read_exact(&mut size).unwrap();
    let mut frame = vec![0; u32::from_be_bytes(size) as usize];
    server.read_exact(&mut frame).unwrap();
    serde_json::from_slice(&frame).unwrap()
}

fn send(server: &mut UnixStream, response: &Response) {
    server
        .write_all(&herdr_process_protocol::encode(response).unwrap())
        .unwrap();
}

mod notices;
