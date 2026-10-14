//! What every Stobie application needs, independent of UI, rendering and any
//! application domain.
//!
//! - [`Workspace`] is the sole serialisable dock tree, with stable [`PaneId`]s
//!   and [`DockNodeId`]s. [`Workspace::with_tabs`] builds a small layout; the
//!   dock reports interactions as [`WorkspaceChange`]s.
//! - [`History`] gives any application undo and redo over its own
//!   [`Command`]s: a capacity bound, grouped and merged entries and a save
//!   point. [`WorkspaceCommand`] makes layout changes undoable too.
//! - [`LogicalPoint`], [`PhysicalPoint`] and [`ViewportPoint`] keep coordinate
//!   spaces distinct across UI boundaries; [`virtual_rows`] and
//!   [`layout_virtual_grid`] materialise only visible rows of large
//!   collections; [`RepaintReason`] records why a frame was requested.
//!
//! Focus and hover belong to the UI integration, and application state belongs
//! to the application. Image applications add `stobie-imaging`. Start from the
//! [application composition guide](https://github.com/robchristie/stobie/blob/main/docs/application-composition.md).

pub mod coords;
pub mod diagnostics;
pub mod dock;
pub mod history;
pub mod ids;
pub mod virtualise;

pub use coords::*;
pub use diagnostics::*;
pub use dock::*;
pub use history::*;
pub use ids::*;
pub use virtualise::*;
