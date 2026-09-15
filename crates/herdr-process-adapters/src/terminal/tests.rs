use super::*;

#[test]
fn hidden_terminal_answers_cursor_and_status_queries_once() {
    let mut terminal = Terminal::new(6, 30);
    terminal.feed(b"\x1b[3;5H\x1b[6n\x1b[5n");
    assert_eq!(terminal.take_responses(), b"\x1b[3;5R\x1b[0n");
    assert!(terminal.take_responses().is_empty());
    assert!(
        !terminal
            .snapshot()
            .windows(4)
            .any(|part| part == b"\x1b[6n")
    );
}

fn attach(terminal: &Terminal, rows: u16, cols: u16) -> vt100::Parser {
    let mut attachment = vt100::Parser::new(rows, cols, 0);
    attachment.process(&terminal.snapshot());
    attachment
}

#[test]
fn fresh_attachment_restores_visible_text_cursor_and_input_modes() {
    let mut terminal = Terminal::new(6, 30);
    terminal.feed(b"draft comment\x1b[3;5H\x1b[?25l\x1b[?1h\x1b=\x1b[?2004h\x1b[?1002h\x1b[?1006h");
    let fresh = attach(&terminal, 6, 30);
    assert_eq!(fresh.screen().contents(), "draft comment");
    assert_eq!(fresh.screen().cursor_position(), (2, 4));
    assert!(fresh.screen().hide_cursor());
    assert!(fresh.screen().application_cursor());
    assert!(fresh.screen().application_keypad());
    assert!(fresh.screen().bracketed_paste());
    assert_eq!(
        fresh.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::ButtonMotion
    );
    assert_eq!(
        fresh.screen().mouse_protocol_encoding(),
        vt100::MouseProtocolEncoding::Sgr
    );
}

#[test]
fn hidden_alternate_screen_and_return_keep_the_original_draft() {
    let mut terminal = Terminal::new(6, 30);
    terminal.feed(b"unfinished shell input\x1b[?1049h\x1b[2J\x1b[Hreview draft");
    assert_eq!(attach(&terminal, 6, 30).screen().contents(), "review draft");
    terminal.feed(b"\x1b[2;1Hcomment while hidden");
    assert_eq!(
        attach(&terminal, 6, 30).screen().contents(),
        "review draft\ncomment while hidden"
    );
    terminal.feed(b"\x1b[?1049l");
    let fresh = attach(&terminal, 6, 30);
    assert_eq!(fresh.screen().contents(), "unfinished shell input");
    assert_eq!(fresh.screen().cursor_position(), (0, 22));
}

#[test]
fn saved_cursor_scroll_region_unicode_and_attributes_survive_hidden_resize() {
    let mut terminal = Terminal::new(6, 30);
    terminal.feed("\x1b[2;2H\x1b[1;38;2;23;45;67m界e\u{301}\x1b7\x1b[5;1Hhidden".as_bytes());
    terminal.resize(8, 40);
    terminal.feed(b"\x1b8!\x1b[4;6r\x1b[6;1Hbottom\nnext");
    let fresh = attach(&terminal, 8, 40);
    let wide = fresh.screen().cell(1, 1).unwrap();
    assert_eq!(wide.contents(), "界");
    assert!(wide.is_wide());
    assert!(wide.bold());
    assert_eq!(wide.fgcolor(), vt100::Color::Rgb(23, 45, 67));
    assert_eq!(fresh.screen().cell(1, 3).unwrap().contents(), "e\u{301}");
    assert_eq!(fresh.screen().cell(1, 4).unwrap().contents(), "!");
    assert!(fresh.screen().contents().contains("bottom"));
    assert!(fresh.screen().contents().contains("next"));
    assert_eq!(fresh.screen().size(), (8, 40));
}

