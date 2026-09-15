use super::*;
use herdr_process_protocol::{Request, Response};
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[test]
fn partial_socket_reads_deliver_once_and_preserve_disconnect_framing() {
    let (mut writer, reader) = UnixStream::pair().unwrap();
    reader
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut peer = Peer::new(reader).unwrap();
    let started = Instant::now();
    assert!(peer.read::<Request>().unwrap().unwrap().is_empty());
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "an empty socket read must return without waiting"
    );
    let frame = encode(&Request::Input {
        bytes: vec![27, 0, 255],
    })
    .unwrap();
    for byte in &frame[..frame.len() - 1] {
        writer.write_all(&[*byte]).unwrap();
        assert!(peer.read::<Request>().unwrap().unwrap().is_empty());
    }
    writer.write_all(&frame[frame.len() - 1..]).unwrap();
    let end = Instant::now() + Duration::from_millis(100);
    let received = loop {
        assert!(Instant::now() < end);
        let messages = peer.read::<Request>().unwrap().unwrap();
        if !messages.is_empty() {
            break messages;
        }
        std::thread::yield_now();
    };
    assert_eq!(
        received,
        vec![Request::Input {
            bytes: vec![27, 0, 255]
        }]
    );
    assert!(peer.read::<Request>().unwrap().unwrap().is_empty());
    writer.write_all(&frame[..2]).unwrap();
    drop(writer);
    let end = Instant::now() + Duration::from_millis(100);
    let error = loop {
        assert!(Instant::now() < end);
        match peer.read::<Request>() {
            Err(error) => break error,
            Ok(Some(messages)) => assert!(messages.is_empty()),
            Ok(None) => panic!("partial frame accepted at disconnect"),
        }
        std::thread::yield_now();
    };
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[test]
fn backpressure_keeps_screen_bytes_in_order_without_blocking_or_unbounded_growth() {
    let start = Instant::now();
    let (one, two) = UnixStream::pair().unwrap();
    one.set_write_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    two.set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut sender = Peer::new(one).unwrap();
    let mut receiver = Peer::new(two).unwrap();
    let bytes = vec![b'x'; 256 * 1024];
    sender
        .send(&Response::Screen {
            bytes: bytes.clone(),
        })
        .unwrap();
    sender.send(&Response::Retire {}).unwrap();
    assert!(
        !sender.flush().unwrap(),
        "private socket must exercise short writes"
    );
    let mut received = Vec::new();
    while received.len() < 2 {
        assert!(start.elapsed() < Duration::from_millis(800));
        sender.flush().unwrap();
        received.extend(receiver.read::<Response>().unwrap().unwrap());
    }
    assert_eq!(
        received,
        vec![Response::Screen { bytes }, Response::Retire {}]
    );
    assert!(sender.is_idle());
    let huge = Response::Error {
        message: "x".repeat(MAX_FRAME),
    };
    assert!(sender.send(&huge).is_err());
    assert!(
        sender.is_idle(),
        "rejected frame must not damage the existing queue"
    );
}
