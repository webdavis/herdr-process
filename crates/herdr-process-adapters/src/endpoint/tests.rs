use super::*;
use crate::{Readiness, test_support::TempRoot};
use herdr_process_protocol::Response;
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    time::{Duration, Instant},
};

#[test]
fn private_endpoint_serializes_owners_and_connects_without_replacing_live_socket() {
    let temporary = TempRoot::new();
    let path = temporary.join("endpoint");
    let endpoint = Endpoint::new(&path).unwrap();
    let first = endpoint
        .bind()
        .unwrap()
        .expect("first manager must own endpoint");
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o700);
    assert_eq!(
        fs::metadata(endpoint.socket()).unwrap().mode() & 0o777,
        0o600
    );
    let identity = fs::metadata(endpoint.socket()).unwrap().ino();
    assert!(endpoint.bind().unwrap().is_none());
    assert_eq!(fs::metadata(endpoint.socket()).unwrap().ino(), identity);
    let mut client = endpoint.connect().unwrap();
    let mut server = first.accept().unwrap().unwrap();
    server.send(&Response::Ack {}).unwrap();
    server.flush().unwrap();
    let start = Instant::now();
    loop {
        let messages = client.read::<Response>().unwrap().unwrap();
        if !messages.is_empty() {
            assert_eq!(messages, vec![Response::Ack {}]);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    }
    drop(first);
    assert!(!endpoint.socket().exists());
    rebind_once_forked_siblings_drop_the_inherited_lock(&endpoint);
}

fn rebind_once_forked_siblings_drop_the_inherited_lock(endpoint: &Endpoint) {
    let start = Instant::now();
    while endpoint.bind().unwrap().is_none() {
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "lock never released"
        );
        std::thread::yield_now();
    }
}

#[test]
fn unsafe_paths_are_rejected_without_changing_existing_bytes_or_permissions() {
    let temporary = TempRoot::new();
    let path = temporary.join("world-readable");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(Endpoint::new(&path).is_err());
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o755);
    let link = temporary.join("link");
    symlink(&path, &link).unwrap();
    assert!(Endpoint::new(&link).is_err());
    let endpoint = Endpoint::new(&temporary.join("occupied")).unwrap();
    fs::write(endpoint.socket(), b"unrelated bytes").unwrap();
    assert!(endpoint.bind().is_err());
    assert_eq!(fs::read(endpoint.socket()).unwrap(), b"unrelated bytes");
}

#[test]
fn stale_private_socket_recovers_but_replaced_socket_survives_old_owner_drop() {
    let temporary = TempRoot::new();
    let endpoint = Endpoint::new(&temporary.join("endpoint")).unwrap();
    drop(UnixListener::bind(endpoint.socket()).unwrap());
    let owner = endpoint.bind().unwrap().unwrap();
    let retained = endpoint.socket().with_extension("retained");
    fs::rename(endpoint.socket(), &retained).unwrap();
    let replacement = UnixListener::bind(endpoint.socket()).unwrap();
    let identity = fs::metadata(endpoint.socket()).unwrap().ino();
    drop(owner);
    assert_eq!(fs::metadata(endpoint.socket()).unwrap().ino(), identity);
    drop(replacement);
}

#[test]
fn watched_listener_waits_the_whole_timeout_when_nobody_connects() {
    let temporary = TempRoot::new();
    let endpoint = Endpoint::new(&temporary.join("endpoint")).unwrap();
    let listener = endpoint.bind().unwrap().unwrap();
    let mut readiness = Readiness::default();
    listener.watch(&mut readiness);
    let start = Instant::now();
    readiness.wait(Some(Duration::from_millis(50))).unwrap();
    assert!(
        start.elapsed() >= Duration::from_millis(50),
        "returned after {:?}",
        start.elapsed()
    );
    assert!(listener.accept().unwrap().is_none());
}

#[test]
fn watched_listener_wakes_as_soon_as_a_client_connects() {
    let temporary = TempRoot::new();
    let endpoint = Endpoint::new(&temporary.join("endpoint")).unwrap();
    let listener = endpoint.bind().unwrap().unwrap();
    let socket = endpoint.socket();
    let client = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        UnixStream::connect(socket).unwrap()
    });
    let mut readiness = Readiness::default();
    listener.watch(&mut readiness);
    let start = Instant::now();
    readiness.wait(Some(Duration::from_secs(5))).unwrap();
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "woke only after {:?}",
        start.elapsed()
    );
    assert!(listener.accept().unwrap().is_some());
    client.join().unwrap();
}
