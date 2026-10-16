//! A client for the Stobie application interface.
//!
//! [`Client`] binds to one application instance through a [`Transport`]:
//! [`SocketTransport`] for a native application's private Unix socket, or
//! `BrowserTransport` (the default `browser` feature) for a page opened with
//! `?automation=1`, through Chromium's DevTools protocol. It
//! observes, queries and discovers; invokes capabilities with typed arguments,
//! waiting for receipts and for the observation that presents their effect;
//! sets framework text fields; and waits for conditions. It holds no CLI
//! formatting, so other front ends can reuse it.

#[cfg(feature = "browser")]
mod browser;
mod client;
mod condition;
mod error;
mod transport;

#[cfg(feature = "browser")]
pub use browser::{BrowserTransport, NOT_STARTED, PageSelector};
pub use client::*;
pub use condition::Condition;
pub use error::{ClientError, ErrorClass};
pub use transport::{REPLY_BYTES, SocketTransport, Transport};

pub use stobie_protocol as protocol;

#[cfg(test)]
mod tests;
