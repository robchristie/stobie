# Generic core and undo history, with imaging as its own layer

Status: active
Next action: Move the image domain and the Lab's demo state out of `stobie-core` (increment 2), with saved-state fixture tests first.

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

## Increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 1. Generic history and workspace commands in core; Record Desk migration | In review | History and workspace-command unit tests and doctests; UI dock tests undo through the generic history; Record Desk model and hosted save-point tests; Record Desk stage (tests, native, browser and interface smokes) unchanged |
| 2. `stobie-imaging` crate; image types and Lab demo state move | Planned | Saved-state fixture tests added first |
| 3. Lab on the generic history; old types removed; docs, architecture rules and authoring probe | Planned | |

## Record Desk migration

The hand-rolled undo `VecDeque<Edit>`, redo `Vec<Edit>`, capacity trimming and
history revision are gone. `Edit` implements `Command<Vec<Record>>`, and
`Desk` keeps a `History<Edit>` bounded at 100 entries. Production `model.rs`
went from 572 to 575 lines: undo and redo shrank from 26 to 12 lines and
Apply's push, trim and revision bookkeeping from 12 to 5, while the `Command`
implementation and its record lookup add 19 lines and the save-point accessors
add 9. `app.rs` keeps the saved layout instead of a copy of the saved state.
