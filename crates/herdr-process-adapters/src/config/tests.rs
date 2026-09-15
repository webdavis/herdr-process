use super::*;
const PROFILES: &str = r#"[windows.editor]
program = "editor"
args = ['a "quoted" value', 'c:\work\file', '']
cwd = "~/projects"
width = "80%"
height = 65
ctrl_c = "hide"
"#;
const HERDR: &str = "[keys]\nprefix='ctrl+b'\n";
#[test]
fn profiles_preserve_arguments_and_resolve_home() {
    let c = Configuration::parse(PROFILES, HERDR, Path::new("/private/home")).unwrap();
    assert_eq!(
        c.profiles()["editor"],
        Profile {
            program: "editor".into(),
            args: vec![
                "a \"quoted\" value".into(),
                r"c:\work\file".into(),
                "".into()
            ],
            cwd: PathBuf::from("/private/home/projects"),
            width: 80,
            height: 65,
            ctrl_c: CtrlC::Hide
        }
    );
}

#[test]
fn native_key_source_routes_verified_legacy_encodings() {
    use herdr_process_application::{Effect, InputRouter};
    use herdr_process_domain::Action;
    use std::time::Duration;
    let cases: &[(&str, &[u8])] = &[
        ("ctrl+a", b"\x01"),
        ("ctrl+h", b"\x08"),
        ("shift+r", b"R"),
        ("alt+x", b"\x1bx"),
        ("up", b"\x1b[A"),
        ("ctrl+left", b"\x1b[1;5D"),
        ("f1", b"\x1bOP"),
        ("shift+f5", b"\x1b[15;2~"),
        ("shift+tab", b"\x1b[Z"),
    ];
    for (key, bytes) in cases {
        let herdr = format!(
            "{HERDR}[[keys.command]]\nkey='prefix+{key}'\ntype='plugin_action'\ncommand='herdr-process.editor:split-below'\n"
        );
        let c = Configuration::parse(PROFILES, &herdr, Path::new("/private/home")).unwrap();
        assert_eq!(c.prefix(), b"\x02");
        let mut r = InputRouter::new(
            c.prefix().to_vec(),
            c.bindings().to_vec(),
            Duration::from_millis(100),
        )
        .unwrap();
        let input = [b"\x02".as_slice(), bytes].concat();
        assert_eq!(
            r.feed(&input, Duration::ZERO),
            vec![Effect::Invoke {
                profile: "editor".into(),
                action: Action::SplitBelow
            }],
            "{key}"
        );
    }
}

#[test]
fn rejects_conflicts_with_native_actions_and_interrupt() {
    for extra in ["help='prefix+R'", "help=['prefix+r','prefix+shift+r']"] {
        let herdr = format!(
            "{HERDR}{extra}\n[[keys.command]]\nkey='prefix+R'\ntype='plugin_action'\ncommand='herdr-process.editor:kill'\n"
        );
        assert!(
            Configuration::parse(PROFILES, &herdr, Path::new("/private/home")).is_err(),
            "{extra}"
        );
    }
}

#[test]
fn selected_paths_follow_release_xdg_and_verbatim_override() {
    let home = Path::new("/private/home");
    let default = resolve_configuration_paths(None, None, None, home);
    assert_eq!(default.herdr, home.join(".config/herdr/config.toml"));
    assert_eq!(default.profiles, home.join(".config/herdr/processes.toml"));
    let xdg = resolve_configuration_paths(None, None, Some(Path::new("/private/xdg")), home);
    assert_eq!(xdg.herdr, Path::new("/private/xdg/herdr/config.toml"));
    let selected = resolve_configuration_paths(
        None,
        Some(Path::new("relative/selected.toml")),
        Some(Path::new("/ignored")),
        home,
    );
    assert_eq!(selected.herdr, Path::new("relative/selected.toml"));
    assert_eq!(selected.profiles, Path::new("relative/processes.toml"));
    let explicit = resolve_configuration_paths(
        Some(Path::new("chosen.toml")),
        Some(Path::new("")),
        None,
        home,
    );
    assert_eq!(explicit.herdr, Path::new(""));
    assert_eq!(explicit.profiles, Path::new("chosen.toml"));
}

