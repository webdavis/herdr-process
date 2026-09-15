use std::{
    process::Command,
    time::{Duration, Instant},
};
fn check(name: &str) {
    let start = Instant::now();
    let output = Command::new("python3")
        .args(["-I", "-S"])
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/cli_composition.py"
        ))
        .arg(env!("CARGO_BIN_EXE_herdr-process"))
        .arg(format!("Composition.{name}"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "{name}: {:?}",
        start.elapsed()
    );
}

#[test]
fn help_does_not_open_config() {
    check("test_help_does_not_open_config");
}
#[test]
fn invalid_arguments_and_invalid_utf8_exit_two() {
    check("test_invalid_arguments_and_invalid_utf8_exit_two");
}
#[test]
fn generate_uses_environment_selected_config() {
    check("test_generate_uses_environment_selected_config");
}
#[test]
fn runtime_errors_are_one_and_home_is_required_absolute() {
    check("test_runtime_errors_are_one_and_home_is_required_absolute");
}
#[test]
fn action_rejects_invalid_profile_before_start() {
    check("test_action_rejects_invalid_profile_before_start");
}
#[test]
fn manager_lock_duplicate_and_private_lifetime() {
    check("test_manager_lock_duplicate_and_private_lifetime");
}
#[test]
fn action_requires_host_context_and_split_target() {
    check("test_action_requires_host_context_and_split_target");
}
#[test]
fn invalid_attachment_ticket_is_never_printed() {
    check("test_invalid_attachment_ticket_is_never_printed");
}
#[test]
fn supervise_dispatch_does_not_load_configuration() {
    check("test_supervise_dispatch_does_not_load_configuration");
}
#[test]
fn attachment_replay_ack_raw_input_resize_and_retire() {
    check("test_attachment_replay_ack_raw_input_resize_and_retire");
}
#[test]
fn attachment_backpressure_keeps_every_partial_screen_byte() {
    check("test_attachment_backpressure_keeps_every_partial_screen_byte");
}
#[test]
fn retire_while_terminal_blocked_restores_without_hanging() {
    check("test_retire_while_terminal_blocked_restores_without_hanging");
}
#[test]
fn attachment_disconnect_restores() {
    check("test_attachment_disconnect_restores");
}
#[test]
fn attachment_handshake_error_preserves_message() {
    check("test_attachment_handshake_error_preserves_message");
}

#[test]
fn attachment_unwind_restores_terminal() {
    check("test_attachment_unwind_restores_terminal");
}

#[test]
fn duplicate_manager_does_not_reload_configuration() {
    check("test_duplicate_manager_does_not_reload_configuration");
}
