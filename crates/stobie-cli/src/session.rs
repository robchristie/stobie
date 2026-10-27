//! Sessions: named native applications that `launch` started (and a
//! supervisor watches) or that `attach` recorded.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use stobie_client::ClientError;

/// Unix socket paths are limited to about 100 bytes.
const SOCKET_PATH_LIMIT: usize = 100;

/// The lock file in a launched session's directory. The supervisor holds an
/// exclusive lock on it, which the application inherits, so the lock is held
/// exactly while one of them still runs. A recorded PID is trusted only then:
/// once both have gone, the PID may belong to an unrelated process.
const LOCK_FILE: &str = "supervisor.lock";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Target {
    /// A native application's automation socket.
    Native {
        socket: PathBuf,
        /// The application process, when `launch` started it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pid: Option<u32>,
        /// The private directory holding the socket and log.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        directory: Option<PathBuf>,
    },
    /// A browser page opened with `?automation=1`.
    Browser {
        endpoint: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub name: String,
    pub target: Target,
}

/// The private directory holding sessions: `$XDG_RUNTIME_DIR/stobie`, or
/// `/tmp/stobie-UID`, created with mode 0700.
pub fn root() -> Result<PathBuf, ClientError> {
    let root = match std::env::var_os("XDG_RUNTIME_DIR") {
        Some(runtime) if !runtime.is_empty() => PathBuf::from(runtime).join("stobie"),
        _ => std::env::temp_dir().join(format!("stobie-{}", unsafe { libc::getuid() })),
    };
    create_private(&root)?;
    Ok(root)
}

fn create_private(path: &Path) -> Result<(), ClientError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(path)
        .map_err(|error| {
            ClientError::new(
                "session_failed",
                format!("cannot create {}: {error}", path.display()),
                "check the runtime directory's permissions",
            )
        })?;
    // Never follow a symbolic link planted in a shared temporary directory.
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        ClientError::new(
            "session_failed",
            error.to_string(),
            "check the runtime directory",
        )
    })?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::getuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(ClientError::new(
            "session_failed",
            format!("{} is not private to this user", path.display()),
            "remove it, or set XDG_RUNTIME_DIR to a private directory",
        ));
    }
    Ok(())
}

fn file(name: &str) -> Result<PathBuf, ClientError> {
    validate_name(name)?;
    Ok(root()?.join(format!("{name}.json")))
}

/// The application's output for session `name`, kept after it exits.
pub fn log_path(name: &str) -> Result<PathBuf, ClientError> {
    validate_name(name)?;
    Ok(root()?.join(format!("{name}.log")))
}

pub fn validate_name(name: &str) -> Result<(), ClientError> {
    if name.is_empty()
        || name.len() > 32
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ClientError::usage(format!(
            "session name {name:?} must be 1 to 32 letters, digits, '-' or '_'"
        )));
    }
    Ok(())
}

pub fn save(session: &Session) -> Result<(), ClientError> {
    let path = file(&session.name)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(session).expect("sessions serialise"),
    )
    .and_then(|()| std::fs::rename(&temporary, &path))
    .map_err(|error| {
        ClientError::new(
            "session_failed",
            format!("cannot record session {}: {error}", session.name),
            "check the runtime directory",
        )
    })
}

pub fn load(name: &str) -> Result<Session, ClientError> {
    let path = file(name)?;
    let bytes = std::fs::read(&path).map_err(|_| {
        ClientError::connection(
            format!("no session named {name}"),
            "run `stobie sessions` to list sessions, or `stobie launch` to start one",
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        ClientError::new(
            "session_failed",
            format!("session {name} is unreadable: {error}"),
            format!("remove {}", path.display()),
        )
    })
}

pub fn remove(name: &str) {
    if let Ok(path) = file(name) {
        let _ = std::fs::remove_file(path);
    }
}

pub fn list() -> Result<Vec<Session>, ClientError> {
    let mut sessions = Vec::new();
    let entries = std::fs::read_dir(root()?).map_err(|error| {
        ClientError::new(
            "session_failed",
            error.to_string(),
            "check the runtime directory",
        )
    })?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
            && let Some(name) = path.file_stem().and_then(|stem| stem.to_str())
            && let Ok(session) = load(name)
        {
            sessions.push(session);
        }
    }
    sessions.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(sessions)
}

