use super::{OpenView, reply::Expected};
use herdr_process_domain::Direction;
use std::ffi::OsString;

pub(super) fn open_arguments(view: &OpenView) -> anyhow::Result<(Vec<OsString>, Expected)> {
    let mut arguments = words(&[
        "plugin",
        "pane",
        "open",
        "--plugin",
        "herdr-process",
        "--entrypoint",
        "attach",
    ]);
    let expected = match &view.split {
        Some((target, direction)) => {
            arguments.extend(words(&[
                "--placement",
                "split",
                "--workspace",
                &target.workspace,
                "--target-pane",
                &target.pane,
                "--direction",
                match direction {
                    Direction::Right => "right",
                    Direction::Below => "down",
                },
            ]));
            Expected::Split
        }
        None => {
            anyhow::ensure!(
                (1..=100).contains(&view.width) && (1..=100).contains(&view.height),
                "invalid popup dimensions"
            );
            arguments.extend(words(&[
                "--placement",
                "popup",
                "--width",
                &format!("{}%", view.width),
                "--height",
                &format!("{}%", view.height),
            ]));
            Expected::Popup
        }
    };
    let mut socket = OsString::from("HERDR_PROCESS_SOCKET=");
    socket.push(&view.manager_socket);
    arguments.extend([
        "--cwd".into(),
        view.cwd.clone().into_os_string(),
        "--env".into(),
        socket,
        "--env".into(),
        format!("HERDR_PROCESS_PROFILE={}", view.profile).into(),
        "--env".into(),
        format!("HERDR_PROCESS_TICKET={}", view.ticket).into(),
    ]);
    Ok((arguments, expected))
}

pub(super) fn focus_arguments(pane: &str) -> (Vec<OsString>, Expected) {
    (
        words(&["plugin", "pane", "focus", pane]),
        Expected::Focus(pane.to_owned()),
    )
}

fn words(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}
