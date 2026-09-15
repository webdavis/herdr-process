use super::*;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::mpsc,
    time::Instant,
};
fn request() -> Request {
    Request::Action {
        profile: "test".into(),
        action: "toggle-float".into(),
        configuration: "generation".into(),
        target: herdr_process_protocol::Target {
            workspace: "w".into(),
            pane: "p".into(),
        },
    }
}
fn receive(stream: &mut UnixStream) -> serde_json::Value {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut size = [0; 4];
    stream.read_exact(&mut size).unwrap();
    let mut data = vec![0; u32::from_be_bytes(size) as usize];
    stream.read_exact(&mut data).unwrap();
    serde_json::from_slice(&data).unwrap()
}
fn frame(stream: &mut UnixStream, payload: &[u8]) {
    stream
        .write_all(&(payload.len() as u32).to_be_bytes())
        .unwrap();
    stream.write_all(payload).unwrap();
}
#[test]
fn acknowledgement_returns_without_waiting_for_server_lifetime() {
    let start = Instant::now();
    let (client, mut server) = UnixStream::pair().unwrap();
    let (release, released) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let value = receive(&mut server);
        assert_eq!(
            value,
            serde_json::json!({"version":1,"message":{"type":"action","profile":"test","action":"toggle-float","configuration":"generation","target":{"workspace":"w","pane":"p"}}})
        );
        frame(&mut server, br#"{"version":1,"message":{"type":"ack"}}"#);
        released.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    let result = exchange(
        &mut Peer::new(client).unwrap(),
        &request(),
        Duration::from_millis(100),
    );
    let active = !worker.is_finished();
    let _ = release.send(());
    worker.join().unwrap();
    result.unwrap();
    assert!(active);
    assert!(start.elapsed() < Duration::from_secs(3));
}
#[test]
fn malformed_eof_and_remote_error_are_failures() {
    for reply in [
        Some(&b"not json"[..]),
        None,
        Some(&br#"{"version":1,"message":{"type":"error","message":"view refused"}}"#[..]),
    ] {
        let start = Instant::now();
        let (client, mut server) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            receive(&mut server);
            if let Some(reply) = reply {
                frame(&mut server, reply);
            }
        });
        let result = exchange(
            &mut Peer::new(client).unwrap(),
            &request(),
            Duration::from_millis(100),
        );
        worker.join().unwrap();
        let message = result.unwrap_err().to_string();
        if reply.is_some_and(|bytes| bytes.windows(4).any(|w| w == b"view")) {
            assert_eq!(message, "view refused");
        }
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
#[test]
fn stalled_ack_times_out_once_with_unknown_result() {
    let start = Instant::now();
    let (client, mut server) = UnixStream::pair().unwrap();
    let worker = std::thread::spawn(move || {
        receive(&mut server);
        server
            .set_read_timeout(Some(Duration::from_millis(180)))
            .unwrap();
        let mut extra = [0; 1];
        match server.read(&mut extra) {
            Err(error) => assert!(matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            )),
            other => panic!("duplicate request or early close: {other:?}"),
        }
    });
    let mut peer = Peer::new(client).unwrap();
    let result = exchange(&mut peer, &request(), Duration::from_millis(30));
    let elapsed = start.elapsed();
    worker.join().unwrap();
    assert!(result.unwrap_err().to_string().contains("unknown"));
    assert!(elapsed < Duration::from_millis(120), "{elapsed:?}");
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[test]
fn composed_controller_loads_selected_configuration_and_sends_one_action() {
    let start = Instant::now();
    let temporary = crate::test_support::TempRoot::new();
    let root = temporary.path().to_path_buf();
    let endpoint = Endpoint::new(&root).unwrap();
    let listener = std::os::unix::net::UnixListener::bind(endpoint.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let profiles = root.join("profiles.toml");
    let herdr = root.join("herdr.toml");
    std::fs::write(
        &profiles,
        "[windows.test]\nprogram='/bin/cat'\ncwd='~/'\nwidth=80\nheight=70\nctrl_c='hide'\n",
    )
    .unwrap();
    std::fs::write(&herdr, "[keys]\nprefix='ctrl+a'\n").unwrap();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut peer = loop {
            match listener.accept() {
                // BSD accept inherits the listener's non-blocking flag, which
                // would make every read below fail with EAGAIN instead of
                // honouring the read timeout.
                Ok((peer, _)) => {
                    peer.set_nonblocking(false).unwrap();
                    break peer;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("{error}"),
            }
            assert!(Instant::now() < deadline, "controller did not connect");
            std::thread::yield_now();
        };
        let envelope = receive(&mut peer);
        let request = &envelope["message"];
        assert_eq!(request["type"], "action");
        assert_eq!(request["action"], "toggle-float");
        assert_eq!(request["profile"], "test");
        assert_eq!(
            request["target"],
            serde_json::json!({"workspace":"workspace","pane":"pane"})
        );
        assert!(
            request["configuration"]
                .as_str()
                .unwrap()
                .contains("/bin/cat")
        );
        frame(&mut peer, br#"{"version":1,"message":{"type":"ack"}}"#);
        peer
    });
    let options = Options {
        profiles: Some(profiles),
        herdr: Some(herdr),
    };
    let result = run_in(
        &options,
        "test",
        Action::ToggleFloat,
        |name| match name {
            "HOME" => Some(root.as_os_str().to_owned()),
            "HERDR_ENV" => Some("1".into()),
            "HERDR_BIN_PATH" => Some("/invalid/private-host".into()),
            "HERDR_SOCKET_PATH" => Some(root.join("host.sock").into_os_string()),
            "HERDR_PLUGIN_CONTEXT_JSON" => {
                Some(r#"{"workspace_id":"workspace","focused_pane_id":"pane"}"#.into())
            }
            _ => None,
        },
        |_, _| root.clone(),
    );
    let server = worker.join().unwrap();
    result.unwrap();
    drop(server);
    assert!(start.elapsed() < Duration::from_secs(4));
}
