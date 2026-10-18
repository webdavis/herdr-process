use super::*;
use crate::test_support::TempRoot;
use std::{
    fs,
    time::{Duration, Instant},
};

const HANG_BOUND: Duration = Duration::from_secs(10);
const POLL_BOUND: Duration = Duration::from_secs(1);

fn shell(script: &str, argument: &str, deadline: Duration) -> HostCall {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", script, "sh", argument]);
    HostCall::spawn(command, Expected::Popup, deadline).unwrap()
}

fn printing(output: &str) -> HostCall {
    shell("printf '%s' \"$1\"", output, HANG_BOUND)
}

fn finish(call: &mut HostCall) -> Result<HostReply, HostFailure> {
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < HANG_BOUND, "host call stalled");
        let tick = Instant::now();
        let result = call.poll();
        assert!(tick.elapsed() < POLL_BOUND, "poll blocked");
        if let Some(reply) = result? {
            return Ok(reply);
        }
        std::thread::yield_now();
    }
}

#[test]
fn a_finished_call_keeps_its_reply() {
    let mut call = printing(POPUP);
    assert_eq!(finish(&mut call), Ok(HostReply::Opened(None)));
    assert_eq!(call.poll(), Ok(Some(HostReply::Opened(None))));
}

#[test]
fn nonzero_exit_and_stderr_fail_without_secret_text() {
    for script in ["printf 'secret' >&2; exit 3", "printf 'secret' >&2"] {
        let failure = finish(&mut shell(script, "", HANG_BOUND)).unwrap_err();
        assert_eq!(failure.code, HostFailureCode::Exit);
        assert!(!format!("{failure:?}").contains("secret"));
    }
}

#[test]
fn a_structured_api_error_wins_over_stderr() {
    let script = "printf '%s' \"$1\"; printf 'noise' >&2";
    let rejected = r#"{"id":"request","error":{"code":"denied","message":"no"}}"#;
    let failure = finish(&mut shell(script, rejected, HANG_BOUND)).unwrap_err();
    assert_eq!(failure.code, HostFailureCode::Rejected);
}

#[test]
fn combined_output_cap_is_enforced_at_boundary() {
    for (length, oversized) in [(65535, false), (65536, false), (65537, true)] {
        let padded = format!("{POPUP}{}", " ".repeat(length - POPUP.len()));
        let failure_or_reply = finish(&mut printing(&padded));
        if oversized {
            assert_eq!(
                failure_or_reply.unwrap_err().code,
                HostFailureCode::Oversized
            );
        } else {
            assert_eq!(failure_or_reply, Ok(HostReply::Opened(None)));
        }
    }
    let both = "printf '%s' \"$1\"; printf '%s' \"$1\" >&2";
    assert_eq!(
        finish(&mut shell(both, &"x".repeat(40000), HANG_BOUND))
            .unwrap_err()
            .code,
        HostFailureCode::Oversized
    );
}

#[test]
fn hanging_command_times_out_without_blocking_poll() {
    let mut waiting = shell("exec /bin/sleep 10", "", HANG_BOUND);
    let tick = Instant::now();
    assert_eq!(waiting.poll(), Ok(None));
    assert!(tick.elapsed() < POLL_BOUND, "poll blocked");
    let mut expired = shell("exec /bin/sleep 10", "", Duration::ZERO);
    assert_eq!(expired.poll().unwrap_err().code, HostFailureCode::Timeout);
}

#[test]
fn drop_reaps_exact_owned_child() {
    let root = TempRoot::new();
    let pid_file = root.join("pid");
    let call = shell(
        "printf '%s' \"$$\" > \"$1\"; exec /bin/sleep 10",
        pid_file.to_str().unwrap(),
        HANG_BOUND,
    );
    let start = Instant::now();
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
    let host = Herdr {
        binary: "/private/tmp/dotfiles-modernization/missing-secret-binary".into(),
        socket: "unused".into(),
    };
    let Err(error) = host.focus("pane") else {
        panic!("missing binary accepted")
    };
    assert!(!format!("{error:?}").contains("secret"));
}
