//! Build, package and preview Stobie applications for the web.
//!
//! This is the one web packaging implementation: the `stobie build-web` and
//! `stobie serve` commands and the repository's `cargo xtask build-web` and
//! production build all call it. It runs `cargo` for `wasm32-unknown-unknown`
//! and checks the `wasm-bindgen` CLI against the version the project locks,
//! writes the shared startup loader and, for an installable page, its web
//! app manifest and service worker, and optionally packages the result with
//! `wasm-opt`, Brotli and gzip into immutable, content-addressed assets.

pub mod package;
pub mod pwa;
pub mod serve;
mod web;

use std::fmt;
use std::path::Path;

pub use web::{
    BuildOptions, BuildReport, GENERATED, ProductionOptions, bindgen, build, cargo_metadata,
    ensure_wasm_bindgen, locked_wasm_bindgen, stage_web, wasm_bindgen_cli_version,
};

/// The shared browser startup loader that application bootstraps import as
/// `./browser-startup.js`. Builds write it beside `index.html`; it is never
/// copied by hand.
pub const STARTUP_LOADER: &str = include_str!("../web/browser-startup.js");

/// The loader's file name inside a web root.
pub const STARTUP_LOADER_FILE: &str = "browser-startup.js";

/// Write the startup loader into `directory`.
pub fn write_startup_loader(directory: &Path) -> Result<(), Error> {
    let path = directory.join(STARTUP_LOADER_FILE);
    std::fs::write(&path, STARTUP_LOADER)
        .map_err(|error| Error::new(format!("write {}: {error}", path.display()), ""))
}

/// A build or packaging failure with a hint for what to do next.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub message: String,
    pub hint: String,
}

impl Error {
    pub fn new(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: hint.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if !self.hint.is_empty() {
            write!(formatter, " ({})", self.hint)?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}
