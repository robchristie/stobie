# Generic core and undo history, with imaging as its own layer

Status: complete
Delivery: [PR #73](https://github.com/robchristie/stobie/pull/73) completes the plan; increments 1 and 2 are [PR #71](https://github.com/robchristie/stobie/pull/71) and [PR #72](https://github.com/robchristie/stobie/pull/72).

## Outcome and boundaries

`stobie-core` holds only what every application needs: the dock workspace,
stable pane and dock-node IDs, generic coordinates, virtualisation, repaint
reasons and a generic undo history. The image domain moves to its own crate,
which image applications opt into, and the Analytical Workspace Lab owns its
demo state. Record Desk and the Lab both use the generic history.

The version 1 inspection protocol, capability IDs, semantic IDs, UI snapshot
baselines, smoke scripts and saved-state formats stay unchanged. Breaking Rust
API changes to the framework crates are allowed because every consumer is in
this repository; no compatibility shims remain at the end.

## History API

Checked against Record Desk, the Lab and the dock before implementation.

- `Command<T>` has `apply(&self, &mut T) -> Result<bool, String>`,
  `revert(&self, &mut T) -> Result<(), String>` and an optional
  `merge(&mut self, &Self) -> bool`. `T` is the target: Record Desk uses its
  record list, and the Lab uses a struct of mutable borrows of its document,
  session and workspace, built at the call site. The trait is generic over the
  target rather than using an associated type, so a borrowed context struct
  needs no generic associated type.
- `apply` returns `Ok(false)` when nothing would change, and the history then
  records nothing and keeps redo. This covers Record Desk's unchanged Apply
  and the Lab's empty camera and unchanged resize commands, which the old
  `CommandHistory` special-cased.
- Errors must leave the target unchanged, and the entry stays where it was.
  This keeps Record Desk's rule that undo of a missing record fails without
  losing the entry. The Lab's image commands keep their lenient reverts: a
  revert of a camera that no longer exists does nothing, as before.
- `History<C>` keeps at most `capacity` entries (100 by default) and forgets
  the oldest. `execute_group` applies several commands as one entry and rolls
  back on failure. `execute_merging` folds a command into the newest entry
  when `merge` accepts, until an undo, redo, save point or `seal`, and drops a
  merged command that `is_noop` reports.
- `mark_saved` and `is_saved` give a save point. Each entry has a unique state
  ID, so undoing back to a save is saved, while recommitting earlier values is
  a new change. Record Desk's "unsaved" now combines the save point with its
  saved layout (layout is saved state but not undoable) instead of comparing
  whole saved states.
- `revision` counts changes and replaces Record Desk's history revision in its
  action meanings.

## Workspace commands

`WorkspaceCommand` implements `Command<Workspace>`, so any application can make
layout undoable. `WorkspaceCommand::from_change` resolves the dock's
`WorkspaceChange` against the current workspace without changing it, and
returns `None` for a change that does nothing.

- `Resize` applies and reverts leniently: it sets the fraction while the split
  exists and otherwise does nothing, as the Lab's `ResizeSplit` did, so an undo
  after a direct layout change never sticks. Fractions outside 0.1–0.9 are
  errors. Consecutive resizes of one split can merge, and a merged run that
  returns to its start leaves no entry (`Command::is_noop`). Applications
  execute each resize separately by default, because each drag or nudge is a
  completed gesture.
- `Activate` records the pane, the tab its group showed and the previously
  active pane. Reverting shows that tab again and restores the active pane
  without changing what its group shows, since the active pane need not be a
  displayed tab; absent panes are skipped.
- `Rearrange` (pane moves) stores the layouts before and after and fails when
  the workspace no longer matches, because a structural change has no lenient
  inverse. An application that makes moves undoable routes every layout
  change through history.
- `dock_workspace` returns the generic `WorkspaceResize` instead of an image
  command, and `dock_workspace_layout`, its duplicate, is removed. The
  Gallery's dock story records resizes in a `History<WorkspaceCommand>`
  instead of the image history, which it used only for layout.

## Decisions

- **Increment 1 renames core's image `Command` enum to `ImageCommand`** so the
  generic trait can take the `Command` name. `CommandHistory` remains the
  image history until increment 3 removes it; until then the Lab and the
  minimal example record dock resizes as `ImageCommand::ResizeSplit`.
- **Workspace commands land in increment 1**, not increment 3, because the UI
  crate must stop returning an image command before increment 2 can move image
  types out of core.

## Imaging layer

- **One new crate, no renames.** `stobie-imaging` holds the image domain that
  was in core: cameras, regional types, image intents and commands, tile
  demands, documents, sessions, gesture previews, image coordinates
  (`ImagePoint`, `WorldPoint`, `ImageToWorld`), the layer, annotation, result,
  source and link-group IDs, and the render and runtime metrics. It depends
  on core for pane IDs, screen coordinates and virtual grids. `ImagePoint`
  moves too: only cameras, regions and the renderer use it.
- **`stobie-runtime` and `stobie-render-wgpu` keep their names and depend on
  `stobie-imaging` instead of core**, rather than merging into it. Merging
  would turn crate-level boundaries (the runtime has no egui or wgpu) into
  feature-level ones. An image application adds `stobie-imaging`, the runtime
  and the renderer it uses, and the UI crate's `image-rendering` feature.
  Renaming the runtime and renderer to `stobie-imaging-*` is deferred until a
  second renderer, such as the WebGL2 path, makes the names ambiguous.
- **The egui image code stays behind `image-rendering`**, moved from the UI
  crate's `lib.rs` into its own module. The feature is no longer a default, so
  the Gallery and Record Desk get the general framework without asking for it,
  and image applications opt in. The UI crate's `eframe` feature still leaves
  GPU backends to the application: `notes` gains an `eframe` dev-dependency
  with its `wgpu` feature, as Record Desk and the Gallery already declare.
- **Same wire, neutral payloads.** The protocol's `DomainReference` kinds
  `result`, `annotation` and `thumbnail` stay, but carry plain values
  (`u64`, and a `TileReference` with the tile key's fields) instead of image
  types, so the UI crate needs no imaging dependency. A test pins every
  kind's JSON before and after the change.
- **Demo state is the Lab's.** `apps/analytical-workspace-lab/src/demo.rs`
  holds the eight-pane layout (was `Workspace::analytical_default`), the
  starting session with cameras for panes 1–4, `ResultRecord`, `result_at` and
  the collection sizes; `metrics.rs` holds `DiagnosticsSnapshot` and
  `VirtualisationMetrics`. `Session::default` in the imaging crate has no image
  panes, and `Session::validate_cameras` takes the application's image panes
  instead of hard-coding 1–4.
- **`ImageIntent::RecenterOnResult` becomes `Recentre { pane, centre }`**,
  because synthetic results are Lab data. The Lab looks up the result's
  position; validation still reads the pane's camera when the intent applies.
- **The synthetic raster's constants stay in imaging.** `RASTER_SIZE`,
  `TILE_SIZE` and `PYRAMID_LEVELS` size the runtime's synthetic tiles and the
  default camera; parameterising the runtime by source is later work.
- **Saved state is unchanged.** Serde names do not depend on crate paths. RON
  and JSON fixtures, added before the move, pin the Lab's eframe value and
  Record Desk's version 1 file; both decode and re-encode byte for byte.

## Increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 1. Generic history and workspace commands in core; Record Desk migration | Complete | [PR #71](https://github.com/robchristie/stobie/pull/71): History and workspace-command unit tests and doctests; UI dock tests undo through the generic history; Record Desk model and hosted save-point tests; Record Desk stage (tests, native, browser and interface smokes) unchanged |
| 2. `stobie-imaging` crate; image types and Lab demo state move | Complete | [PR #72](https://github.com/robchristie/stobie/pull/72): Saved-state fixtures and a domain-reference wire test added first and unchanged after the move; workspace tests; `cargo tree` shows the Gallery and Record Desk exclude imaging, runtime and renderer (increment 3 enforces it) |
| 3. Lab on the generic history; old types removed; docs, architecture rules and authoring probe | Complete | [PR #73](https://github.com/robchristie/stobie/pull/73): Workspace tests; architecture-rule regression tests and temporary-violation checks; probe below |

## Lab on the generic history

- `ImageCommand` implements `Command<ImageTarget<'_>>`, where `ImageTarget`
  borrows the image document and session. Empty or unchanged camera commands
  return `Ok(false)`, so they are not recorded, as before. Reverts stay
  lenient. `ImageCommand::ResizeSplit`, `UndoScope` and `CommandHistory` are
  removed.
- The Lab's `LabCommand` wraps `ImageCommand` and core's `WorkspaceCommand`
  over a `LabTarget` of mutable borrows of its document, session and
  workspace. A changing resize becomes a lenient `WorkspaceCommand::Resize`
  in the image history, as `ResizeSplit` was, so a drag released after an
  undo or reset still applies; tab activation and moves still apply
  directly. The browser fixture's `resize_split` action keeps its wire name
  and clamps its fraction, as before.
- The Lab's history keeps 1,000 entries. It was unbounded; camera moves are
  undoable, so the bound is generous, and no test or smoke comes near it.
- The minimal example records image commands in a `History<ImageCommand>` and
  applies resizes directly, because it has no undo action.

## Architecture rules

`cargo xtask architecture` now also checks that:
- `stobie-core`'s dependency tree has no `stobie-imaging`, runtime or
  renderer on any target, and its sources define none of the listed image or
  Lab-demo item names (a single-line definition scan with its own regression
  test, a tripwire rather than a proof); core cannot re-export image types it
  cannot reach;
- the Gallery and Record Desk resolve no image-layer crate on any target, and
  reach `wgpu` only through eframe and its `egui-wgpu` backend.

Temporary violations (enabling `image-rendering` in the Gallery, defining a
`Camera` in core) fail the check with a specific message.

## Authoring probe

A fresh-context agent was given only the composition guide, its links, the
README, the `notes` example and the generated rustdoc. It made Add, Remove last
and Clear undoable, with Clear removing each note as a separate step in one
`execute_group` entry. It also added Undo and Redo actions with availability
and a history-revision `meaning`. It built on the first attempt with no
compile errors and read no function bodies. The host has no display, so the
agent could not run the app.

Its `notes` example is adopted as the compiled undo and redo example, with
unit tests of Clear, Add and Remove last through undo and redo. Driven
natively under the repository's Xvfb sysroot with `tools/application-cli.mjs`:
- Clear removed both notes as one undo step, and Undo restored them.
- Redo cleared them again.
- A second Undo was rejected while nothing was left to undo.
- Remove last undid correctly.

These stumbles were fixed:
- The guide's sample could panic despite the rule that errors leave the target
  unchanged. The guide now shows the example's ID-checked `NoteEdit`.
- Nothing said how commands keep identity. The guide now says to carry whole
  items with stable IDs and to allocate IDs outside history.
- The guide gave no import path. It now names `stobie_core::{Command, History}`.
- The order in which a group is undone was undocumented. `undo` and `redo`
  rustdoc and the guide now say: reverse order to undo, forward order to redo.
- Nothing warned that direct changes make recorded commands stale. The guide
  now says to route every change to history-managed state through history.
- It was unclear whether text fields still see `Cmd+Z`. The guide now says that
  an available Undo takes the chord even while a text field has focus, and an
  unavailable one lets it through.
- There was no way to see the next redo. `History::last_redo` now pairs with
  `last_undo`.
- It was unclear whether undo restores drafts. The guide now says undo
  restores committed state, not drafts.
- The `Application` rustdoc pointed to a nonexistent "module guide". It now
  points to the crate documentation and the guide.

The example's old `Cmd+Z` for Remove last clashed with Undo and moved to
`Cmd+Backspace`. The derive rejects duplicate chords at compile time, so the
clash could not have shipped.

## Effects on later work

- The ARIA mirror and the MCP server are unaffected: the semantic snapshot and
  inspection protocol are unchanged. `History::revision` gives applications a
  ready input for action meanings that agents rely on.
- WebGL2 becomes easier to scope: applications without images no longer
  resolve the image renderer, so a WebGL2 path needs only an eframe backend
  change for them. The image renderer stays wgpu-only.
- Action arguments and availability that reads UI state are unaffected.

## Record Desk migration

The hand-rolled undo `VecDeque<Edit>`, redo `Vec<Edit>`, capacity trimming and
history revision are gone. `Edit` implements `Command<Vec<Record>>`, and
`Desk` keeps a `History<Edit>` bounded at 100 entries. Production `model.rs`
went from 572 to 575 lines: undo and redo shrank from 26 to 12 lines and
Apply's push, trim and revision bookkeeping from 12 to 5, while the `Command`
implementation and its record lookup add 19 lines and the save-point accessors
add 9. `app.rs` keeps the saved layout instead of a copy of the saved state.
