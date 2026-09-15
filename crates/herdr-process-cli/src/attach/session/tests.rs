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
    // Both descriptors are immediately owned; neither refers to the operator terminal.
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
