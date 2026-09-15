use super::*;

const OWNED: &str =
    "[[keys.command]]\nkey='prefix+R'\ntype='plugin_action'\ncommand='herdr-process.editor:kill'\n";

fn parse(herdr: &str) -> Result<Configuration, anyhow::Error> {
    Configuration::parse(PROFILES, herdr, Path::new("/private/home"))
}

#[test]
fn foreign_chords_the_legacy_encoder_cannot_represent_are_skipped() {
    for foreign in [
        "focus_agent='prefix+ctrl+1..9'\n",
        "[keys.indexed]\nfocus_agent='prefix+ctrl'\n",
        "[[keys.command]]\nkey='prefix+ctrl+1'\ntype='plugin_action'\ncommand='other.plugin'\n",
        "[[keys.command]]\nkey='prefix+ctrl+.'\ntype='plugin_action'\ncommand='other.plugin'\n",
        "[[keys.command]]\nkey=['prefix+ctrl+.','prefix+.']\ntype='plugin_action'\ncommand='other.plugin'\n",
        "navigate_workspace_up='ctrl+.'\n",
    ] {
        let configuration = parse(&format!("{HERDR}{foreign}{OWNED}"))
            .unwrap_or_else(|error| panic!("rejected {foreign}: {error:#}"));
        assert_eq!(configuration.bindings().len(), 1, "{foreign}");
    }
}

#[test]
fn foreign_chords_the_legacy_encoder_can_represent_still_collide() {
    // The reported chord is the second spelling registered for the same bytes.
    for (herdr, chord) in [
        (format!("{HERDR}help='prefix+shift+r'\n{OWNED}"), "prefix+R"),
        (
            format!("{HERDR}goto='prefix+R'\nhelp='prefix+shift+r'\n"),
            "prefix+shift+r",
        ),
        (
            format!(
                "{HERDR}{OWNED}[[keys.command]]\nkey='prefix+shift+r'\ntype='shell'\ncommand='unrelated'\n"
            ),
            "prefix+shift+r",
        ),
    ] {
        let error = parse(&herdr).expect_err(&herdr).to_string();
        assert_eq!(
            error,
            format!("duplicate or conflicting native chord {chord:?}"),
            "{herdr}"
        );
    }
}

#[test]
fn owned_bindings_with_unrepresentable_chords_still_fail_loudly() {
    for key in ["prefix+ctrl+1", "prefix+ctrl+."] {
        let owned = format!(
            "[[keys.command]]\nkey='{key}'\ntype='plugin_action'\ncommand='herdr-process.editor:kill'\n"
        );
        let error = format!("{:#}", parse(&format!("{HERDR}{owned}")).expect_err(key));
        assert!(error.contains(key), "{key}: {error}");
    }
}
