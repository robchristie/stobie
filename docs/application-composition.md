# Compose a Stobie application

A Stobie application declares its state, its actions, its UI and how it
applies changes. The framework host owns everything else in each UI pass:
semantic observation and audits, shortcuts, availability checks, agent
inspection and invocation, and repaint decisions. The same code runs natively
and in the browser.

The complete example is [`notes.rs`](../crates/stobie-ui-egui/examples/notes.rs),
about 200 lines with no inspection code. Run it from the repository root with
the native [prerequisites](../README.md#prerequisites):

```sh
cargo run -p stobie-ui-egui --example notes
```

## Write the four parts

**Actions.** Declare every user action once. The derive checks labels,
shortcuts and stable IDs at compile time; IDs default to
`<prefix>.<kebab-case variant>`.

```text
#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "notes")]
enum NoteAction {
    #[action(label = "Add note", description = "Add the draft as a note", shortcut = "Cmd+Enter")]
    Add,
    #[action(label = "Clear", description = "Remove every note", shortcut = "Cmd+Shift+K")]
    Clear,
}
```

**State and availability.** Implement `Application` for your state. One
`availability` rule serves buttons, shortcuts, discovery and agents.

```text
impl Application for Notes {
    type Action = NoteAction;
    type Intent = NoteIntent;   // data changes, such as an edited draft
    type UiState = ();          // UI-only state, such as a pending focus request

    fn identity(&self) -> ApplicationIdentity { application_identity!("Notes") }

    fn availability(&self, target: ActionTarget<NoteAction>) -> Availability {
        match target.action {
            NoteAction::Add if self.draft.trim().is_empty() =>
                Availability::Disabled { reason: "Type a note first".into() },
            _ => Availability::Enabled,
        }
    }
```

**UI.** `ui` borrows the application immutably. It hands panes a narrow read
view, and the host's dock presents them inside a panel:

```text
    fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, NoteAction, NoteIntent>) {
        let mut panes = NotePanes { notes: &self.notes, draft: &self.draft, status: &self.status };
        egui::CentralPanel::default().show(ui, |ui| frame.dock(ui, &self.workspace, &mut panes));
    }
```

A pane reads its view, emits data intents and presents actions. `Frame::action`
resolves availability and triggers the action when clicked; `status` publishes
a status line that agents can wait for.

```text
impl Panes<NoteAction, NoteIntent> for NotePanes<'_> {
    fn title(&self, _pane: PaneId) -> &'static str { "Notes" }
    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: PaneId, frame: &mut Frame<'_, NoteAction, NoteIntent>) {
        let mut p = frame.pane_presentation(ui, PresentationScope::new("notes"), pane);
        p.heading(ui, "title", "Notes");
        let mut draft = self.draft.to_owned();
        // ... a TextEdit for `draft`, observed as a text input (see the example)
        if draft != self.draft { frame.emit(NoteIntent::EditDraft(draft)); }
        frame.action(ui, &mut p, NoteAction::Add, ActionEmphasis::Primary);
        p.status(ui, "status", self.status, false);
    }
}
```

The application owns the dock layout. `Workspace::with_tabs(&[NOTES_PANE])`
builds a single-pane layout. The dock emits tab, move and resize changes as
your intent through `From<WorkspaceChange>`.

**Changes.** `apply` handles data intents. `perform` handles actions from
clicks, shortcuts and agents alike, after the host rechecks availability. Both
report whether state changed or return an error, which `report_error` shows.

```text
    fn apply(&mut self, intent: NoteIntent, _: &mut ()) -> Result<bool, String> { ... }
    fn perform(&mut self, target: ActionTarget<NoteAction>, _: &mut ()) -> Result<bool, String> { ... }
    fn report_error(&mut self, message: String) { self.status = message; }
}
```

Start it with `stobie_ui_egui::run_native("Notes", options, |_| Notes::new())`.

## Inspect and invoke it

Enable automation with a private socket, then discover and invoke actions with
the shared [application interface](application-interface.md) client:

```sh
AUTOMATION_DIR=$(mktemp -d)
STOBIE_AUTOMATION_SOCKET="$AUTOMATION_DIR/notes.sock" cargo run -p stobie-ui-egui --example notes
node tools/application-cli.mjs --socket "$AUTOMATION_DIR/notes.sock" discover '{}'
node tools/application-cli.mjs --socket "$AUTOMATION_DIR/notes.sock" invoke '{"capability":"notes.clear"}'
```

Every action is invocable unless it declares `physical_only = "reason"`. Use
that for actions that choose a value: actions with arguments are not supported
yet, so text such as a draft can only be typed physically. Without the
environment variable, no automation starts. Use a short, fresh directory for
the socket: Unix socket paths are limited to about 100 bytes, and a socket left
by a killed process blocks the next start until it is removed. Startup failures
are printed to stderr and shown through `report_error`.

## Know what the host does

Each pass, the host runs `prepare`, then queued agent invocations, then
shortcuts, then your `ui`. Shortcuts need exact modifiers and an available
action. A shortcut without `Cmd` does not fire while a text field has focus, so
give text-adjacent actions a `Cmd` chord. It
publishes the completed observation and calls `completed`. Then it applies
this pass's data intents in emission order (`order_intents` can rearrange
them), followed by its actions, rechecking each action's availability first. It repaints only when state changed, an error was
shown, a dock drag is active or you call `Frame::request_repaint(reason)`. The
`AppHost` rustdoc gives the exact contract.

