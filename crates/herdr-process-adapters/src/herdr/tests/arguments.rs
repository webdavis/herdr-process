use super::*;
use crate::herdr::arguments::{focus_arguments, open_arguments};
use std::ffi::OsStr;

fn words(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

#[test]
fn open_builds_the_exact_public_call_for_each_placement() {
    let target = Target {
        workspace: "workspace-2".into(),
        pane: "pane-3".into(),
    };
    for (split, argv, expected) in [
        (
            None,
            vec![
                "plugin",
                "pane",
                "open",
                "--plugin",
                "herdr-process",
                "--entrypoint",
                "attach",
                "--placement",
                "popup",
                "--width",
                "80%",
                "--height",
                "70%",
                "--cwd",
                "/private/tmp/dotfiles-modernization/exact cwd",
                "--env",
                "HERDR_PROCESS_SOCKET=/private/tmp/dotfiles-modernization/manager socket",
                "--env",
                "HERDR_PROCESS_PROFILE=a profile; $HOME",
                "--env",
                "HERDR_PROCESS_TICKET=private-ticket",
            ],
            Expected::Popup,
        ),
        (
            Some((target.clone(), Direction::Right)),
            vec![
                "plugin",
                "pane",
                "open",
                "--plugin",
                "herdr-process",
                "--entrypoint",
                "attach",
                "--placement",
                "split",
                "--workspace",
                "workspace-2",
                "--target-pane",
                "pane-3",
                "--direction",
                "right",
                "--cwd",
                "/private/tmp/dotfiles-modernization/exact cwd",
                "--env",
                "HERDR_PROCESS_SOCKET=/private/tmp/dotfiles-modernization/manager socket",
                "--env",
                "HERDR_PROCESS_PROFILE=a profile; $HOME",
                "--env",
                "HERDR_PROCESS_TICKET=private-ticket",
            ],
            Expected::Split,
        ),
        (
            Some((target, Direction::Below)),
            vec![
                "plugin",
                "pane",
                "open",
                "--plugin",
                "herdr-process",
                "--entrypoint",
                "attach",
                "--placement",
                "split",
                "--workspace",
                "workspace-2",
                "--target-pane",
                "pane-3",
                "--direction",
                "down",
                "--cwd",
                "/private/tmp/dotfiles-modernization/exact cwd",
                "--env",
                "HERDR_PROCESS_SOCKET=/private/tmp/dotfiles-modernization/manager socket",
                "--env",
                "HERDR_PROCESS_PROFILE=a profile; $HOME",
                "--env",
                "HERDR_PROCESS_TICKET=private-ticket",
            ],
            Expected::Split,
        ),
    ] {
        let mut request = view();
        request.split = split;
        assert_eq!(open_arguments(&request).unwrap(), (words(&argv), expected));
    }
}

#[test]
fn focus_builds_the_public_call_and_expects_the_same_pane() {
    assert_eq!(
        focus_arguments("pane-7"),
        (
            words(&["plugin", "pane", "focus", "pane-7"]),
            Expected::Focus("pane-7".into())
        )
    );
}

#[test]
fn host_command_runs_the_binary_against_its_socket() {
    let host = Herdr {
        binary: "/private/bin/herdr".into(),
        socket: "/private/host.sock".into(),
    };
    let command = host.command(words(&["plugin", "pane", "focus", "pane-7"]));
    assert_eq!(command.get_program(), "/private/bin/herdr");
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["plugin", "pane", "focus", "pane-7"]
    );
    assert_eq!(
        command.get_envs().collect::<Vec<_>>(),
        [(
            OsStr::new("HERDR_SOCKET_PATH"),
            Some(OsStr::new("/private/host.sock"))
        )]
    );
}
