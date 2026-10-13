use super::*;
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
        KeymapError::EmptyPrefix,
        KeymapError::EmptyChord { binding: 0 },
        KeymapError::AmbiguousBindings {
            first: 0,
            second: 1,
        },
        KeymapError::AmbiguousBindings {
            first: 0,
            second: 1,
        },
        KeymapError::AmbiguousBindings {
            first: 0,
            second: 1,
        },
        KeymapError::PrefixConflict { binding: 0 },
        KeymapError::PrefixConflict { binding: 0 },
        KeymapError::PrefixConflict { binding: 0 },
        KeymapError::PasteConflict { binding: None },
        KeymapError::PasteConflict { binding: Some(0) },
    ];
    for ((prefix, bindings), error) in bad.into_iter().zip(errors) {
        assert_eq!(Keymap::new(prefix, bindings).err(), Some(error));
    }
}

#[test]
fn rejects_interrupt_prefix_and_chord_ambiguity() {
    assert_eq!(
        Keymap::new(vec![3], vec![]).err(),
        Some(KeymapError::InterruptConflict { binding: None })
    );
    assert_eq!(
        Keymap::new(vec![2], vec![binding(b"\x03", "a", Action::Kill)]).err(),
        Some(KeymapError::InterruptConflict { binding: Some(0) })
    );
}
