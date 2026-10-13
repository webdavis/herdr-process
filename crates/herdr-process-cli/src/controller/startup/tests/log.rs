use super::*;
fn startup_logs(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.starts_with("startup-"))
        .collect();
    names.sort();
    names
}
#[test]
fn second_action_against_running_manager_leaves_one_startup_log_not_two() {
    let (root, endpoint, mut launch) = fixture();
    let _listener = endpoint.bind().unwrap().unwrap();
    std::fs::write(root.join("startup-1-1.log"), "herdr-process:ready\n").unwrap();
    connect(&endpoint, &mut launch, Duration::from_millis(100)).unwrap();
    assert_eq!(startup_logs(root.path()), ["startup-1-1.log"]);
}
#[test]
fn startup_log_is_removed_once_the_manager_reports_ready_or_duplicate() {
    for script in [
        "echo $$ > \"$HOME/pid\"; echo herdr-process:duplicate >&2",
        "echo $$ > \"$HOME/pid\"; echo herdr-process:ready >&2; exec /bin/sleep 1",
    ] {
        let (root, endpoint, mut launch) = fixture();
        launch.command.arg("-c").arg(script);
        let winner = listener_after_pid(root.path().to_path_buf());
        let connection = connect(&endpoint, &mut launch, Duration::from_millis(200)).unwrap();
        let _winner = winner.join().unwrap();
        assert_eq!(startup_logs(root.path()), Vec::<String>::new(), "{script}");
        drop(connection);
    }
}
