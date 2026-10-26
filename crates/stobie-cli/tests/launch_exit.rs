//! `launch` must report an application that exits before it answers as soon
//! as it exits, with the tail of its log, rather than waiting out `--timeout`.

use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn an_application_that_exits_at_once_fails_the_launch_at_once_with_its_log() {
    use std::os::unix::fs::DirBuilderExt;
    let runtime = std::env::temp_dir().join(format!("stobie-exit-{}", std::process::id()));
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&runtime)
        .unwrap();

    let started = Instant::now();
    let launched = Command::new(env!("CARGO_BIN_EXE_stobie"))
        .args(["launch", "early", "--json", "--timeout", "60000", "--"])
        .args([
            "/bin/sh",
            "-c",
            "echo 'cannot open the display' >&2; exit 3",
        ])
        .env("XDG_RUNTIME_DIR", &runtime)
        .output()
        .expect("stobie runs");
    let elapsed = started.elapsed();

    let stderr = String::from_utf8_lossy(&launched.stderr);
    assert!(
        elapsed < Duration::from_secs(10),
        "launch waited {elapsed:?} for an application that had exited: {stderr}"
    );
    assert_eq!(launched.status.code(), Some(3), "{stderr}");
    let error: serde_json::Value = serde_json::from_str(stderr.trim()).expect("a JSON error");
    assert_eq!(error["error"]["code"], "launch_failed", "{error}");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("exited before answering") && message.contains("cannot open the display"),
        "the error names the exit and quotes the log: {message}"
    );
    // The supervisor removed the session it recorded.
    assert!(!runtime.join("stobie/early.json").exists());
    assert!(!runtime.join("stobie/early.d").exists());

    let _ = std::fs::remove_dir_all(&runtime);
}
