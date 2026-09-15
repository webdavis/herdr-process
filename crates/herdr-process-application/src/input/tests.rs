use super::*;
fn binding(chord: &[u8], profile: &str, action: Action) -> Binding {
    Binding {
        chord: chord.to_vec(),
        profile: profile.into(),
        action,
    }
}
fn router() -> InputRouter {
    InputRouter::new(
        vec![2],
        vec![
            binding(b"R", "review", Action::SplitRight),
            binding(b"\x1b[A", "shell", Action::SplitBelow),
        ],
        Duration::from_millis(100),
    )
    .unwrap()
}
#[test]
fn split_reads_route_only_configured_chords_and_preserve_effect_order() {
    let mut r = router();
    assert_eq!(
        r.feed(b"a\x02", Duration::ZERO),
        vec![Effect::Forward(b"a".to_vec())]
    );
    assert_eq!(
        r.feed(b"Rb\x02\x1b[", Duration::ZERO),
        vec![
            Effect::Invoke {
                profile: "review".into(),
                action: Action::SplitRight
            },
            Effect::Forward(b"b".to_vec())
        ]
    );
    assert_eq!(
        r.feed(b"A", Duration::ZERO),
        vec![Effect::Invoke {
            profile: "shell".into(),
            action: Action::SplitBelow
        }]
    );
    assert_eq!(
        r.feed(b"rR\x1b[A\x03\xff", Duration::ZERO),
        vec![
            Effect::Forward(b"rR\x1b[A".to_vec()),
            Effect::Interrupt,
            Effect::Forward(vec![255])
        ]
    );
}

#[test]
fn timeout_flushes_in_order_at_deadline_and_double_prefix_escapes() {
    let mut r = router();
    assert!(r.feed(b"\x02\x1b[", Duration::ZERO).is_empty());
    assert!(r.expire(Duration::from_millis(99)).is_empty());
    assert_eq!(
        r.expire(Duration::from_millis(100)),
        vec![Effect::Forward(b"\x02\x1b[".to_vec())]
    );
    assert!(r.expire(Duration::from_millis(101)).is_empty());
    assert_eq!(
        r.feed(b"A\x02\x02", Duration::from_millis(101)),
        vec![Effect::Forward(b"A\x02".to_vec())]
    );
    assert!(r.feed(b"\x02", Duration::from_millis(200)).is_empty());
    assert_eq!(
        r.feed(b"R", Duration::from_millis(300)),
        vec![Effect::Forward(b"\x02R".to_vec())]
    );
}

#[test]
fn paste_is_opaque_across_every_split_and_timeout_in_delimiters() {
    let bytes = b"\x02\x1b[200~\x02R\x03\xff\x1b[200~\x02\x02\x1b[201~";
    for split in 0..=bytes.len() {
        let mut r = router();
        let mut effects = r.feed(&bytes[..split], Duration::ZERO);
        effects.extend(r.expire(Duration::from_secs(1)));
        effects.extend(r.feed(&bytes[split..], Duration::from_secs(1)));
        effects.extend(r.expire(Duration::from_secs(2)));
        let mut forwarded = Vec::new();
        for effect in effects {
            match effect {
                Effect::Forward(b) => forwarded.extend(b),
                other => panic!("paste invoked {other:?} at split {split}"),
            }
        }
        assert_eq!(forwarded, bytes, "split {split}");
        assert_eq!(
            r.feed(b"\x02R", Duration::from_secs(3)),
            vec![Effect::Invoke {
                profile: "review".into(),
                action: Action::SplitRight
            }]
        );
    }
}

