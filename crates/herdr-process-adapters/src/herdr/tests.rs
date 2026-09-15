use super::*;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
// A run-unique base: the shell double and its socket outlive the call that
// builds them, so this directory is not removed at the end of the test, and a
// name keyed only on the reusable process identifier would collide with the
// one an earlier run left behind.
const POPUP: &str = r#"{"id":"request","result":{"type":"ok"}}"#;
const SPLIT: &str = r#"{"id":"request","result":{"type":"plugin_pane_opened","plugin_pane":{"plugin_id":"herdr-process","entrypoint":"attach","pane":{"pane_id":"pane-7"}}}}"#;
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn fixture(body: &str) -> Herdr {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::path::PathBuf::from(format!(
        "/tmp/hp-host-{}-{stamp:x}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    let binary = dir.join("herdr-double");
    fs::write(&binary, format!("#!/bin/bash\nset -euo pipefail\n{body}\n")).unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    Herdr {
        binary,
        socket: dir.join("private-host.sock"),
    }
}
fn view() -> OpenView {
    OpenView {
        profile: "a profile; $HOME".into(),
        ticket: "private-ticket".into(),
        manager_socket: "/private/tmp/dotfiles-modernization/manager socket".into(),
        cwd: "/private/tmp/dotfiles-modernization/exact cwd".into(),
        width: 80,
        height: 70,
        split: None,
    }
}
/// What a double in this file is allowed before it is called hung. EVERY ONE
/// OF THEM PRINTS ONE LINE AND EXITS, in single-digit milliseconds, so this
/// number bounds a hang and measures nothing: no amount of load takes a shell
/// that far, and the product's own half second stays where it belongs, in
/// `DEADLINE`.
const HANG_BOUND: Duration = Duration::from_secs(10);
/// The longest ONE `poll` may take. It is not a speed reading either: a poll
/// that blocked would park on the pipe until its double exited, which
/// `hanging_command_times_out_without_blocking_poll` holds open for ten
/// seconds, so a second of scheduling noise cannot reach this and a real
/// block cannot hide under it.
const POLL_BOUND: Duration = Duration::from_secs(1);
fn finish(call: &mut HostCall) -> Result<HostReply, HostFailure> {
    // THE DOUBLE'S OWN EXIT IS THE SIGNAL. The bound below only stops a hang,
    // so nothing here waits on the product deadline the double has to beat.
    call.bound(HANG_BOUND);
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < HANG_BOUND * 2, "host call stalled");
        let tick = Instant::now();
        let result = call.poll();
        assert!(tick.elapsed() < POLL_BOUND, "poll blocked");
        match result? {
            Some(reply) => return Ok(reply),
            None => std::thread::yield_now(),
        }
    }
}
fn emit(json: &str) -> String {
    format!("printf '%s' {}", quote(json))
}
fn checks(args: &[&str]) -> String {
    let mut script = format!("[ \"$#\" -eq {} ]\n", args.len());
    for arg in args {
        script.push_str(&format!("[ \"$1\" = {} ]; shift\n", quote(arg)));
    }
    script.push_str("[ \"$HERDR_SOCKET_PATH\" = \"${0%/*}/private-host.sock\" ]\n");
    script
}
#[test]
fn popup_executes_exact_public_call_and_has_no_pane() {
    let _speed = Speed::start("popup_executes_exact_public_call_and_has_no_pane");
    let script = checks(&[
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
    ]);
    let mut call = fixture(&(script + &emit(POPUP))).open(&view()).unwrap();
    assert_eq!(finish(&mut call), Ok(HostReply::Opened(None)));
    assert_eq!(call.poll(), Ok(Some(HostReply::Opened(None))));
}
#[test]
fn split_executes_targeted_call_and_returns_pane() {
    let _speed = Speed::start("split_executes_targeted_call_and_returns_pane");
    for (direction, spelling) in [(Direction::Right, "right"), (Direction::Below, "down")] {
        let script = checks(&[
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
            spelling,
            "--cwd",
            "/private/tmp/dotfiles-modernization/exact cwd",
            "--env",
            "HERDR_PROCESS_SOCKET=/private/tmp/dotfiles-modernization/manager socket",
            "--env",
            "HERDR_PROCESS_PROFILE=a profile; $HOME",
            "--env",
            "HERDR_PROCESS_TICKET=private-ticket",
        ]);
        let mut request = view();
        request.split = Some((
            Target {
                workspace: "workspace-2".into(),
                pane: "pane-3".into(),
            },
            direction,
        ));
        let mut call = fixture(&(script + &emit(SPLIT))).open(&request).unwrap();
        assert_eq!(
            finish(&mut call),
            Ok(HostReply::Opened(Some("pane-7".into())))
        );
    }
}
#[test]
fn focus_executes_public_call_and_checks_identity() {
    let _speed = Speed::start("focus_executes_public_call_and_checks_identity");
    let response = SPLIT.replace("plugin_pane_opened", "plugin_pane_focused");
    let script = checks(&["plugin", "pane", "focus", "pane-7"]) + &emit(&response);
    assert_eq!(
        finish(&mut fixture(&script).focus("pane-7").unwrap()),
        Ok(HostReply::Focused)
    );
    for wrong in [
        response.replace("pane-7", "pane-8"),
        response.replace("attach", "other"),
        response.replace("herdr-process", "other"),
    ] {
        assert_eq!(
            finish(&mut fixture(&emit(&wrong)).focus("pane-7").unwrap())
                .unwrap_err()
                .code,
            HostFailureCode::Malformed
        );
    }
}
#[test]
fn zero_exit_busy_is_typed_and_diagnostics_are_private() {
    let _speed = Speed::start("zero_exit_busy_is_typed_and_diagnostics_are_private");
    let json = r#"{"id":"r","error":{"code":"ui_busy","message":"password=secret"}}"#;
    let failure = finish(&mut fixture(&emit(json)).open(&view()).unwrap()).unwrap_err();
    assert!(failure.is_busy());
    assert!(!format!("{failure:?} {failure}").contains("secret"));
}
#[test]
fn captures_nonzero_stderr_and_rejected_json_without_secret_text() {
    let _speed = Speed::start("captures_nonzero_stderr_and_rejected_json_without_secret_text");
    for (script, code) in [
        ("printf 'secret' >&2; exit 3".into(), HostFailureCode::Exit),
        ("printf 'secret' >&2".into(), HostFailureCode::Exit),
        (
            emit(r#"{"id":"r","error":{"code":"denied","message":"secret"}}"#),
            HostFailureCode::Rejected,
        ),
    ] {
        let failure = finish(&mut fixture(&script).open(&view()).unwrap()).unwrap_err();
        assert_eq!(failure.code, code);
        assert!(!format!("{failure:?}").contains("secret"));
    }
}
#[test]
fn rejects_malformed_wrong_shape_and_wrong_identity() {
    let _speed = Speed::start("rejects_malformed_wrong_shape_and_wrong_identity");
    for json in [
        "secret",
        "{}",
        r#"{"id":"r","result":{"type":"plugin_pane_focused"}}"#,
        r#"{"id":"r","result":{"type":"ok"},"error":{"code":"ui_busy","message":"x"}}"#,
    ] {
        assert_eq!(
            finish(&mut fixture(&emit(json)).open(&view()).unwrap())
                .unwrap_err()
                .code,
            HostFailureCode::Malformed
        );
    }
    let mut request = view();
    request.split = Some((
        Target {
            workspace: "w".into(),
            pane: "p".into(),
        },
        Direction::Right,
    ));
    for json in [
        SPLIT.replace("herdr-process", "other"),
        SPLIT.replace("pane_id", "id"),
        POPUP.into(),
    ] {
        assert_eq!(
            finish(&mut fixture(&emit(&json)).open(&request).unwrap())
                .unwrap_err()
                .code,
            HostFailureCode::Malformed
        );
    }
}
#[test]
fn combined_output_cap_is_enforced_at_boundary() {
    let _speed = Speed::start("combined_output_cap_is_enforced_at_boundary");
    for (length, expected) in [(65535, false), (65536, false), (65537, true)] {
        let padded = format!("{POPUP}{}", " ".repeat(length - POPUP.len()));
        let failure_or_reply = finish(&mut fixture(&emit(&padded)).open(&view()).unwrap());
        if expected {
            assert_eq!(
                failure_or_reply.unwrap_err().code,
                HostFailureCode::Oversized
            );
        } else {
            assert_eq!(failure_or_reply, Ok(HostReply::Opened(None)));
        }
    }
    let script = format!(
        "{}\n{} >&2",
        emit(&"x".repeat(40000)),
        emit(&"x".repeat(40000))
    );
    assert_eq!(
        finish(&mut fixture(&script).open(&view()).unwrap())
            .unwrap_err()
            .code,
        HostFailureCode::Oversized
    );
}
#[test]
fn hanging_command_times_out_without_blocking_poll() {
    let _speed = Speed::start("hanging_command_times_out_without_blocking_poll");
    let mut call = fixture("exec /bin/sleep 10").open(&view()).unwrap();
    // THE NON-BLOCKING HALF: the double parks for ten seconds and this answers
    // anyway, still inside the product deadline it was spawned with.
    let tick = Instant::now();
    assert_eq!(call.poll(), Ok(None));
    assert!(tick.elapsed() < POLL_BOUND, "poll blocked");
    // AND THE REFUSAL HALF, reached by moving the bound rather than by
    // sleeping out the product's half second: the double is still running, its
    // deadline has passed, and the call is refused instead of waited on.
    call.bound(Duration::ZERO);
    assert_eq!(call.poll().unwrap_err().code, HostFailureCode::Timeout);
}
#[test]
fn drop_reaps_exact_owned_child() {
    let _speed = Speed::start("drop_reaps_exact_owned_child");
    let host = fixture("printf '%s' \"$$\" > \"${0%/*}/pid\"; exec /bin/sleep 10");
    let call = host.open(&view()).unwrap();
    let start = Instant::now();
    let pid_file = host.binary.parent().unwrap().join("pid");
    let pid = loop {
        if let Ok(text) = fs::read_to_string(&pid_file)
            && let Ok(pid) = text.parse::<i32>()
        {
            break pid;
        }
        assert!(start.elapsed() < Duration::from_secs(2));
        std::thread::yield_now();
    };
    drop(call);
    let mut status = 0;
    // SAFETY: this is the exact private fixture child, and status is writable.
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}
#[test]
fn spawn_failure_omits_private_path() {
    let _speed = Speed::start("spawn_failure_omits_private_path");
    let host = Herdr {
        binary: "/private/tmp/dotfiles-modernization/missing-secret-binary".into(),
        socket: "unused".into(),
    };
    let Err(error) = host.focus("pane") else {
        panic!("missing binary accepted")
    };
    assert!(!format!("{error:?}").contains("secret"));
}

struct Speed {
    name: &'static str,
    start: Instant,
}
impl Speed {
    fn start(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}
/// The REVIEW line. Every test here is milliseconds of work, so one past a
/// second has earned a look, and the printed reading above is how it gets one.
const SPEED_REPORT: Duration = Duration::from_secs(1);
/// The FAILURE line, which bounds a hang rather than reading a speed. Wall
/// time under a parallel runner is contention and not cost (the same finding
/// pns's own `TEST_CEILING_MS` records), and a one-second hard line failed
/// pull requests whose diff never touched this crate.
const SPEED_CEILING: Duration = Duration::from_secs(10);
impl Drop for Speed {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        eprintln!("{}: {:.3} ms", self.name, elapsed.as_secs_f64() * 1000.0);
        if elapsed > SPEED_REPORT {
            eprintln!("{}: over the {SPEED_REPORT:?} review line", self.name);
        }
        if !std::thread::panicking() {
            assert!(elapsed < SPEED_CEILING, "{} hung: {elapsed:?}", self.name);
        }
    }
}
