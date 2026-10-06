//! Opt-in private native transport. One listener thread services one connection
//! at a time, with one bounded JSON line per connection and absolute deadlines.

use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{INSPECTION_REQUEST_BYTES, Inspection, InspectionError, InspectionErrorCode};

const CONNECTION_DEADLINE: Duration = Duration::from_secs(2);

/// Owns a private 0600 Unix socket and its bounded listener thread. Dropping the
/// host stops admission, joins the worker and removes only its own socket inode.
/// No application model crosses the thread boundary. Read requests never wake
/// egui; admission of an invocation calls the supplied repaint waker once.
pub struct NativeInspectionHost {
    inspection: Inspection,
    path: PathBuf,
    inode: (u64, u64),
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl NativeInspectionHost {
    pub(super) fn start(
        inspection: Inspection,
        path: PathBuf,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "automation socket path must be absolute",
            ));
        }
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "automation socket path already exists; refusing replacement",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let mut state = inspection.state.lock().expect("inspection mutex poisoned");
        if state.waker.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "inspection already has a native host",
            ));
        }
        let listener = UnixListener::bind(&path)?;
        let metadata = match (|| {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)?;
            std::fs::symlink_metadata(&path)
        })() {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
        };
        let inode = (metadata.dev(), metadata.ino());
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_inspection = inspection.clone();
        let worker = match thread::Builder::new()
            .name("polyorama-inspection".into())
            .spawn(move || {
                while !thread_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            let outcome = read_request(&mut stream).and_then(|bytes| {
                                String::from_utf8(bytes).map_err(|error| {
                                    io::Error::new(io::ErrorKind::InvalidData, error)
                                })
                            });
                            if thread_stop.load(Ordering::Acquire) {
                                break;
                            }
                            let reply = match outcome {
                                Ok(request) => thread_inspection.handle_json(&request),
                                Err(error) => {
                                    serde_json::to_string(&thread_inspection.error_reply(
                                        String::new(),
                                        InspectionError::new(
                                            InspectionErrorCode::InvalidRequest,
                                            format!("invalid native request: {error}"),
                                        ),
                                    ))
                                    .expect("reply serialises")
                                }
                            };
                            let _ = write_reply(&mut stream, reply.as_bytes());
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20))
                        }
                        Err(_) => break,
                    }
                }
            }) {
            Ok(worker) => worker,
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
        };
        state.waker = Some(wake);
        drop(state);
        Ok(Self {
            inspection,
            path,
            inode,
            stop,
            worker: Some(worker),
        })
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for NativeInspectionHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.inspection
            .state
            .lock()
            .expect("inspection mutex poisoned")
            .waker = None;
        if std::fs::symlink_metadata(&self.path)
            .is_ok_and(|metadata| (metadata.dev(), metadata.ino()) == self.inode)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn read_request(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let deadline = Instant::now() + CONNECTION_DEADLINE;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "request deadline exceeded"))?;
        stream.set_read_timeout(Some(remaining))?;
        let count = stream.read(&mut buffer)?;
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "request deadline exceeded",
            ));
        }
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "request requires one newline-terminated JSON line",
            ));
        }
        let chunk = &buffer[..count];
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let line = newline.map_or(chunk, |position| &chunk[..position]);
        if bytes.len() + line.len() > INSPECTION_REQUEST_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "request exceeds byte limit",
            ));
        }
        bytes.extend_from_slice(line);
        if newline.is_some() {
            return Ok(bytes);
        }
    }
}

fn write_reply(stream: &mut UnixStream, bytes: &[u8]) -> io::Result<()> {
    let deadline = Instant::now() + CONNECTION_DEADLINE;
    let mut written = 0;
    while written < bytes.len() {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "reply deadline exceeded"))?;
        stream.set_write_timeout(Some(remaining))?;
        let count = stream.write(&bytes[written..])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "native reply stream closed",
            ));
        }
        written += count;
    }
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "reply deadline exceeded"))?;
    stream.set_write_timeout(Some(remaining))?;
    stream.write_all(b"\n")
}
