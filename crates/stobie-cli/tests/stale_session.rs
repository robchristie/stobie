//! A session whose supervisor and application have gone must never have its
//! recorded PID signalled: by then the PID may be an unrelated process's.

use std::process::{Command, Stdio};

fn stobie(runtime: &std::path::Path, arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_stobie"))
        .args(arguments)
        .env("XDG_RUNTIME_DIR", runtime)
        .output()
        .expect("stobie runs")
}

#[test]
fn a_stale_session_is_removed_without_signalling_its_recorded_pid() {
    use std::os::unix::fs::DirBuilderExt;
    let runtime = std::env::temp_dir().join(format!("stobie-stale-{}", std::process::id()));
    let sessions = runtime.join("stobie");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(sessions.join("stale.d"))
        .unwrap();
    // An unrelated live process now has the PID the session recorded, and a
    // leftover lock file nobody holds.
    let mut unrelated = Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    std::fs::write(sessions.join("stale.d/supervisor.lock"), "").unwrap();
    let session = serde_json::json!({
        "name": "stale",
        "target": {
            "kind": "native",
            "socket": sessions.join("stale.d/app.sock"),
            "pid": unrelated.id(),
            "directory": sessions.join("stale.d"),
        },
    });
    std::fs::write(sessions.join("stale.json"), session.to_string()).unwrap();

    let listed = stobie(&runtime, &["sessions", "--json"]);
    let listed: serde_json::Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(listed["sessions"][0]["running"], false, "{listed}");

    let stopped = stobie(&runtime, &["stop", "stale"]);
    assert!(stopped.status.success(), "{stopped:?}");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(
        unrelated.try_wait().unwrap().is_none(),
        "stop signalled an unrelated process"
    );
    assert!(!sessions.join("stale.json").exists());
    assert!(!sessions.join("stale.d").exists());

    unrelated.kill().unwrap();
    let _ = unrelated.wait();
    let _ = std::fs::remove_dir_all(&runtime);
}
