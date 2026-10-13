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
        "input prefix must not be empty",
        "binding a:kill has an empty chord",
        "bindings a:kill and b:kill use the same or overlapping keys",
        "bindings a:kill and b:kill use the same or overlapping keys",
        "bindings ctrl-i:kill and tab:kill use the same or overlapping keys",
        "binding a:kill overlaps the prefix key",
        "binding a:kill overlaps the prefix key",
        "binding a:kill overlaps the prefix key",
        "input prefix conflicts with bracketed paste detection",
        "binding a:kill conflicts with bracketed paste detection",
    ];
    for ((prefix, bindings), error) in bad.into_iter().zip(errors) {
        assert_eq!(
            Keymap::new(prefix, bindings)
                .map_err(|e| e.to_string())
                .err(),
            Some(error.to_string())
        );
    }
}

#[test]
fn rejects_interrupt_prefix_and_chord_ambiguity() {
    assert_eq!(
        Keymap::new(vec![3], vec![])
            .map_err(|e| e.to_string())
            .err(),
        Some("input prefix conflicts with ordinary Ctrl+C interrupt".to_string())
    );
    assert_eq!(
        Keymap::new(vec![2], vec![binding(b"\x03", "a", Action::Kill)])
            .map_err(|e| e.to_string())
            .err(),
        Some("binding a:kill conflicts with ordinary Ctrl+C interrupt".to_string())
    );
}
