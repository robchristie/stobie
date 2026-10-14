//! The image domain: documents, sessions, cameras, image intents and commands,
//! and renderer-independent tile and regional demands.
//!
//! Image applications opt into this crate; it builds on `stobie-core`'s pane
//! IDs, coordinates and dock workspace. Keep durable annotations in
//! [`Document`] and transient selection, cameras, tools and gesture previews in
//! [`Session`]. Focus and hover belong to the UI integration. A new
//! [`Session`] has no image panes: add a [`CameraState`] and an
//! [`ActiveTool`] for each, and check restored sessions with
//! [`Session::validate_cameras`]. Coordinate newtypes preserve domain
//! distinctions at each boundary; do not substitute untyped screen points for
//! [`WorldPoint`].
//!
//! Collect feature [`ImageIntent`]s from a narrow pane view, call
//! [`validate_intent`] against the current state and execute the resulting
//! [`ImageCommand`] in core's generic `History`, with an [`ImageTarget`]
//! borrowing the document and session. History applies and reverses commands;
//! it does not validate arbitrary constructed commands. One completed gesture
//! should produce one history entry; render its [`GesturePreview`] in the
//! current frame. An application whose commands also change other state, such
//! as its layout, wraps `ImageCommand` in its own command type, as the Lab does.
//!
//! ```
//! use stobie_core::History;
//! use stobie_imaging::{Document, ImageIntent, ImageTarget, LayerId, Session,
//!     WorldPoint, validate_intent};
//! let mut document = Document::default();
//! let mut session = Session::default();
//! let command = validate_intent(ImageIntent::CommitPolygon {
//!     layer: LayerId(1),
//!     vertices: vec![WorldPoint::new(0.0, 0.0), WorldPoint::new(1.0, 0.0),
//!                    WorldPoint::new(0.0, 1.0)],
//! }, &mut document, &session)?;
//! let mut history = History::default();
//! history.execute(command, &mut ImageTarget::new(&mut document, &mut session))?;
//! assert_eq!(document.annotations.len(), 1);
//! assert!(history.undo(&mut ImageTarget::new(&mut document, &mut session))?);
//! assert!(document.annotations.is_empty());
//! # Ok::<(), String>(())
//! ```
//!
//! [`TileDemand`] describes renderer-independent desired work, not a work queue.
//! Use `stobie-runtime` to reconcile complete desired sets and reject stale
//! completions, and `stobie-render-wgpu` for GPU presentation. See the
//! [image application guide](https://github.com/robchristie/stobie/blob/main/docs/image-application-composition.md)
//! for the image lifecycle and a small native consumer.

pub mod camera;
pub mod commands;
pub mod coords;
pub mod data;
pub mod diagnostics;
pub mod ids;
pub mod regional;

pub use camera::*;
pub use commands::*;
pub use coords::*;
pub use data::*;
pub use diagnostics::*;
pub use ids::*;
pub use regional::*;
