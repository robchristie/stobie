//! The Stobie application interface without egui: the version 1 inspection
//! wire protocol, typed argument contracts, and the semantic, action and text
//! observation types it carries. Applications reach it through
//! `stobie-ui-egui`, which re-exports everything here; clients such as the
//! `stobie` CLI depend on it alone.
//!
//! The optional `emath` feature converts egui's geometry to these types.

pub mod actions;
pub mod inspection;
pub mod semantics;
pub mod text;

pub use actions::*;
pub use inspection::*;
pub use semantics::*;
pub use text::*;

/// Pane and dock identities that selectors and domain references carry.
pub use stobie_core::{DockNodeId, PaneId};