#[test]
fn successive_snapshots_clear_disabled_mouse_modes() {
    let mut terminal = Terminal::new(6, 30);
    let mut attachment = vt100::Parser::new(6, 30, 0);
    terminal.feed(b"\x1b[?1002h\x1b[?1006h");
    attachment.process(&terminal.snapshot());
    assert_eq!(
        attachment.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::ButtonMotion
    );
    assert_eq!(
        attachment.screen().mouse_protocol_encoding(),
        vt100::MouseProtocolEncoding::Sgr
    );
    terminal.feed(b"\x1b[?1002l\x1b[?1006l");
    attachment.process(&terminal.snapshot());
    assert_eq!(
        attachment.screen().mouse_protocol_mode(),
        vt100::MouseProtocolMode::None
    );
    assert_eq!(
        attachment.screen().mouse_protocol_encoding(),
        vt100::MouseProtocolEncoding::Default
    );
}

#[test]
fn attachment_paste_framing_is_independent_of_child_mode() {
    use herdr_process_application::{Effect, InputRouter};
    use herdr_process_domain::{Action, Binding};
    use std::time::Duration;
    let terminal = Terminal::new(6, 30);
    let attachment = attach(&terminal, 6, 30);
    // Herdr's public paste path supplies delimiters only when its pane enables them.
    let input = if attachment.screen().bracketed_paste() {
        b"\x1b[200~ak\x1b[201~".as_slice()
    } else {
        b"ak".as_slice()
    };
    let mut router = InputRouter::new(
        b"a".to_vec(),
        vec![Binding {
            chord: b"k".to_vec(),
            profile: "shell".into(),
            action: Action::Kill,
        }],
        Duration::from_millis(100),
    )
    .unwrap();
    let effects = router.feed(input, Duration::ZERO);
    assert!(
        effects
            .iter()
            .all(|effect| matches!(effect, Effect::Forward(_))),
        "pasting invoked {effects:?}"
    );
    assert!(attachment.screen().bracketed_paste());
}

#[test]
fn shared_arrow_shortcuts_route_in_both_terminal_cursor_modes() {
    use crate::Configuration;
    use herdr_process_application::{Effect, InputRouter};
    use herdr_process_domain::Action;
    use std::{path::Path, time::Duration};
    for prefix in ["ctrl+a", "up"] {
        let config = Configuration::parse(
            "[windows.shell]\nprogram='bash'\ncwd='/'\nwidth=80\nheight=80\nctrl_c='hide'",
            &format!("[keys]\nprefix='{prefix}'\n[[keys.command]]\nkey='prefix+down'\ntype='plugin_action'\ncommand='herdr-process.shell:toggle-float'"),
            Path::new("/"),
        ).unwrap();
        for application in [false, true] {
            let mut terminal = Terminal::new(6, 30);
            if application {
                terminal.feed(b"\x1b[?1h");
            }
            let attachment = attach(&terminal, 6, 30);
            let arrow = if attachment.screen().application_cursor() {
                b'O'
            } else {
                b'['
            };
            let prefix_bytes = if prefix == "up" {
                vec![27, arrow, b'A']
            } else {
                vec![1]
            };
            let mut input = prefix_bytes.clone();
            input.extend([27, arrow, b'B']);
            for split in 0..=input.len() {
                let mut router = InputRouter::new(
                    config.prefix().to_vec(),
                    config.bindings().to_vec(),
                    Duration::from_millis(100),
                )
                .unwrap();
                let mut effects = router.feed(&input[..split], Duration::ZERO);
                effects.extend(router.feed(&input[split..], Duration::ZERO));
                assert_eq!(
                    effects,
                    vec![Effect::Invoke {
                        profile: "shell".into(),
                        action: Action::ToggleFloat
                    }],
                    "{prefix} application={application} split={split}"
                );
                // An unmatched key must retain the actual bytes supplied by Herdr.
                let mut unmatched = prefix_bytes.clone();
                unmatched.extend([27, arrow, b'C']);
                let mut forwarded = router.feed(&unmatched, Duration::ZERO);
                forwarded.extend(router.expire(Duration::from_millis(100)));
                assert_eq!(forwarded, vec![Effect::Forward(unmatched)]);
            }
        }
    }
}
