use super::*;

const ERROR: &str =
    "shell split-right: split needs a workspace and pane; invoke from a current pane";
const SHOWN: &str = "\x1b7\x1b[24;1H\x1b[2K\x1b[38;2;243;139;168mshell split-right: split needs a workspace and pane; invoke from a current pane\x1b[0m\x1b8";

fn attached_session(script: impl FnOnce(&mut UnixStream) + Send + 'static) -> String {
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
                message: ERROR.into(),
            },
        );
        script(&mut server);
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
    String::from_utf8_lossy(&output).into_owned()
}

#[test]
fn an_expired_notice_gives_way_to_the_last_screen_without_waiting_for_the_next() {
    let output = attached_session(|_| std::thread::sleep(Duration::from_millis(5300)));
    assert_eq!(output.matches(SHOWN).count(), 1, "{output:?}");
    assert!(output.contains(&format!("{SHOWN}$ ")), "{output:?}");
    assert!(!output.contains("\x1b[2K\x1b8"), "{output:?}");
}

#[test]
fn an_error_then_retire_shows_the_notice_and_the_session_ends_cleanly() {
    let output = attached_session(|_| std::thread::sleep(Duration::from_millis(50)));
    assert_eq!(output.matches(SHOWN).count(), 1, "{output:?}");
    assert_eq!(output.matches("$ ").count(), 1, "{output:?}");
}