#[test]
fn rejects_ambiguous_bindings_literal_prefix_and_paste_conflicts() {
    let bad = [
        (vec![], vec![]),
        (vec![2], vec![binding(b"", "a", Action::Kill)]),
        (
            vec![2],
            vec![
                binding(b"r", "a", Action::Kill),
                binding(b"r", "b", Action::Kill),
            ],
        ),
        (
            vec![2],
            vec![
                binding(b"r", "a", Action::Kill),
                binding(b"rr", "b", Action::Kill),
            ],
        ),
        (
            vec![2],
            vec![
                binding(b"\t", "ctrl-i", Action::Kill),
                binding(b"\t", "tab", Action::Kill),
            ],
        ),
        (vec![2], vec![binding(b"\x02", "a", Action::Kill)]),
        (b"ab".to_vec(), vec![binding(b"a", "a", Action::Kill)]),
        (b"ab".to_vec(), vec![binding(b"abc", "a", Action::Kill)]),
        (b"\x1b".to_vec(), vec![]),
        (vec![2], vec![binding(b"\x1b[200~", "a", Action::Kill)]),
    ];
    let errors = [
        RouterError::EmptyPrefix,
        RouterError::EmptyChord { binding: 0 },
        RouterError::AmbiguousBindings {
            first: 0,
            second: 1,
        },
        RouterError::AmbiguousBindings {
            first: 0,
            second: 1,
        },
        RouterError::AmbiguousBindings {
            first: 0,
            second: 1,
        },
        RouterError::PrefixConflict { binding: 0 },
        RouterError::PrefixConflict { binding: 0 },
        RouterError::PrefixConflict { binding: 0 },
        RouterError::PasteConflict { binding: None },
        RouterError::PasteConflict { binding: Some(0) },
    ];
    for ((prefix, bindings), error) in bad.into_iter().zip(errors) {
        assert_eq!(
            InputRouter::new(prefix, bindings, Duration::from_millis(100)).err(),
            Some(error)
        );
    }
    assert_eq!(
        InputRouter::new(vec![2], vec![], Duration::ZERO).err(),
        Some(RouterError::ZeroTimeout)
    );
}

#[test]
fn shared_mapping_routes_all_actions_and_more_than_thirty_two_profiles() {
    let bindings: Vec<_> = (0..80)
        .map(|i| {
            binding(
                format!("{i:03}").as_bytes(),
                &format!("profile-{i}"),
                [
                    Action::SplitRight,
                    Action::SplitBelow,
                    Action::ToggleFloat,
                    Action::Kill,
                ][i % 4],
            )
        })
        .collect();
    let mut r = InputRouter::new(
        b"\x1bx".to_vec(),
        bindings.clone(),
        Duration::from_millis(100),
    )
    .unwrap();
    for binding in bindings {
        let mut effects = Vec::new();
        for byte in b"\x1bx".iter().chain(&binding.chord) {
            effects.extend(r.feed(&[*byte], Duration::ZERO));
        }
        assert_eq!(
            effects,
            vec![Effect::Invoke {
                profile: binding.profile,
                action: binding.action
            }]
        );
    }
    assert!(r.feed(b"\x1bx\x1b", Duration::ZERO).is_empty());
    assert_eq!(
        r.feed(b"x", Duration::ZERO),
        vec![Effect::Forward(b"\x1bx".to_vec())]
    );
}

#[test]
fn unmatched_escape_unicode_and_partial_paste_are_byte_preserved() {
    let bytes = b"\x02?\x1b\x1b[20X\x02\x1b[99u\xc3\xa9\x00\xff\x1b";
    for chunk_size in 1..=bytes.len() {
        let mut r = router();
        let mut effects = Vec::new();
        for chunk in bytes.chunks(chunk_size) {
            effects.extend(r.feed(chunk, Duration::ZERO));
        }
        effects.extend(r.expire(Duration::from_millis(100)));
        let mut forwarded = Vec::new();
        for effect in effects {
            match effect {
                Effect::Forward(b) => forwarded.extend(b),
                other => panic!("unmatched input invoked {other:?}"),
            }
        }
        assert_eq!(forwarded, bytes);
    }
}

#[test]
fn empty_reads_and_backward_clock_do_not_flush_or_extend_deadline() {
    let mut r = router();
    assert!(r.feed(b"\x02", Duration::from_millis(100)).is_empty());
    assert!(r.expire(Duration::from_millis(50)).is_empty());
    assert!(r.feed(b"", Duration::from_millis(199)).is_empty());
    assert_eq!(
        r.expire(Duration::from_millis(200)),
        vec![Effect::Forward(vec![2])]
    );
}

mod interrupt;

#[test]
fn bracketed_paste_is_forwarded_whole_without_routing_pasted_shortcuts() {
    let input = b"\x1b[200~\x02R\x03\xff\x1b[200~literal\x1b[201~";
    for split in 0..=input.len() {
        let mut r = router();
        let mut effects = r.feed(&input[..split], Duration::ZERO);
        effects.extend(r.feed(&input[split..], Duration::ZERO));
        let mut bytes = Vec::new();
        for effect in effects {
            match effect {
                Effect::Forward(data) => bytes.extend(data),
                other => panic!("paste routed {other:?}"),
            }
        }
        assert_eq!(bytes, input.as_slice(), "split={split}");
        assert!(matches!(
            r.feed(b"\x02R", Duration::ZERO).as_slice(),
            [Effect::Invoke { .. }]
        ));
    }
}
