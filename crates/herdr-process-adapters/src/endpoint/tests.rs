use super::*;
use crate::test_support::TempRoot;
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
    // Another test in this binary may have forked a child that still holds an
    // inherited copy of the released lock descriptor until it reaches exec, so
    // the re-bind is retried for a bounded window rather than asserted once.
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