Presentation contexts, choice and range recipes, and the dock register their
semantic nodes and measured text automatically. Each is clipped to the region
it was drawn in, and a discarded layout pass is replaced rather than merged.
For a custom widget, call `PresentationContext::observe_node` and
`observe_text`, or the free `observe_node`, with the pane's semantic node as
parent. Record native egui text fields with
`PresentationContext::native(NativeTextControlKind::TextEdit, ...)` and observe
them as `UiRole::TextInput`. Observe a selectable list row as
`UiRole::ResultRow` with `selected` and, for a done or checked state,
`checked`. Do not observe a recipe's own nodes again.

## Keep these rules

- Change state only in `apply`, `perform` and `prepare`. Keep focus requests
  and other UI-only state in `UiState`.
- Derive presentation scopes and keys from stable domain IDs, never from labels,
  positions or counters.
- Give a pane-scoped action (`scope = "pane"`) its pane by presenting it inside
  `Frame::pane_presentation`, or pass `ActionTarget::pane(action, pane)`;
  shortcuts reach it through `Application::active_pane`.
- Override `meaning` when an action's effect depends on state an agent could
  have observed, such as a selection, a draft or history depth. A target
  discovered before that state changed is then rejected as stale. Leave focus,
  layout and counters out of it.
- Publish bounded facts with `observe`: `fact(key, value)` for booleans,
  integers and text, and `counted_collection` for lists. Facts have no null;
  use an empty string for "none". Never publish whole collections.
- Never request an unconditional repaint.
- Override `preferences` and `theme` to change appearance; the host reapplies
  the design system when they change and resolves `Frame::tokens` from them.

## Grow to several panes

`Frame::dock` presents the single core `Workspace` read-only, so the tree is
yours to build: `Workspace::with_tabs` for tab groups, or a `DockNode::Split` of
tab groups for side-by-side panes, then check it with `Workspace::validate`.
Apply a dock change with `change.apply(&mut self.workspace)` in `apply`, and
adapt the tree to the viewport in `prepare`. Tab titles are static; show
changing counts in pane content.
[Record Desk](../consumers/record-desk/README.md) is a two-pane reference:
`app.rs` holds state, rules and handlers, `panes.rs` the pane UI, and
`model.rs` and `store.rs` the domain and persistence.

## Run in the browser

Put the application in `lib.rs`, with a thin `main.rs` that starts it with
`run_native`. Build the library as `crate-type = ["cdylib", "rlib"]` with a direct
`wasm-bindgen` dependency pinned to your bindgen CLI's version, then export a
handle:

```text
#[cfg(target_arch = "wasm32")]
stobie_ui_egui::web_handle!(Notes, |_| Notes::new());
```

The page creates `new WebHandle()` and awaits `start(canvas)`. With
`?automation=1` in its URL, it calls `enable_automation(crypto.randomUUID())`
and exposes `automation_request` as `window.__STOBIE_AUTOMATION.request`.
Record Desk's [`web/bootstrap.js`](../consumers/record-desk/web/bootstrap.js)
and `build-web.sh` are a working template. For page hooks beyond these, write
the handle around `WebHost` yourself: `with_host` reads the running host and
`with_host_mut(|host| host.submit(intent))` sends an intent through `apply`.
The component Gallery's `lib.rs` is an example. The
[browser startup guide](browser-startup.md) covers packaging.

## Choose dependencies

Copy Record Desk's [`Cargo.toml`](../consumers/record-desk/Cargo.toml): it
depends on `stobie-core` and on `stobie-ui-egui` with
`default-features = false, features = ["eframe"]`. Enable the default
`image-rendering` feature only for the GPU image path described in
[Compose an image application](image-application-composition.md).

## Generate and check the API documentation

```sh
cargo xtask docs
```

This generates the framework crates' rustdoc in `target/doc/`, runs the
selected examples, checks the `notes` example and the image guide's
`minimal-workspace` example natively and for WASM, and checks guide links. `cargo xtask verify` includes it. The
[documentation evidence](api-documentation-evidence.md) records exercised
environments.
