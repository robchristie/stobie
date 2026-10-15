//! Transports carry one JSON request and its JSON reply.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use stobie_protocol::{INSPECTION_PUBLICATION_BYTES, INSPECTION_REQUEST_BYTES};

use crate::ClientError;

/// The largest reply a client reads: a publication plus framing.
pub const REPLY_BYTES: usize = INSPECTION_PUBLICATION_BYTES + 64 * 1024;

/// Carries one request to the application and returns its reply, before
/// `deadline`. A transport sends each request at most once.
pub trait Transport {
    fn request(&mut self, body: &str, deadline: Instant) -> Result<String, ClientError>;
    /// A short description for diagnostics, such as the socket path.
    fn describe(&self) -> String;
}

impl Transport for Box<dyn Transport> {
    fn request(&mut self, body: &str, deadline: Instant) -> Result<String, ClientError> {
        (**self).request(body, deadline)
    }

    fn describe(&self) -> String {
        (**self).describe()
    }
}

/// A native application's private Unix socket: one connection per request,
/// one newline-terminated JSON line each way.
pub struct SocketTransport {
    path: PathBuf,
}

impl SocketTransport {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(unix)]
impl Transport for SocketTransport {
    fn request(&mut self, body: &str, deadline: Instant) -> Result<String, ClientError> {
        use std::os::unix::net::UnixStream;
        if body.len() > INSPECTION_REQUEST_BYTES {
            return Err(ClientError::usage("the request exceeds 256 KiB"));
        }
        let remaining = || {
            deadline
                .checked_duration_since(Instant::now())
                .filter(|remaining| !remaining.is_zero())
                .ok_or_else(|| ClientError::timeout("the deadline passed before a reply"))
        };
        let mut stream = UnixStream::connect(&self.path).map_err(|error| {
            ClientError::connection(
                format!("cannot connect to {}: {error}", self.path.display()),
                "check the session or socket path; the application may have exited",
            )
        })?;
        let transport_error = |error: std::io::Error| {
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ) {
                ClientError::timeout("the application did not reply before the deadline")
            } else {
                ClientError::connection(
                    format!("the connection failed: {error}"),
                    "the application may have exited; check the session",
                )
            }
        };
        stream
            .set_write_timeout(Some(remaining()?))
            .map_err(transport_error)?;
        stream
            .write_all(body.as_bytes())
            .and_then(|()| stream.write_all(b"\n"))
            .map_err(transport_error)?;
        stream
            .set_read_timeout(Some(remaining()?))
            .map_err(transport_error)?;
        let mut reply = Vec::new();
        let mut reader = BufReader::new(stream).take(REPLY_BYTES as u64 + 1);
        reader
            .read_until(b'\n', &mut reply)
            .map_err(transport_error)?;
        if reply.len() > REPLY_BYTES {
            return Err(ClientError::new(
                "protocol_violation",
                "the reply exceeds its size bound",
                "report this application",
            ));
        }
        if reply.is_empty() {
            return Err(ClientError::connection(
                "the application closed the connection without replying",
                "it may have exited; check the session",
            ));
        }
        String::from_utf8(reply).map_err(|_| {
            ClientError::new(
                "protocol_violation",
                "the reply is not UTF-8",
                "report this application",
            )
        })
    }

    fn describe(&self) -> String {
        format!("socket {}", self.path.display())
    }
}

#[cfg(not(unix))]
impl Transport for SocketTransport {
    fn request(&mut self, _body: &str, _deadline: Instant) -> Result<String, ClientError> {
        Err(ClientError::connection(
            "native sockets need a Unix host",
            "attach to a browser page instead",
        ))
    }

    fn describe(&self) -> String {
        format!("socket {}", self.path.display())
    }
}

use std::io::Read as _;
