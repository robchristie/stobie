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
    supervisor.spawn().map_err(|error| {
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
        format!("the application exited before answering. Last log lines:\n{tail}"),
        "run the program directly to see why it exits",
    ))
}

/// The supervisor: run the application with its automation socket, record
/// the session, and clean up when the application exits.
pub fn supervise(name: &str, directory: &Path, program: &Path, arguments: &[String]) -> i32 {
    let socket = directory.join("app.sock");
    // Held for this process's life, and inherited by the application.
    let Some(_lock) = hold_lock(&directory.join(LOCK_FILE)) else {
        return 1;
    };
    let log = log_path(name)
        .ok()
        .and_then(|path| std::fs::File::create(path).ok());
    let mut command = std::process::Command::new(program);
    command
        .args(arguments)
        .env("STOBIE_AUTOMATION_SOCKET", &socket)
        .stdin(Stdio::null());
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
    status.ok().and_then(|status| status.code()).unwrap_or(0)
}

/// Lock `path` exclusively and keep the lock across `exec`, so a child
/// holds it too. `None` if it cannot be taken.
fn hold_lock(path: &Path) -> Option<std::fs::File> {
    use std::os::fd::AsRawFd;
    let file = std::fs::File::create(path).ok()?;
    let fd = file.as_raw_fd();
    unsafe {
        if libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) != 0 {
            return None;
        }
        let flags = libc::fcntl(fd, libc::F_GETFD);
        if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
            return None;
        }
    }
    Some(file)
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
        // A child inherits the lock and keeps it after the holder lets go.
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        drop(lock);
        assert!(lock_held(&path), "the application keeps the lock");
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(
            !lock_held(&path),
            "nobody holds the lock once both have gone"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
