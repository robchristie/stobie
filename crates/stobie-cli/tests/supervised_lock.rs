//! A launched application holds its session's lock on its own, so a session
//! still counts as running after its supervisor has gone, and `stop` still
//! stops it.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn stobie(runtime: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stobie"))
        .args(arguments)
        .env("XDG_RUNTIME_DIR", runtime)
        .output()
        .expect("stobie runs")
}

fn running(runtime: &std::path::Path) -> serde_json::Value {
    let listed = stobie(runtime, &["sessions", "--json"]);
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    listed["sessions"][0]["running"].clone()
}

#[test]
fn the_application_keeps_its_session_running_after_its_supervisor_goes() {
    use std::os::unix::fs::DirBuilderExt;
    let runtime = std::env::temp_dir().join(format!("stobie-held-{}", std::process::id()));
    let sessions = runtime.join("stobie");
    let directory = sessions.join("held.d");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&directory)
        .unwrap();
    // The supervisor `launch` starts, running an application that never
    // answers.
    let mut supervisor = Command::new(env!("CARGO_BIN_EXE_stobie"))
        .arg("__supervise")
        .arg("held")
        .arg(&directory)
        .args(["/bin/sh", "-c", "sleep 30"])
        .env("XDG_RUNTIME_DIR", &runtime)
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while !sessions.join("held.json").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the supervisor recorded no session"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(running(&runtime), true);

    supervisor.kill().unwrap();
    supervisor.wait().unwrap();
    assert_eq!(
        running(&runtime),
        true,
        "the application inherited the lock and still holds it"
    );

    let stopped = stobie(&runtime, &["stop", "held"]);
    assert!(stopped.status.success(), "{stopped:?}");
    assert!(!sessions.join("held.json").exists());
    assert!(!directory.exists());

    let _ = std::fs::remove_dir_all(&runtime);
}
