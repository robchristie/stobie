//! Application state and validated transitions, independent of UI and rendering.
//!
//! Keep durable annotations in [`Document`], transient selection, cameras, tools
//! and gesture previews in [`Session`], and the sole serialisable dock tree in
//! [`Workspace`]. Focus and hover belong to the UI integration. The defaults are
//! the analytical demo's starting state; applications can construct smaller
//! states explicitly. Coordinate newtypes preserve domain distinctions at each
//! boundary; do not substitute untyped screen points for [`WorldPoint`].
//!
//! Collect feature [`ImageIntent`]s from a narrow pane view, call
//! [`validate_intent`] against the current state and execute successful commands
//! through [`CommandHistory`]. History applies and reverses commands; it does not
//! validate arbitrary constructed commands. One completed gesture should produce
//! one history entry; render its [`GesturePreview`] in the current frame.
//!
//! ```
//! use polyorama_core::{CommandHistory, Document, ImageIntent, LayerId, Session,
//!     Workspace, WorldPoint, validate_intent};
//! let mut document = Document::default();
//! let mut session = Session::default();
//! let mut workspace = Workspace::analytical_default();
//! let command = validate_intent(ImageIntent::CommitPolygon {
//!     layer: LayerId(1),
//!     vertices: vec![WorldPoint::new(0.0, 0.0), WorldPoint::new(1.0, 0.0),
//!                    WorldPoint::new(0.0, 1.0)],
//! }, &mut document, &session)?;
//! let mut history = CommandHistory::default();
//! history.execute(command, &mut document, &mut session, &mut workspace);
//! assert_eq!(document.annotations.len(), 1);
//! assert!(history.undo(&mut document, &mut session, &mut workspace));
//! assert!(document.annotations.is_empty());
//! # Ok::<(), String>(())
//! ```
//!
//! [`TileDemand`] describes renderer-independent desired work, not a work queue.
//! Use `polyorama-runtime` to reconcile complete desired sets and reject stale
//! completions. See the
//! [application composition guide](https://github.com/robchristie/polyorama/blob/main/docs/application-composition.md)
//! for the four-crate lifecycle and a small native consumer.

pub mod camera;
pub mod commands;
pub mod coords;
pub mod data;
pub mod diagnostics;
pub mod dock;
pub mod ids;
pub mod regional;
pub mod virtualise;

pub use camera::*;
pub use commands::*;
pub use coords::*;
pub use data::*;
pub use diagnostics::*;
pub use dock::*;
pub use ids::*;
pub use regional::*;
pub use virtualise::*;
