use super::*;
fn values<'a>(entries: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |key| {
        entries
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value.into())
    }
}
#[test]
fn explicit_paths_win_and_environment_path_is_verbatim() {
    let entries = [
        ("HOME", "/private/home"),
        ("HERDR_CONFIG_PATH", "relative/custom.toml"),
        ("XDG_CONFIG_HOME", "/private/xdg"),
    ];
    let env = Environment::read(&Options::default(), values(&entries)).unwrap();
    assert_eq!(env.home, PathBuf::from("/private/home"));
    assert_eq!(env.paths.herdr, PathBuf::from("relative/custom.toml"));
    assert_eq!(env.paths.profiles, PathBuf::from("relative/processes.toml"));
    let options = Options {
        herdr: Some("chosen/file.toml".into()),
        profiles: Some("separate/profiles.toml".into()),
    };
    let env = Environment::read(&options, values(&entries)).unwrap();
    assert_eq!(env.paths.herdr, PathBuf::from("chosen/file.toml"));
    assert_eq!(env.paths.profiles, PathBuf::from("separate/profiles.toml"));
}
#[test]
fn defaults_and_home_validation() {
    let env = Environment::read(
        &Options::default(),
        values(&[
            ("HOME", "/private/home"),
            ("XDG_CONFIG_HOME", "/private/xdg"),
        ]),
    )
    .unwrap();
    assert_eq!(
        env.paths.herdr,
        PathBuf::from("/private/xdg/herdr/config.toml")
    );
    let env = Environment::read(&Options::default(), values(&[("HOME", "/private/home")])).unwrap();
    assert_eq!(
        env.paths.profiles,
        PathBuf::from("/private/home/.config/herdr/processes.toml")
    );
    assert!(Environment::read(&Options::default(), values(&[])).is_err());
    assert!(Environment::read(&Options::default(), values(&[("HOME", "relative")])).is_err());
}
#[test]
fn exact_host_and_json_target_with_field_fallback() {
    let entries = [
        ("HERDR_BIN_PATH", "/private/bin/custom host"),
        ("HERDR_SOCKET_PATH", "/private/host.sock"),
        (
            "HERDR_PLUGIN_CONTEXT_JSON",
            r#"{"workspace_id":"json-w","focused_pane_id":"json-p"}"#,
        ),
        ("HERDR_WORKSPACE_ID", "fallback-w"),
        ("HERDR_PANE_ID", "fallback-p"),
    ];
    let selected = host(values(&entries)).unwrap();
    assert_eq!(selected.binary, PathBuf::from("/private/bin/custom host"));
    assert_eq!(selected.socket, PathBuf::from("/private/host.sock"));
    assert_eq!(
        target(values(&entries)).unwrap(),
        Target {
            workspace: "json-w".into(),
            pane: "json-p".into()
        }
    );
    let target = target(values(&[
        (
            "HERDR_PLUGIN_CONTEXT_JSON",
            r#"{"workspace_id":"json-w","focused_pane_id":null}"#,
        ),
        ("HERDR_PANE_ID", "fallback-p"),
    ]))
    .unwrap();
    assert_eq!(target.workspace, "json-w");
    assert_eq!(target.pane, "fallback-p");
}
#[test]
fn malformed_context_is_rejected_and_absent_target_is_empty() {
    assert!(target(values(&[("HERDR_PLUGIN_CONTEXT_JSON", "invalid")])).is_err());
    assert_eq!(
        target(values(&[])).unwrap(),
        Target {
            workspace: String::new(),
            pane: String::new()
        }
    );
    assert!(host(values(&[])).is_err());
}
#[test]
fn endpoint_identity_separates_host_and_configuration_paths() {
    let mut host = Herdr {
        binary: "/private/bin/herdr".into(),
        socket: "/private/host1".into(),
    };
    let mut paths = ConfigurationPaths {
        profiles: "/private/profiles1".into(),
        herdr: "/private/herdr1".into(),
    };
    let first = runtime(&host, &paths);
    assert_eq!(first, runtime(&host, &paths));
    assert!(first.as_os_str().len() < 75);
    host.socket = "/private/host2".into();
    assert_ne!(first, runtime(&host, &paths));
    host.socket = "/private/host1".into();
    paths.profiles = "/private/profiles2".into();
    assert_ne!(first, runtime(&host, &paths));
    paths.profiles = "/private/profiles1".into();
    paths.herdr = "/private/herdr2".into();
    assert_ne!(first, runtime(&host, &paths));
}

#[test]
fn context_must_be_an_object_when_supplied() {
    for invalid in ["null", "[]", "42", "\"context\""] {
        assert!(
            target(values(&[("HERDR_PLUGIN_CONTEXT_JSON", invalid)])).is_err(),
            "{invalid}"
        );
    }
}
