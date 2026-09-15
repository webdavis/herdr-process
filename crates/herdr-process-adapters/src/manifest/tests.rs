use super::*;
use std::path::Path;
#[test]
fn manifest_has_native_entrypoint_and_four_array_actions_per_profile() {
    let config = Configuration::parse(
        r#"[windows.work]
program = 'a "quoted" program'
args = ['"x"', 'c:\file']
cwd = '/private/project'
width = 100
height = '1%'
ctrl_c = 'quit'
"#,
        "[keys]\nprefix='ctrl+b'",
        Path::new("/private/home"),
    )
    .unwrap();
    let manifest: toml::Value = toml::from_str(&config.render_manifest().unwrap()).unwrap();
    assert_eq!(manifest["id"].as_str(), Some("herdr-process"));
    assert_eq!(manifest["min_herdr_version"].as_str(), Some("0.9.0"));
    assert_eq!(
        manifest["platforms"].as_array().unwrap(),
        &[toml::Value::from("macos")]
    );
    let build: Vec<String> = manifest["build"][0]["command"].clone().try_into().unwrap();
    assert_eq!(build, ["cargo", "build", "--release", "--locked"]);
    assert_eq!(manifest["panes"][0]["id"].as_str(), Some("attach"));
    let attach: Vec<String> = manifest["panes"][0]["command"].clone().try_into().unwrap();
    assert_eq!(attach, ["./target/release/herdr-process", "attach"]);
    let actions = manifest["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 4);
    for (row, action) in actions
        .iter()
        .zip(["split-right", "split-below", "toggle-float", "kill"])
    {
        assert_eq!(row["id"].as_str().unwrap(), format!("work:{action}"));
        let command: Vec<String> = row["command"].clone().try_into().unwrap();
        assert_eq!(
            command,
            ["./target/release/herdr-process", "action", "work", action]
        );
        assert!(row.get("key").is_none());
    }
}

#[test]
fn selected_configuration_activates_only_its_generated_manifest() {
    let files = crate::test_support::TempRoot::new();
    let home = files.path().join("home");
    std::fs::create_dir(&home).unwrap();
    let profiles = files.path().join("processes.toml");
    let selected = files.path().join("alternate.toml");
    let unrelated = files.path().join("config.toml");
    let destination = files.path().join("herdr-plugin.toml");
    let profile_text = "[windows.job]\nprogram='worker'\nargs=['a \"quoted\" arg', 'c:\\path']\ncwd='~/work'\nwidth=80\nheight='75%'\nctrl_c='quit'";
    let herdr_text = "[theme]\nname='untouched'\n[keys]\nprefix='ctrl+d'\n[[keys.command]]\nkey='prefix+R'\ntype='plugin_action'\ncommand='herdr-process.job:kill'";
    std::fs::write(&profiles, profile_text).unwrap();
    std::fs::write(&selected, herdr_text).unwrap();
    std::fs::write(&unrelated, "invalid unselected config").unwrap();
    std::fs::write(&destination, "previous manifest").unwrap();
    let c = Configuration::load(&profiles, &selected, &home).unwrap();
    assert_eq!(c.prefix(), &[4]);
    assert_eq!(c.profiles()["job"].args, ["a \"quoted\" arg", r"c:\path"]);
    assert_eq!(c.profiles()["job"].cwd, home.join("work"));
    c.write_manifest(files.path()).unwrap();
    assert_eq!(
        std::fs::read_to_string(&destination).unwrap(),
        c.render_manifest().unwrap()
    );
    assert_eq!(std::fs::read_to_string(&profiles).unwrap(), profile_text);
    assert_eq!(std::fs::read_to_string(&selected).unwrap(), herdr_text);
    assert_eq!(
        std::fs::read_to_string(&unrelated).unwrap(),
        "invalid unselected config"
    );
    std::fs::write(
        &selected,
        herdr_text
            .replace("ctrl+d", "ctrl+e")
            .replace("prefix+R", "prefix+T")
            .replace(":kill", ":split-right"),
    )
    .unwrap();
    let changed = Configuration::load(&profiles, &selected, &home).unwrap();
    assert_ne!(changed.identity(), c.identity());
    let native = changed
        .resolve_action("herdr-process.job:split-right")
        .unwrap();
    assert_eq!(
        native,
        ("job".into(), herdr_process_domain::Action::SplitRight)
    );
    let mut router = herdr_process_application::InputRouter::new(
        changed.prefix().to_vec(),
        changed.bindings().to_vec(),
        std::time::Duration::from_millis(100),
    )
    .unwrap();
    assert_eq!(
        router.feed(b"\x05T", std::time::Duration::ZERO),
        vec![herdr_process_application::Effect::Invoke {
            profile: native.0,
            action: native.1
        }]
    );
    assert_eq!(
        router.feed(b"\x04R", std::time::Duration::ZERO),
        vec![herdr_process_application::Effect::Forward(
            b"\x04R".to_vec()
        )]
    );
    std::fs::write(&selected, herdr_text).unwrap();
    for bad in [
        "not toml",
        "[windows.job]",
        &profile_text.replace("width=80", "width=101"),
        &profile_text.replace("ctrl_c='quit'", "ctrl_c='ignore'"),
    ] {
        std::fs::write(&profiles, bad).unwrap();
        let activation = Configuration::load(&profiles, &selected, &home)
            .and_then(|c| c.write_manifest(files.path()));
        assert!(activation.is_err());
        assert_eq!(
            std::fs::read_to_string(&destination).unwrap(),
            c.render_manifest().unwrap()
        );
    }
    std::fs::write(&profiles, profile_text).unwrap();
    for bad in [
        "[keys",
        "[keys]",
        &herdr_text.replace("job:kill", "missing:kill"),
        &herdr_text.replace("prefix+R", "prefix+cmd+R"),
    ] {
        std::fs::write(&selected, bad).unwrap();
        assert!(
            Configuration::load(&profiles, &selected, &home)
                .and_then(|c| c.write_manifest(files.path()))
                .is_err()
        );
        assert_eq!(
            std::fs::read_to_string(&destination).unwrap(),
            c.render_manifest().unwrap()
        );
    }
}

#[test]
fn partial_write_failure_retains_previous_manifest_bytes() {
    let files = crate::test_support::TempRoot::new();
    let destination = files.path().join("herdr-plugin.toml");
    std::fs::write(&destination, b"previous generation").unwrap();
    let failure = replace_manifest(files.path(), b"next generation", |file, bytes| {
        file.write_all(&bytes[..4])?;
        Err(io::Error::other("injected disk full"))
    });
    assert!(failure.is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"previous generation");
    assert_eq!(std::fs::read_dir(files.path()).unwrap().count(), 1);
    assert!(
        replace_manifest(&files.path().join("missing"), b"new", |f, b| f.write_all(b)).is_err()
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"previous generation");
}