#[test]
fn identity_tracks_validated_profile_and_binding_sources() {
    let home = Path::new("/private/home");
    let original = Configuration::parse(PROFILES, HERDR, home).unwrap();
    assert!(!original.identity().is_empty());
    assert_eq!(
        original.identity(),
        Configuration::parse(PROFILES, HERDR, home)
            .unwrap()
            .identity()
    );
    for changed in [
        PROFILES.replace("80%", "81%"),
        PROFILES.replace("editor", "other"),
        PROFILES.replace("hide", "quit"),
        PROFILES.replace("projects", "elsewhere"),
    ] {
        assert_ne!(
            original.identity(),
            Configuration::parse(&changed, HERDR, home)
                .unwrap()
                .identity()
        );
    }
    assert_ne!(
        original.identity(),
        Configuration::parse(PROFILES, "[keys]\nprefix='ctrl+d'", home)
            .unwrap()
            .identity()
    );
    assert_ne!(
        original.identity(),
        Configuration::parse(PROFILES, HERDR, Path::new("/private/other-home"))
            .unwrap()
            .identity()
    );
}

#[test]
fn forty_profiles_invoke_all_four_native_manifest_actions_from_focused_input() {
    use herdr_process_application::{Effect, InputRouter};
    use herdr_process_domain::Action;
    use std::time::{Duration, Instant};
    let started = Instant::now();
    let mut profiles = String::new();
    let mut herdr = HERDR.to_string();
    let actions = [
        ("split-right", Action::SplitRight),
        ("split-below", Action::SplitBelow),
        ("toggle-float", Action::ToggleFloat),
        ("kill", Action::Kill),
    ];
    let mut expected = Vec::new();
    for i in 0..40 {
        profiles.push_str(&format!("[windows.job{i}]\nprogram='worker'\nargs=[]\ncwd='/private/work'\nwidth=80\nheight=70\nctrl_c='hide'\n"));
        for (j, (name, action)) in actions.iter().enumerate() {
            let key = char::from_u32(0x400 + i * 4 + j as u32).unwrap();
            herdr.push_str(&format!("[[keys.command]]\nkey='prefix+{key}'\ntype='plugin_action'\ncommand='herdr-process.job{i}:{name}'\n"));
            expected.push((key, format!("job{i}"), *name, *action));
        }
    }
    let c = Configuration::parse(&profiles, &herdr, Path::new("/private/home")).unwrap();
    let manifest: toml::Value = toml::from_str(&c.render_manifest().unwrap()).unwrap();
    let entries = manifest["actions"].as_array().unwrap();
    assert_eq!(entries.len(), 160);
    let mut router = InputRouter::new(
        c.prefix().to_vec(),
        c.bindings().to_vec(),
        Duration::from_millis(100),
    )
    .unwrap();
    for (key, profile, spelling, action) in expected {
        let local = format!("{profile}:{spelling}");
        let entry = entries
            .iter()
            .find(|entry| entry["id"].as_str() == Some(&local))
            .unwrap();
        let argv: Vec<String> = entry["command"].clone().try_into().unwrap();
        assert_eq!(
            argv,
            [
                "./target/release/herdr-process",
                "action",
                &profile,
                spelling
            ]
        );
        let native = c.resolve_action(&format!("herdr-process.{local}")).unwrap();
        assert_eq!(native, (profile.clone(), action));
        let mut effects = router.feed(&[2], Duration::ZERO);
        for byte in key.to_string().as_bytes() {
            effects.extend(router.feed(&[*byte], Duration::ZERO));
        }
        assert_eq!(effects, vec![Effect::Invoke { profile, action }]);
    }
    assert!(started.elapsed() < Duration::from_secs(1));
}

mod validation;

mod chords;
