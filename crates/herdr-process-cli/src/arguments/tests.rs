use super::*;

fn args(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).into()).collect()
}

#[test]
fn explicit_paths_and_profile_actions_survive_argument_adaptation() {
    let actual = parse(&args(&[
        "action",
        "scratch",
        "split-below",
        "--profiles",
        "/private path/p.toml",
        "--herdr-config",
        "/private path/h.toml",
    ]))
    .unwrap();
    assert_eq!(
        actual,
        Arguments::Action {
            options: Options {
                profiles: Some("/private path/p.toml".into()),
                herdr: Some("/private path/h.toml".into())
            },
            profile: "scratch".into(),
            action: Action::SplitBelow
        }
    );
    assert_eq!(
        parse(&args(&["generate", "--output", "/private path/plugin"])).unwrap(),
        Arguments::Generate {
            options: Options::default(),
            output: "/private path/plugin".into()
        }
    );
    assert_eq!(
        parse(&args(&["manager", "--runtime-dir", "/private endpoint"])).unwrap(),
        Arguments::Manager {
            options: Options::default(),
            runtime: "/private endpoint".into()
        }
    );
}

#[test]
fn invalid_flags_never_fall_through_to_launch_or_generation() {
    for words in [
        vec!["action", "scratch", "toggle"],
        vec!["action", "scratch", "kill", "--output", "/tmp"],
        vec!["generate"],
        vec!["generate", "--output"],
        vec!["generate", "--output", "a", "--output", "b"],
        vec!["attach", "--profiles", "a"],
        vec!["manager"],
        vec!["manager", "--runtime-dir", "a", "--unknown"],
        vec![
            "action",
            "scratch",
            "kill",
            "--profiles",
            "a",
            "--profiles",
            "b",
        ],
        vec!["action", "scratch", "kill", "extra"],
        vec!["supervise", "socket"],
    ] {
        assert!(parse(&args(&words)).is_err(), "{words:?}");
    }
}

#[test]
fn internal_capabilities_are_literal_and_help_requires_no_configuration() {
    assert_eq!(
        parse(&args(&["supervise", "/tmp/private socket", "capability"])).unwrap(),
        Arguments::Supervise(args(&["/tmp/private socket", "capability"]))
    );
    assert_eq!(parse(&args(&["attach"])).unwrap(), Arguments::Attach);
    assert_eq!(parse(&args(&["--help"])).unwrap(), Arguments::Help);
}
