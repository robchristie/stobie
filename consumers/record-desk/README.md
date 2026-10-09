# Record Desk

A maintained independent Polyorama consumer for finding, reviewing and editing
12 deterministic synthetic records. It uses only public `polyorama-core` and
`polyorama-ui-egui` APIs. Its own Cargo workspace, dependency declarations,
profiles and lockfile do not inherit the product workspace. UI's image-rendering
feature is disabled: neither Polyorama's image renderer nor its worker runtime
is needed. Eframe still owns the host's GPU rendering of egui.

## Build and run

Run from this directory (`consumers/record-desk`). Use the repository's Rust
1.99.0 toolchain. Native needs a graphical session and a wgpu-compatible backend;
Linux enables X11 and Wayland. See the framework
[prerequisites](../../README.md#prerequisites).

```sh
cargo build --locked --release
cargo run --locked --release
```

For browser development, install the WASM target and matching bindgen CLI once:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.127
bash build-web.sh
python3 -m http.server 8090 --bind 127.0.0.1 --directory web
```

Open [Record Desk](http://localhost:8090) in a WebGPU-capable browser. Serve over
HTTP, then rebuild and reload after Rust changes. No worker or data service is
needed. For a configured private HTTPS development gateway:

```sh
dev-preview up --worktree ../.. --json
```

Append `/record-desk/` to the returned URL. The gateway requires the canonical
Git worktree root; its root adapter adds an explicit consumer asset allowlist.
Launcher readiness is separate from browser workflow verification. The framework
[browser startup guide](../../docs/browser-startup.md) covers production
packaging; this consumer supplies a small development launcher with visible
startup failure reporting, not production hosting.

## Workflow and state ownership

`model.rs` owns records, stable `RecordId`s, filters, selected identity, editing
draft and bounded transaction history. `app.rs` owns one core `Workspace`, dock
interaction previews, persistence feedback and the latest read-only observation.
`panes.rs` receives narrow record views and emits `Intent`s; it has no mutable
access to the model, complete application, runtime or GPU resources. The
[composition guide](../../docs/application-composition.md) and
[UI guides](../../docs/ui-guides/README.md) own the framework contracts.

Search is case-insensitive over title and notes; category and review filters
combine with it. Filtering out a selection keeps its stable identity and draft
in the editor with an explicit message. No other record is selected implicitly.
Reset filters clears search, category and review together, preserving that
selection and any editing draft. It is available only when a filter differs
from its default, including a non-empty search field.
Changing selection with a dirty draft is rejected visibly: Apply or Cancel first.
No matching records gives a useful recovery message. Empty stored collections
are valid and show a choose-record state.

Fields edit a transient draft. Apply validates non-empty title (up to 120
characters), notes (up to 2000 characters), supported category and stable ID;
title and notes are trimmed at commit. Invalid or cancelled edits do not change
records or history. A successful changed Apply creates one transaction regardless
of the number of edited fields; unchanged normalised drafts create none. Undo
and redo reverse/reapply committed record changes and refresh the selected
draft. They require Apply or Cancel first if a draft is dirty. A new Apply clears
redo. History retains at most 100 transactions and does not survive restart.

The application presents views, gathers draft/filter outputs, then handles
selection/actions against current state. This order makes a same-pass Apply
consume the current draft. Each mutation requests one subsequent presentation;
egui handles input/animation invalidation. Inspection reads do not repaint.
`dock_workspace_layout` returns a layout-only `WorkspaceResize`, validated via
`apply` without a fabricated image `Document` or `Session`. Tab moves/activation
update the same canonical tree directly. Dirty tracking compares the complete
saved envelope, so these changes are included too.

Below 640 points the root split stacks the panes in the authoritative tree.
Local scrolling retains editor actions and list content. Arrange panes toggles
the root orientation on wider screens (or resets a tabbed layout to the default).
The stacked arrangement can be retained with Save.
Arrange is unavailable for a split at narrow width because that layout remains
stacked; it can still recover a tabbed tree into the two-pane arrangement.

## Keyboard

Tab/Shift+Tab reaches search, category/review choices, record rows, editor fields
and actions; Enter/Space activates focused choices/rows/actions. Combo boxes use
the native egui keyboard path. Dock tabs support their framework roving keys and
splitters support arrow-key adjustments.

| Shortcut | Action |
|---|---|
| Ctrl/Cmd+F | Activate the list and focus search |
| Ctrl/Cmd+Shift+F | Reset search, category and review filters |
| Ctrl/Cmd+Enter | Apply |
| Ctrl/Cmd+Shift+Enter | Cancel draft |
| Ctrl/Cmd+Z / Ctrl/Cmd+Shift+Z | Undo / redo a committed change |
| Ctrl/Cmd+S | Save committed records and layout |
| Ctrl/Cmd+Shift+R | Restore (requires no unsaved state/draft, or a repaired load error) |
| Ctrl/Cmd+L | Arrange panes |

Unavailable actions retain reasons and their application shortcuts do not consume
input. The browser launcher reserves registered chords while the canvas or its
text input has focus, preventing browser Find, Location and Reload actions from
stealing that focus. Copy/paste, ordinary text input and unregistered shortcuts
retain their browser behaviour.
Browser semantic/keyboard evidence does not claim a browser accessibility adapter
or screen-reader support; see the framework's
[accessibility limits](../../docs/ui-guides/accessibility.md).

## Extend it

Add a record field to `Record`, its defaults and `validate_record`; add an editor
control using a stable field key derived from `RecordId`. Emit the complete
`Intent::Draft` without mutating committed data. The existing Apply transaction
then snapshots the new field automatically; add a focused validation/history
test. Changes to the durable shape require a deliberate persistence version and
migration decision; unsupported versions must remain protected.

For a control, use `PresentationContext` and shared recipes, tokens and measured
text. Native editable controls use `NativeTextControlKind::TextEdit` and retain
their explicit audit exclusion. Use `choice_control_with_options` to retain
current floating popup targets; do not clip these option nodes to the pane.
Browser read-only app inspection also requires eframe's WASM `App::as_any_mut`
hook. Record rows use a consumer-owned measured recipe with stable
domain/semantic identity and the native egui activation behaviour.
Custom controls must keep their AccessKit name, description, state and current
visible bounds consistent with their semantic observation. Shared dock splitter
names describe the divider's orientation, perpendicular to the split axis.
The consumer owns observation visibility: toolbar actions are omitted only when
a valid allocation is completely clipped in the current sizing pass. Partial
targets keep their visible bounds; invalid allocation evidence remains auditable.
`tests/workflow_ui.rs` audits the actual AccessKit tree at ordinary/narrow sizes
with full records and open/closed options, and checks semantic/text observations
from every startup presentation. Run it when adding interactive chrome.
For an action, extend `Action`, its stable ID/specification, availability and the
application action handler. Emit `Intent::Action`; route the registered shortcut
through the same availability rule. Domain operations belong in `Desk`, not panes.
If a new browser shortcut conflicts with a browser default, add its exact chord
to the focus-scoped reservation in `web/bootstrap.js`; leave the event forwarding
to eframe intact.
Add a model or UI test that proves the behavioural invariant rather than the enum.
When shortcuts share a key, match their declared Shift/Alt modifiers before
calling `consume_action_shortcut`: egui's logical matcher accepts extra
modifiers. An unavailable Shift shortcut must not activate its plain counterpart.
The Reset filters tests exercise the button, shortcut, disabled input and
preservation of a hidden selection with an editing draft through public UI input.

## Persistence

Save explicitly writes **committed records and the canonical layout**, excluding
drafts, filters, selection, focus and history. Saving while editing says that the
draft was excluded. Restarts/reloads restore saved state and start fresh history;
there is no automatic exit save.

The UTF-8 JSON envelope is version 1:
`{ "schema_version": 1, "records": [...], "workspace": {...} }`. Records have
`id`, `title`, `category` (`Research`, `Operations`, `Ideas`), `notes` and
`reviewed`. The embedded layout uses core's layout schema version. Loading
validates record identities/fields/count, layout structure and the exact two
consumer panes; input is limited to 1 MiB and 1000 records.

Native uses `$XDG_STATE_HOME/polyorama-record-desk/state.json`, falling back to
`$HOME/.local/state/polyorama-record-desk/state.json`. Set
`RECORD_DESK_STATE_PATH=/absolute/path/state.json` for an isolated desk. Writes
sync a temporary file in the same directory then atomically replace the state.
Browser uses the current origin's `localStorage` key
`polyorama.record-desk.v1`. Keeping the same origin/port preserves local state.

Missing saved data starts the synthetic collection. Malformed, unsupported,
unreadable or oversized state reports an error and blocks Save. Existing state
is checked again before replacement. Repair or move the file/key externally,
cancel any draft, then Restore to clear the hold. The app never silently replaces
unrecognised saved state. Storage write/quota failures retain the unsaved state
and show the error. Files/localStorage are local to this device and are not a
backend, backup or cross-device synchronisation service.

## Verify and inspect

```sh
python3 ../../tools/test-rust.py record-desk
cargo fmt --package record-desk --check
cargo clippy --locked --all-targets -- -D warnings
bash build-web.sh
python3 ../../tools/check-record-desk.py
```

The last command verifies workspace/dependency independence, formatting, tests,
native/WASM lint/build and browser packaging; it is also in `cargo xtask verify`.
The test command retains the consumer's locked default selection with Nextest
**0.9.146** or later, followed by separate Cargo doctests. Its JUnit report is
`../../.tools/runtime/verification-evidence/nextest/record-desk/junit.xml`,
distinct from both framework test configurations and retained on test failure.
From the product root, physical workflows use:

```sh
bash tools/record-desk-native-smoke.sh
bash tools/record-desk-browser-smoke.sh
```

These operate current semantic/control geometry using OS/Playwright pointer and
keyboard input, and retain step snapshots/captures in ignored runtime evidence.
The browser journey waits for an enabled target with stable current geometry
before each pointer event; a bounded timeout fails visibly. Reports retain the
frame and rectangle used, including popup options that need following passes.
`RECORD_DESK_SNAPSHOT=/absolute/path/snapshot.json` enables native read-only
observations; browser `window.__RECORD_DESK.snapshot()` returns the same JSON.
The hook supplies no application commands. Direct model/UI tests and physical
runtime evidence are distinguished in the maintained
[consumer evidence](../../docs/record-desk-evidence.md), including opened
ordinary/narrow images and the fresh-context extension probe.
