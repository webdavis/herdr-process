use super::*;
use crate::{Request, Response};

#[test]
fn split_and_coalesced_frames_preserve_order_and_raw_terminal_bytes() {
    let first = Request::Input {
        bytes: vec![0, 3, 27, 255, 240],
    };
    let second = Request::Resize {
        rows: 40,
        cols: 120,
    };
    let mut wire = encode(&first).unwrap();
    wire.extend(encode(&second).unwrap());
    for split in 0..=wire.len() {
        let mut decoder = Decoder::default();
        let mut actual = decoder.feed::<Request>(&wire[..split]).unwrap();
        actual.extend(decoder.feed::<Request>(&wire[split..]).unwrap());
        assert_eq!(
            actual,
            vec![
                Request::Input {
                    bytes: vec![0, 3, 27, 255, 240]
                },
                Request::Resize {
                    rows: 40,
                    cols: 120
                },
            ],
            "split {split}"
        );
        decoder.finish().unwrap();
    }
}

#[test]
fn oversized_or_unknown_version_frames_fail_before_dispatch() {
    let mut decoder = Decoder::default();
    let header = ((MAX_FRAME + 1) as u32).to_be_bytes();
    assert_eq!(
        decoder.feed::<Request>(&header).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    let mut decoder = Decoder::default();
    let payload = br#"{"version":2,"message":{"type":"detach"}}"#;
    let mut wire = (payload.len() as u32).to_be_bytes().to_vec();
    wire.extend(payload);
    assert_eq!(
        decoder.feed::<Request>(&wire).unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
}

#[test]
fn malformed_truncated_and_unknown_message_fields_are_rejected() {
    for payload in [
        b"garbage".as_slice(),
        br#"{"version":1,"message":{"type":"detach","extra":true}}"#,
    ] {
        let mut wire = (payload.len() as u32).to_be_bytes().to_vec();
        wire.extend(payload);
        assert!(Decoder::default().feed::<Request>(&wire).is_err());
    }
    let wire = encode(&Response::Screen {
        bytes: b"draft".to_vec(),
    })
    .unwrap();
    for prefix in 1..wire.len() {
        let mut decoder = Decoder::default();
        assert!(
            decoder
                .feed::<Response>(&wire[..prefix])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            decoder.finish().unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
