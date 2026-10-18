use super::*;
use std::os::unix::net::UnixStream;

#[test]
fn fragmented_control_does_not_parse_until_frame_completes() -> Result<()> {
    let start = Instant::now();
    let (mut peer, stream) = UnixStream::pair()?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut control = Control::new(stream)?;
    peer.write_all(b"\"St")?;
    assert!(control.receive()?.is_none());
    assert!(
        start.elapsed() < Duration::from_millis(50),
        "a partial control frame must return without waiting"
    );
    peer.write_all(b"op\"\n")?;
    assert!(matches!(control.receive()?, Some(Message::Stop)));
    assert!(start.elapsed() < Duration::from_secs(1));
    Ok(())
}
#[test]
fn oversized_complete_frame_is_rejected() -> Result<()> {
    let start = Instant::now();
    let (mut peer, stream) = UnixStream::pair()?;
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    let mut control = Control::new(stream)?;
    let mut bytes = serde_json::to_vec(&Message::Failure("x".repeat(65530)))?;
    bytes.push(b'\n');
    // Feed bounded chunks and consume them, never depend on socket buffer size.
    for chunk in bytes.chunks(4095) {
        peer.write_all(chunk)?;
        match control.receive() {
            Err(_) => {
                assert!(start.elapsed() < Duration::from_secs(1));
                return Ok(());
            }
            Ok(Some(_)) => panic!("oversized complete frame was accepted"),
            Ok(None) => (),
        }
    }
    panic!("oversized frame did not fail")
}
