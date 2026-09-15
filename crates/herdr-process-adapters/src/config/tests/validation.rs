use super::*;
#[test]
fn malformed_profiles_and_unsupported_keys_fail_closed() {
    let home = Path::new("/private/home");
    let bad_profiles = [
        PROFILES.replace("program = \"editor\"", "program = ' '"),
        PROFILES.replace("program = \"editor\"", r#"program = "a\u0000b""#),
        PROFILES.replace(
            "args = ['a \"quoted\" value', 'c:\\work\\file', '']",
            "args='not an array'",
        ),
        PROFILES.replace(
            "args = ['a \"quoted\" value', 'c:\\work\\file', '']",
            "args=[1]",
        ),
        PROFILES.replace("cwd = \"~/projects\"", "cwd='relative'"),
        PROFILES.replace("cwd = \"~/projects\"", "cwd='~someone/project'"),
        PROFILES.replace("width = \"80%\"", "width=0"),
        PROFILES.replace("width = \"80%\"", "width=101"),
        PROFILES.replace("width = \"80%\"", "width='2.5%'"),
        PROFILES.replace("width = \"80%\"", "width='80'"),
        PROFILES.replace("width = \"80%\"", "width=' 80%'"),
        PROFILES.replace("ctrl_c = \"hide\"", "ctrl_c='interrupt'"),
        PROFILES.replace("windows.editor", "windows.'bad.name'"),
        PROFILES.replace("windows.editor", "windows.'bad:name'"),
        PROFILES.replace("windows.editor", &format!("windows.{}", "x".repeat(108))),
    ];
    for text in bad_profiles {
        assert!(
            Configuration::parse(&text, HERDR, home).is_err(),
            "accepted {text}"
        );
    }
    for key in [
        "prefix+ctrl+shift+a",
        "prefix+ctrl+i",
        "prefix+ctrl+m",
        "prefix+cmd+r",
        "prefix+f13",
        "prefix+shift+1",
        "prefix+shift+enter",
        "prefix+home",
        "prefix+ctrl+c",
        "prefix+esc",
        "prefix+ctrl+b",
        "prefix+a+b",
        "prefix+",
        "R",
    ] {
        let herdr = format!(
            "{HERDR}[[keys.command]]\nkey='{key}'\ntype='plugin_action'\ncommand='herdr-process.editor:kill'"
        );
        assert!(
            Configuration::parse(PROFILES, &herdr, home).is_err(),
            "accepted {key}"
        );
    }
    for command in [
        "herdr-process.missing:kill",
        "herdr-process.editor:hide",
        "herdr-process.editor.kill",
    ] {
        let herdr = format!(
            "{HERDR}[[keys.command]]\nkey='prefix+R'\ntype='plugin_action'\ncommand='{command}'"
        );
        assert!(
            Configuration::parse(PROFILES, &herdr, home).is_err(),
            "accepted {command}"
        );
    }
    for herdr in [
        "",
        "[keys]",
        "[keys]\nprefix='ctrl+c'",
        "[keys]\nprefix='esc'",
    ] {
        assert!(Configuration::parse(PROFILES, herdr, home).is_err());
    }
}

#[test]
fn duplicate_aliases_and_other_commands_cannot_steal_owned_chords() {
    let own = "[[keys.command]]\nkey='prefix+R'\ntype='plugin_action'\ncommand='herdr-process.editor:kill'\n";
    for other in [
        "[[keys.command]]\nkey='prefix+shift+r'\ncommand='unrelated'\ntype='shell'\n",
        "[[keys.command]]\nkey=['prefix+X','prefix+R']\ncommand='other.plugin'\ntype='plugin_action'\n",
        own,
    ] {
        for commands in [format!("{own}{other}"), format!("{other}{own}")] {
            assert!(
                Configuration::parse(
                    PROFILES,
                    &format!("{HERDR}{commands}"),
                    Path::new("/private/home")
                )
                .is_err()
            );
        }
    }
    let herdr = format!(
        "{HERDR}[[keys.command]]\nkey=['prefix+R','prefix+shift+r']\ntype='plugin_action'\ncommand='herdr-process.editor:kill'"
    );
    assert!(Configuration::parse(PROFILES, &herdr, Path::new("/private/home")).is_err());
}

#[test]
fn command_schema_and_native_whitespace_are_not_silently_ignored() {
    let command = "[[keys.command]]\nkey='prefix+R'\ntype='plugin_action'\ncommand=' herdr-process.editor:kill '\n";
    let c = Configuration::parse(
        PROFILES,
        &format!("{HERDR}{command}"),
        Path::new("/private/home"),
    )
    .unwrap();
    assert_eq!(c.bindings().len(), 1);
    for text in [
        command.replace("type='plugin_action'", "type='typo'"),
        command.replace("key='prefix+R'", "key=[1]"),
        command.replace("key='prefix+R'", "key=[]"),
        command.replace("key='prefix+R'", "key=''"),
        command.replace("command=' herdr-process.editor:kill '", "command=12"),
        command.replace("command=' herdr-process.editor:kill '", "command=''"),
        "[[keys.command]]\nkey='prefix+R'\ntype='typo'\ncommand='unrelated'".into(),
    ] {
        assert!(
            Configuration::parse(
                PROFILES,
                &format!("{HERDR}{text}"),
                Path::new("/private/home")
            )
            .is_err(),
            "accepted {text}"
        );
    }
}