/// The session a command addresses: `--session`, else `$STOBIE_SESSION`,
/// else the only recorded session.
pub fn resolve(name: Option<&str>) -> Result<Session, ClientError> {
    if let Some(name) = name {
        return load(name);
    }
    if let Some(name) = std::env::var_os("STOBIE_SESSION").and_then(|name| name.into_string().ok())
    {
        return load(&name);
    }
    let sessions = list()?;
    match sessions.as_slice() {
        [session] => Ok(session.clone()),
        [] => Err(ClientError::connection(
            "no application is attached",
            "run `stobie launch NAME ...`, or pass --socket PATH or --session NAME",
        )),
        _ => Err(ClientError::usage(format!(
            "several sessions exist ({}); choose one with --session NAME",
            sessions
                .iter()
                .map(|session| session.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Whether process `pid` is alive.
pub fn alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Whether some process holds the lock on `path`: the probe takes it only
/// when nobody does, and releases it at once.
pub fn lock_held(path: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let free = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
    !free
}

/// Whether `session`'s launched application still runs under its supervisor
/// or on its own. Only then may its recorded PID be signalled.
pub fn running(session: &Session) -> Option<bool> {
    match &session.target {
        Target::Native {
            pid: Some(_),
            directory: Some(directory),
            ..
        } => Some(lock_held(&directory.join(LOCK_FILE))),
        Target::Native { .. } | Target::Browser { .. } => None,
    }
}

/// Build the application if needed and return its executable: binary `bin`
/// or example `example` when one is given.
pub fn build(
    manifest: Option<&str>,
    package: Option<&str>,
    bin: Option<&str>,
    example: Option<&str>,
    release: bool,
) -> Result<PathBuf, ClientError> {
    let mut cargo = std::process::Command::new(std::env::var_os("CARGO").unwrap_or("cargo".into()));
    cargo.args(["build", "--message-format=json-render-diagnostics"]);
    if release {
        cargo.arg("--release");
    }
    if let Some(manifest) = manifest {
        cargo.args(["--manifest-path", manifest]);
    }
    if let Some(package) = package {
        cargo.args(["--package", package]);
    }
    if let Some(bin) = bin {
        cargo.args(["--bin", bin]);
    }
    if let Some(example) = example {
        cargo.args(["--example", example]);
    }
    let (kind, wanted) = match example {
        Some(example) => ("example", Some(example)),
        None => ("bin", bin),
    };
    let output = cargo.stderr(Stdio::inherit()).output().map_err(|error| {
        ClientError::new(
            "build_failed",
            format!("cannot run cargo: {error}"),
            "install Rust and cargo",
        )
    })?;
    if !output.status.success() {
        return Err(ClientError::new(
            "build_failed",
            "cargo build failed; its diagnostics are above",
            "fix the build, or launch a built binary with `-- PROGRAM`",
        ));
    }
    let executables: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| message["reason"] == "compiler-artifact")
        .filter(|message| {
            message["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|target| target == kind))
        })
        .filter(|message| wanted.is_none_or(|name| message["target"]["name"] == name))
        .filter_map(|message| message["executable"].as_str().map(PathBuf::from))
        .collect();
    match executables.as_slice() {
        [executable] => Ok(executable.clone()),
        [] => Err(ClientError::new(
            "build_failed",
            "the build produced no executable",
            "choose a package with a binary, or pass --bin NAME",
        )),
        _ => Err(ClientError::usage(format!(
            "the build produced several executables ({}); choose one with --bin NAME",
            executables
                .iter()
                .filter_map(|path| path.file_name()?.to_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Start `program` under a detached supervisor with a fresh private socket
/// directory, record session `name`, and wait until the application answers.
pub fn launch(
    name: &str,
    program: &Path,
    arguments: &[String],
    answered: impl Fn(&Path) -> bool,
    timeout: Duration,
) -> Result<Session, ClientError> {
    validate_name(name)?;
    if let Ok(existing) = load(name) {
        if running(&existing) == Some(true) {
            return Err(ClientError::usage(format!(
                "session {name} is running; stop it with `stobie stop {name}` or choose another name"
            )));
        }
        remove(name);
    }
    let directory = root()?.join(format!("{name}.d"));
    let _ = std::fs::remove_dir_all(&directory);
    create_private(&directory)?;
    let socket = directory.join("app.sock");
    if socket.as_os_str().len() > SOCKET_PATH_LIMIT {
        return Err(ClientError::new(
            "session_failed",
            format!(
                "the socket path {} is too long for a Unix socket",
                socket.display()
            ),
            "set XDG_RUNTIME_DIR to a shorter private directory",
        ));
    }
    let log = log_path(name)?;
    let executable = std::env::current_exe().map_err(|error| {
        ClientError::new(
            "session_failed",
            error.to_string(),
            "run stobie from a readable path",
        )
    })?;
    // The supervisor runs the application and removes the directory and
    // session when it exits, in its own process group so the caller's shell
    // can exit first.
    let mut supervisor = std::process::Command::new(executable);
    supervisor
        .arg("__supervise")
        .arg(name)
        .arg(&directory)
        .arg(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    {
        use std::os::unix::process::CommandExt;
        supervisor.process_group(0);
    }
    let mut supervisor = supervisor.spawn().map_err(|error| {
        ClientError::new(
            "launch_failed",
            format!("cannot start the supervisor: {error}"),
            "check that the stobie binary is executable",
        )
    })?;
    let started = Instant::now();
    loop {
        if let Ok(session) = load(name)
            && let Target::Native { pid: Some(pid), .. } = session.target
        {
            if answered(&socket) {
                return Ok(session);
            }
            if !alive(pid) {
                break;
            }
        }
        // The supervisor exits only once the application has, after removing
        // the session it recorded, so an application that exits at once can
        // leave no session to load; its exit is the signal.
        if matches!(supervisor.try_wait(), Ok(Some(_))) {
            break;
        }
        if started.elapsed() > timeout {
            let _ = stop(name, Duration::from_secs(5));
            return Err(ClientError::timeout(format!(
                "the application did not answer within {} ms; its log is {}",
                timeout.as_millis(),
                log.display()
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // The supervisor exits with the application's status once it has
    // cleaned up, which follows the application's exit closely.
    let waited = Instant::now();
    let status = loop {
        match supervisor.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if waited.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => break None,
        }
    };
    let exited = match status {
        Some(code) => format!("the application exited with status {code} before answering"),
        None => "the application exited before answering".to_owned(),
    };
    let tail = std::fs::read_to_string(&log).unwrap_or_default();
    let tail: String = tail
        .lines()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    Err(ClientError::new(
        "launch_failed",
        format!("{exited}. Last log lines:\n{tail}"),
        "run the program directly to see why it exits",
    ))
}

/// The supervisor: run the application with its automation socket, record
/// the session, and clean up when the application exits. It exits with the
/// application's status, or 128 plus the signal that ended it, as a shell
/// reports one.
pub fn supervise(name: &str, directory: &Path, program: &Path, arguments: &[String]) -> i32 {
    use std::io::Write;
    use std::os::unix::process::ExitStatusExt;
    let socket = directory.join("app.sock");
    // This run's log starts before anything can fail, so `launch` never
    // quotes an earlier run's.
    let mut log = log_path(name)
        .ok()
        .and_then(|path| std::fs::File::create(path).ok());
    // Held for this process's life, and inherited by the application.
    let Some(lock) = hold_lock(&directory.join(LOCK_FILE)) else {
        if let Some(log) = &mut log {
            let _ = writeln!(log, "the supervisor could not take the session lock");
        }
        return 1;
    };
    let mut command = std::process::Command::new(program);
    command
        .args(arguments)
        .env("STOBIE_AUTOMATION_SOCKET", &socket)
        .stdin(Stdio::null());
    inherit_lock(&mut command, &lock);
    if let Some(log) = log {
        let copy = log.try_clone().ok();
        command.stdout(log);
        if let Some(copy) = copy {
            command.stderr(copy);
        }
    }
    let Ok(mut child) = command.spawn() else {
        if let Ok(path) = log_path(name) {
            let _ = std::fs::write(path, "the program could not be started\n");
        }
        return 1;
    };
    let session = Session {
        name: name.into(),
        target: Target::Native {
            socket,
            pid: Some(child.id()),
            directory: Some(directory.to_path_buf()),
        },
    };
    if save(&session).is_err() {
        let _ = child.kill();
    }
    let status = child.wait();
    // Keep the session file only if a newer launch replaced it.
    if load(name).is_ok_and(|current| current == session) {
        remove(name);
    }
    let _ = std::fs::remove_dir_all(directory);
    status
        .ok()
        .and_then(|status| {
            status
                .code()
                .or_else(|| status.signal().map(|signal| 128 + signal))
        })
        .unwrap_or(1)
}

/// The program `launch -- PROGRAM` runs: `program` itself when it names a
/// path, or else the first executable file of that name in `path`, the
/// value of `PATH`, as a shell finds one.
pub fn find_program(
    program: &Path,
    path: Option<&std::ffi::OsStr>,
) -> Result<PathBuf, ClientError> {
    use std::os::unix::fs::PermissionsExt;
    let bare = program.components().count() == 1 && program.parent() == Some(Path::new(""));
    let found = if bare {
        path.into_iter()
            .flat_map(std::env::split_paths)
            .map(|directory| directory.join(program))
            .find(|candidate| {
                std::fs::metadata(candidate).is_ok_and(|metadata| {
                    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                })
            })
            .ok_or_else(|| {
                ClientError::usage(format!("cannot find {} on PATH", program.display()))
            })?
    } else {
        program.to_path_buf()
    };
    std::fs::metadata(&found)
        .map_err(|error| ClientError::usage(format!("cannot find {}: {error}", found.display())))?;
    // Absolute, as the supervisor may run elsewhere, but not resolved: a
    // multi-call program, such as rustup's `cargo`, acts on the name it was
    // run by.
    std::path::absolute(&found)
        .map_err(|error| ClientError::usage(format!("cannot find {}: {error}", found.display())))
}

/// Lock `path` exclusively. The lock closes on `exec`, so only a child
/// given it with [`inherit_lock`] holds it too. `None` if it cannot be
/// taken.
fn hold_lock(path: &Path) -> Option<std::fs::File> {
    use std::os::fd::AsRawFd;
    let file = std::fs::File::create(path).ok()?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return None;
    }
    Some(file)
}

/// Keep `lock` open across `exec` in `command`'s process alone, so it holds
/// the lock after its parent lets go. Any other child, spawned meanwhile by
/// another thread, does not.
fn inherit_lock(command: &mut std::process::Command, lock: &std::fs::File) {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    let fd = lock.as_raw_fd();
    // SAFETY: the hook only calls `fcntl`, which is async-signal-safe, on a
    // descriptor the child's copy of the table holds.
    unsafe {
        command.pre_exec(move || {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// Stop session `name`'s application and wait for its supervisor to clean up.
/// A session whose application no longer runs is removed without signalling
/// anything: its recorded PID may now be another process's.
pub fn stop(name: &str, timeout: Duration) -> Result<Session, ClientError> {
    let session = load(name)?;
    let Target::Native { pid, directory, .. } = &session.target else {
        // A browser session only records where the page is.
        remove(name);
        return Ok(session);
    };
    if let (Some(pid), Some(true)) = (*pid, running(&session)) {
        let still_running = || running(&session) == Some(true);
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
        let started = Instant::now();
        while still_running() && started.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(50));
        }
        if still_running() && alive(pid) {
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGKILL);
            }
        }
    }
    // The supervisor removes both once the application exits.
    let started = Instant::now();
    while file(name).is_ok_and(|path| path.exists()) && started.elapsed() < Duration::from_secs(2) {
        std::thread::sleep(Duration::from_millis(50));
    }
    remove(name);
    if let Some(directory) = directory {
        let _ = std::fs::remove_dir_all(directory);
    }
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_is_held_while_its_holder_or_an_inheriting_child_runs() {
        let directory = std::env::temp_dir().join(format!("stobie-lock-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join(LOCK_FILE);
        assert!(!lock_held(&path), "a missing lock file is not held");
        let lock = hold_lock(&path).expect("the lock is free");
        assert!(lock_held(&path));
        assert!(
            hold_lock(&path).is_none(),
            "a held lock cannot be taken again"
        );
        // A child given the lock keeps it after the holder lets go, and
        // another child, such as one a concurrent test spawned, does not.
        let mut application = std::process::Command::new("sleep");
        application.arg("30");
        inherit_lock(&mut application, &lock);
        let mut application = application.spawn().unwrap();
        let mut other = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        drop(lock);
        assert!(lock_held(&path), "the application keeps the lock");
        application.kill().unwrap();
        application.wait().unwrap();
        assert!(
            !lock_held(&path),
            "a child not given the lock does not hold it"
        );
        other.kill().unwrap();
        other.wait().unwrap();
        assert!(
            !lock_held(&path),
            "nobody holds the lock once both have gone"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_bare_program_name_is_found_on_path_as_a_shell_finds_it() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("stobie-path-{}", std::process::id()));
        let (first, second) = (root.join("first"), root.join("second"));
        for directory in [&first, &second] {
            std::fs::create_dir_all(directory).unwrap();
        }
        let file = |path: &Path, mode: u32| {
            std::fs::write(path, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        };
        // Only an executable file counts, and the first directory wins.
        file(&first.join("tool"), 0o644);
        file(&second.join("tool"), 0o755);
        file(&second.join("plain"), 0o644);
        let path = std::env::join_paths([&first, &second]).unwrap();
        assert_eq!(
            find_program(Path::new("tool"), Some(&path)).unwrap(),
            second.join("tool")
        );
        file(&first.join("tool"), 0o755);
        assert_eq!(
            find_program(Path::new("tool"), Some(&path)).unwrap(),
            first.join("tool")
        );
        // A link is run by its own name, as a multi-call program needs.
        std::os::unix::fs::symlink(second.join("tool"), first.join("alias")).unwrap();
        assert_eq!(
            find_program(Path::new("alias"), Some(&path)).unwrap(),
            first.join("alias")
        );
        for missing in ["plain", "absent"] {
            assert_eq!(
                find_program(Path::new(missing), Some(&path))
                    .unwrap_err()
                    .message,
                format!("cannot find {missing} on PATH")
            );
        }
        assert!(find_program(Path::new("tool"), None).is_err());
        // A path is taken as given, not searched for.
        let relative = Path::new("./tool");
        assert!(
            find_program(relative, Some(&path))
                .unwrap_err()
                .message
                .starts_with("cannot find ./tool: ")
        );
        assert_eq!(
            find_program(&second.join("plain"), Some(&path)).unwrap(),
            second.join("plain")
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
