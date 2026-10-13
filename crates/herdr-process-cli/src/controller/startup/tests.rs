use super::*;
use herdr_process_adapters::{Listener, test_support::TempRoot};
mod log;
fn fixture() -> (TempRoot, Endpoint, Launch) {
    let temporary = TempRoot::new();
    let root = temporary.path().to_path_buf();
    let endpoint = Endpoint::new(&root).unwrap();
    let mut command = Command::new("/bin/sh");
    command
        .env_clear()
        .env("HOME", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    (
        temporary,
        endpoint,
        Launch {
            command,
            runtime: root,
        },
    )
}
fn listener_after_pid(root: PathBuf) -> std::thread::JoinHandle<Listener> {
    std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_millis(180);
        while !root.join("pid").exists() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        Endpoint::new(&root).unwrap().bind().unwrap().unwrap()
    })
}
#[test]
fn existing_endpoint_never_spawns_candidate() {
    let start = Instant::now();
    let (root, endpoint, mut launch) = fixture();
    let _listener = endpoint.bind().unwrap().unwrap();
    launch
        .command
        .arg("-c")
        .arg("echo wrong > \"$HOME/spawned\"");
    let result = connect(&endpoint, &mut launch, Duration::from_millis(100));
    assert!(result.is_ok());
    assert!(!root.join("spawned").exists());
    assert!(start.elapsed() < Duration::from_millis(300));
}
#[test]
fn stalled_startup_kills_and_reaps_only_owned_unstarted_child() {
    let start = Instant::now();
    let (root, endpoint, mut launch) = fixture();
    launch
        .command
        .arg("-c")
        .arg("echo $$ > \"$HOME/pid\"; exec /bin/sleep 0.5");
    let result = connect(&endpoint, &mut launch, Duration::from_millis(70));
    assert!(result.err().unwrap().to_string().contains("startup"));
    let pid: i32 = std::fs::read_to_string(root.join("pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let _guard = OwnedPid(pid);
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(start.elapsed() < Duration::from_millis(200));
}
#[test]
fn exited_duplicate_is_reaped_while_connecting_to_winner() {
    let start = Instant::now();
    let (root, endpoint, mut launch) = fixture();
    launch
        .command
        .arg("-c")
        .arg("echo $$ > \"$HOME/pid\"; echo herdr-process:duplicate >&2");
    let worker = listener_after_pid(root.path().to_path_buf());
    let result = connect(&endpoint, &mut launch, Duration::from_millis(200));
    let _winner = worker.join().unwrap();
    assert!(result.is_ok());
    let pid: i32 = std::fs::read_to_string(root.join("pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let _guard = OwnedPid(pid);
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(start.elapsed() < Duration::from_millis(300));
}

struct OwnedPid(i32);
impl Drop for OwnedPid {
    fn drop(&mut self) {
        let mut status = 0;
        if unsafe { libc::waitpid(self.0, &mut status, libc::WNOHANG) } == 0 {
            unsafe {
                libc::kill(self.0, libc::SIGKILL);
                libc::waitpid(self.0, &mut status, 0);
            }
        }
    }
}

#[test]
fn started_manager_has_private_streams_and_independent_process_group() {
    let start = Instant::now();
    let (root, endpoint, mut launch) = fixture();
    let script = r#"import os,socket,sys,time
assert os.getpgrp() == os.getpid()
assert os.read(0,1) == b''
s=socket.socket(socket.AF_UNIX)
s.bind(os.environ['HOME']+'/session.sock')
s.listen(1)
print('herdr-process:ready', file=sys.stderr, flush=True)
s.settimeout(3)
c,_=s.accept()
c.settimeout(3)
try: c.recv(1)
except TimeoutError: pass
"#;
    launch.command = Command::new("python3");
    launch
        .command
        .arg("-c")
        .arg(script)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null());
    let connection = connect(&endpoint, &mut launch, Duration::from_secs(2)).unwrap();
    assert!(connection.candidate.is_some());
    let pid = connection.candidate.as_ref().unwrap().child.id() as i32;
    let _guard = OwnedPid(pid);
    drop(connection);
    let mut status = 0;
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
    assert!(start.elapsed() < Duration::from_secs(3));
}
