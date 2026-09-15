use super::*;
#[test]
fn interrupt_is_ordered_and_paste_remains_data_at_every_split() {
    let bytes = b"a\x03\x02R\x1b[200~\x03\x1b[201~\x03z";
    for split in 0..=bytes.len() {
        let mut r = router();
        let mut actual = r.feed(&bytes[..split], Duration::ZERO);
        actual.extend(r.feed(&bytes[split..], Duration::ZERO));
        let mut joined = Vec::new();
        for effect in actual {
            match effect {
                Effect::Forward(bytes) => forward(&mut joined, &bytes),
                other => joined.push(other),
            }
        }
        assert_eq!(
            joined,
            vec![
                Effect::Forward(b"a".to_vec()),
                Effect::Interrupt,
                Effect::Invoke {
                    profile: "review".into(),
                    action: Action::SplitRight
                },
                Effect::Forward(b"\x1b[200~\x03\x1b[201~".to_vec()),
                Effect::Interrupt,
                Effect::Forward(b"z".to_vec())
            ],
            "split {split}"
        );
    }
    let mut r = router();
    assert_eq!(
        r.feed(b"\x02\x03", Duration::ZERO),
        vec![Effect::Forward(vec![2, 3])]
    );
}

#[test]
fn rejects_interrupt_prefix_and_chord_ambiguity() {
    assert_eq!(
        InputRouter::new(vec![3], vec![], Duration::from_millis(100)).err(),
        Some(RouterError::InterruptConflict { binding: None })
    );
    assert_eq!(
        InputRouter::new(
            vec![2],
            vec![binding(b"\x03", "a", Action::Kill)],
            Duration::from_millis(100)
        )
        .err(),
        Some(RouterError::InterruptConflict { binding: Some(0) })
    );
}

#[test]
fn escape_modified_interrupt_and_unknown_sequences_remain_bytes() {
    for bytes in [b"\x1b\x03".as_slice(), b"\x1b[20\x03", b"\x02\x03"] {
        for split in 0..=bytes.len() {
            let mut r = router();
            let mut effects = r.feed(&bytes[..split], Duration::ZERO);
            effects.extend(r.feed(&bytes[split..], Duration::ZERO));
            let mut forwarded = Vec::new();
            for effect in effects {
                match effect {
                    Effect::Forward(bytes) => forwarded.extend(bytes),
                    other => panic!("unknown input produced {other:?}"),
                }
            }
            assert_eq!(forwarded, bytes);
        }
    }
}
