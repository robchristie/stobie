# Compose a Stobie application

A Stobie application declares its state, its actions, its UI and how it
applies changes. The framework host owns everything else in each UI pass:
semantic observation and audits, shortcuts, availability checks, agent
inspection and invocation, and repaint decisions. The same code runs natively
and in the browser.

To start an application in its own repository, run `stobie new`; see
[Start a new app](start-a-new-app.md). This guide explains the parts it
generates. The complete example is [`notes.rs`](../crates/stobie-ui-egui/examples/notes.rs),
about 350 lines plus tests of its undo and redo, with no inspection code.
Run it from the repository root with the native
[prerequisites](../README.md#prerequisites):

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

An application with no actions yet declares an enum with no variants, such as
`enum NoAction {}`, with the same derives and prefix; its `availability` and
`perform` are `match target.action {}`.

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
resolves availability and triggers the action when clicked; `Frame::text_field`
presents a labelled field and emits each edit as an intent; `status` publishes
a status line that agents can wait for. Each recipe publishes its own semantics,
so agents can fill the draft with `stobie set-text notes.draft TEXT` and press
Add.

```text
impl Panes<NoteAction, NoteIntent> for NotePanes<'_> {
    fn title(&self, _pane: PaneId) -> &'static str { "Notes" }
    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: PaneId, frame: &mut Frame<'_, NoteAction, NoteIntent>) {
        let mut p = frame.pane_presentation(ui, PresentationScope::new("notes"), pane);
        p.heading(ui, "title", "Notes");
        let draft = TextFieldSpec::new("New note").id("notes.draft");
        frame.text_field(ui, &mut p, "draft", &draft, self.draft, NoteIntent::EditDraft);
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
the [`stobie` command](agent-cli.md), which speaks the shared
[application interface](application-interface.md):

```sh
stobie launch notes --package stobie-ui-egui --example notes
stobie discover
stobie invoke notes.clear
stobie stop notes
```

Every action is invocable unless it declares `physical_only = "reason"`, for
example because it opens a menu. An action that chooses a value declares an
`argument` (text, boolean, bounded number or a `ChoiceArgument` choice) and
implements `Application::perform_with`, which receives the validated value;
apply the same intent its control emits. An action presented once per row,
such as Open on each person, passes a `BoundAction` carrying the row's ID to
the frame's recipes, and `Application::availability_with` decides
availability for each row; see
[Actions on rows](ui-guides/interactions.md#actions-on-rows). Every text field presented through
`PresentationContext::native` and observed as a text input can be set through
the framework's `stobie.set-text` capability without application code, and
every link presented through `PresentationContext::link` can be opened through
`stobie.open-link`, always in a new browser tab. Without the
environment variable, no automation starts. Use a short, fresh directory for
the socket: Unix socket paths are limited to about 100 bytes, and a socket left
by a killed process blocks the next start until it is removed. Startup failures
are printed to stderr and shown through `report_error`.

## Know what the host does

Each pass, the host runs `prepare` (which also receives UI state, for example
to turn worker results into textures), then queued agent invocations, then
shortcuts, then your `ui`. Shortcuts need exact modifiers and an available
action. A shortcut without `Cmd` does not fire while a text field has keyboard
focus, so typing reaches the field; give text-adjacent actions a `Cmd` chord. A
text field is an egui `TextEdit`, or a custom control you observe with the
`TextInput` or `MultilineTextInput` role and `focused` set. Other focus, such
as a clicked viewport, does not block shortcuts, except that plain `Enter` and
`Space` yield to a focused button, tab or other control that they activate. A
viewport observed with the `Viewport` role and `focused` set still receives
them. It publishes the completed observation and calls `completed`. Then it
applies this pass's data intents in emission order (`order_intents` can
rearrange them), followed by its actions, rechecking each action's availability
first. It repaints only when state changed, an error was shown, a dock drag is
active or you call `Frame::request_repaint(reason)`;
`Frame::request_repaint_after(reason, delay)` schedules a later pass, and
`PassContext` offers both to `prepare`. A change in whether the page or window
can be seen reaches `Application::visibility_changed`, and then intents sent
from outside a pass apply, before `prepare`, and present in that pass (see
[Receive events from outside](#receive-events-from-outside)). To
persist to eframe storage, implement `Application::save`; return true from
`take_save_request` after an action such as "Save layout" to save at once. The
`AppHost` rustdoc gives the exact contract.

Presentation contexts, choice and range recipes, and the dock register their
semantic nodes and measured text automatically, and a discarded layout pass is
replaced rather than merged. The host publishes them as presented, so the audits
report clipped and overflowing content. Dock panes scroll, so inside a pane it
publishes only what is visible: nodes are clipped to the region they were drawn
in or omitted, and partly visible text is not audited, except reading text and
links, which declare that a scroll area clips them. Wrap other scrolling
content in `observe_visible_only`.
Prefer a recipe: actions, text fields, composers, choices, switches, links and
dialogs observe themselves. Present a plain egui widget, such as a checkbox, a
progress bar, an image or a table, through
`PresentationContext::observed(ui, id, role, name, |ui| widget)`: it builds the
node from the widget's response (its rectangle, enabled, focused and, for a
toggle, checked state) and gives the widget's AccessKit node the same ID, name
and role. A plain `TextEdit` observed as `UiRole::TextInput` this way also
accepts `stobie set-text`.

Hand-build semantics only for a custom widget whose node needs more than its
response says. Call `PresentationContext::observe_node` and `observe_text`, or
the free `observe_node`, with the pane's semantic node as parent. A hand-built
text field records its egui text edit with
`PresentationContext::native(NativeTextControlKind::TextEdit, ...)` and is
observed as `UiRole::TextInput`; the host pairs it with that text edit by its
AccessKit `author_id` or its unclipped rectangle, so it accepts `set-text` while
partly scrolled out of view. Observe a selectable list row as
`UiRole::ResultRow` with `selected` and, for a done or checked state,
`checked`. Present the rows inside `list_box(ui, name, ...)`, and give each one
`set_position_semantics(&response, position, Some(total))`, so a virtualised
list still reports each row's place. Observe a recipe's own node again only to
add context such as its pane or domain; that observation replaces the recipe's.
A custom widget's AccessKit node must agree with its semantic node:
`audit_accesskit` checks roles, names, states, live settings, list containers
and bounds, and the browser layer mirrors the AccessKit side.

## Receive events from outside

A socket message, a timer, a fetch result or a worker thread delivers data
outside the host's passes. Implement `Application::started`, which the host
calls once before its first pass with an `IntentSender` for your intent type,
and give clones of the sender to your transports:

```text
fn started(&mut self, intents: IntentSender<FeedIntent>) {
    self.socket = Some(FeedSocket::open(&self.feed_url, intents));
}

// In the transport, for each message:
if intents.send(FeedIntent::Event(event)).is_err() {
    // The host has stopped; close the connection.
}
```

- `send` queues the intent and wakes the host when the queue was empty. At the
  start of its next pass, before `prepare`, the host applies every queued
  intent in the order sent, through the same `apply` as the UI's intents, so
  that pass presents them and asks for no other. An error goes to
  `report_error` and shows in the same pass. `CompletedPass::sent_intents`
  counts them.
- Nothing runs between events: an idle application stays idle until something
  is sent. Each send costs one pass, and egui follows it with one settling
  pass of its own, even when the intent changes nothing. Filter heartbeats,
  acknowledgements and replayed events in the transport.
- The sender is `Send` and `Sync` when the intent is `Send`, so a native
  thread can own it. In the browser everything runs on the page's thread:
  call it from a WebSocket callback, a timer or a `wasm_bindgen_futures`
  future.
- While the viewport is hidden (a minimised or occluded window, or a hidden
  browser page, where eframe wakes about once a second) eframe runs no pass,
  but the host still applies sent intents. It then checks every 250 ms
  (`HIDDEN_RECHECK`) until a visible pass presents them, so a page shown again
  catches up without input. A hidden application with nothing new does
  nothing.
- `Application::visibility_changed` tells the application when the page or
  window is hidden or shown again (`AppHost::visibility` reads it), before the
  intents sent since, so it can stop marking what arrives as read while
  nobody can see it and mark what is open as read once it is shown. The host
  starts out `HostVisibility::Visible`. In the browser the page's
  `visibilitychange` wakes the host; natively eframe reports a minimised or
  fully occluded window. A window behind others but not covered, or a page
  that has lost focus to another window, is still visible.
- A stream that changes state on every pass delays an agent's `set-text`
  until it pauses, because a semantic edit waits for a pass that state did
  not change under.
- `send` fails with `HostStopped`, returning the intent, once the host has
  stopped, for example after the page calls `destroy()` or the window closed.
- For timed work that needs no data, `Frame::request_repaint_after` and
  `PassContext::request_repaint_after` schedule a later pass instead.
- `AppHost::intent_sender` returns another sender, for a page hook or a test.

Which connections to open, how to reconnect and what the protocol says belong
to the application, not to Stobie. The [Live Feed](../apps/live-feed/README.md)
example has a browser WebSocket client that reconnects with a growing delay
and catches up from the last sequence number it received, and a native source
that reads standard input on a thread.

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
  By default `preferences` is `UiPreferences::system()`, which follows the
  system's light or dark appearance and, in the browser, its text size; start
  an override from it to keep following what the person has not chosen.

## Test it on the host

Host tests run the application as eframe does, in an egui_kittest harness.
Enable the UI crate's `test-support` feature in the application's
dev-dependencies:

```toml
[dev-dependencies]
stobie-ui-egui = { path = "...", features = ["test-support"] }
```

`stobie_ui_egui::testing` re-exports `egui_kittest`, so its queries, such as
`egui_kittest::kittest::Queryable`, need no dependency of their own.

`stobie_ui_egui::testing::host_harness(size, app)` builds a harness that
runs `app` on an `AppHost` in the harness's own context, so the senders that
`Application::started` receives and the host's repaint requests wake it:

```text
let mut harness = host_harness(egui::vec2(390.0, 844.0), Feed::default());
harness.state().intent_sender().send(FeedIntent::Event(event)).unwrap();
harness.run();
assert_eq!(harness.state().app().events().len(), 1);
harness.get_by_label("Clear").click();
harness.run();
```

- The harness's state dereferences to the `AppHost`, so `app()`,
  `app_mut()`, `ui_snapshot()`, `submit()` and `repaint_reasons()` are a call
  away.
- The application is laid out at exactly the harness's size, with no margin,
  as in a window or browser canvas of that size, so a breakpoint such as
  `PaneWidthClass`'s 720 points flips at that width.
- `testing::set_visibility(&mut harness, HostVisibility::Hidden)` hides the
  page as a browser would: each step then reports the change and applies
  sent intents but runs no pass, until the harness is made visible.
- `host_harness_with(builder, create)` takes egui_kittest's builder, for
  pixels per point or step length, and creates the application from the
  harness's context; an application that needs eframe's creation context
  makes one with `eframe::CreationContext::_new_kittest`.
- `web_push::PushDouble` stands in for the browser's Web Push; see
  [Web Push](web-push.md#test-it-natively).

The [Live Feed](../apps/live-feed/src/tests.rs) tests use it.

## Add undo and redo

Core's `History<C>` (`use stobie_core::{Command, History};`) records completed
changes as reversible commands. The `notes` example makes Add, Remove last and
Clear undoable; read its `NoteEdit`. Implement `Command<T>` for a small command
type, where `T` is what it changes: your model, or a struct of mutable borrows
when one change spans several owners. A command carries the values it needs in
both directions, so validate the intent first and build the command from
current state. Carry whole items with their stable IDs, so redo restores the
same item, and allocate IDs outside history so they are never reused.

```text
enum NoteEdit {
    Insert { index: usize, note: Note },
    Remove { index: usize, note: Note },
}

impl Command<Vec<Note>> for NoteEdit {
    // Ok(false) means "changed nothing": no entry is recorded. An error must
    // leave the list unchanged, so check the note is where it was first.
    fn apply(&self, notes: &mut Vec<Note>) -> Result<bool, String> { ... }
    fn revert(&self, notes: &mut Vec<Note>) -> Result<(), String> { ... }
}
```

Keep a `History<NoteEdit>` beside the state it changes and call it from
`perform` or `apply`; `History::default()` keeps 100 entries and
`History::new(capacity)` sets another bound. Route every change to that state
through history: a direct change makes the values recorded in earlier
commands, such as indices, stale.

- `execute(command, &mut self.notes)` applies one command as one entry and
  clears redo.
- `execute_group(commands, &mut self.notes)` makes several changes one undo
  step now, such as removing every note for Clear. Undo reverts them in
  reverse order and redo reapplies them in order. If one fails, the others are
  reverted and nothing is recorded.
- `execute_merging(command, ...)` extends the newest entry later instead, when
  your `Command::merge` accepts it, so a run of small steps undoes at once.
  `seal` ends the run; undo, redo and `mark_saved` also end it. Implement
  `Command::is_noop` so a run that returns to its start leaves no entry.
- `undo` and `redo` return `Ok(false)` when there is nothing to do. A command
  that returns an error must leave its target unchanged; the entry then stays.
- `last_undo` and `last_redo` show the commands the next undo or redo would
  reverse or reapply, for a status message.
- `mark_saved` records a save point and `is_saved` reports whether undo and
  redo have returned to it, for an "unsaved changes" indicator.

Make Undo and Redo unavailable when `can_undo` or `can_redo` is false, and
include `revision()` in their `meaning` so an agent's stale target is rejected.
Keep drafts and other in-progress edits out of history: one completed edit is
one entry, and undo restores committed state, not the draft that produced it.
Give Undo and Redo `Cmd+Z` and `Cmd+Shift+Z`. While they are available, the
host takes those chords even when a text field has focus, so the field's own
undo does not run; when they are unavailable, the key reaches the field. When
a command spans several owners, borrow them for the call:

```text
struct Model<'a> { notes: &'a mut Vec<Note>, workspace: &'a mut Workspace }
// impl Command<Model<'_>> for Edit { ... }
self.history.undo(&mut Model { notes: &mut self.notes, workspace: &mut self.workspace })?;
```

To make layout undoable, resolve each dock change with
`WorkspaceCommand::from_change(change, &self.workspace)` and execute the
command in a `History<WorkspaceCommand>`, or wrap it in your own command type.
Execute each resize separately: a pointer drag is one completed resize, and so
is each keyboard nudge. Record Desk's `model.rs` applies each validated draft
as one `Edit` command and keeps layout changes out of history.

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
`stobie new` generates such a page: its `web/bootstrap.js` starts the handle
through the shared startup loader, which shows loading and failure states and
records startup milestones. `stobie build-web` builds the library, runs
`wasm-bindgen` into `web/pkg/` and writes the loader. For page hooks beyond these, write
the handle around `WebHost` yourself: `with_host` reads the running host and
`with_host_mut(|host| host.submit(intent))` sends an intent through `apply`
at once. Events the application receives itself, such as WebSocket messages,
take the [sender route](#receive-events-from-outside) instead.
The component Gallery's `lib.rs` is an example. The
[browser startup guide](browser-startup.md) covers packaging.

The host prefers WebGPU and falls back to WebGL2 when WebGPU is missing, has
no adapter or fails to start. `?renderer=webgl2` or `?renderer=webgpu` in the
page URL forces one. The canvas's `data-stobie-renderer` attribute and the
`renderer` field of `hello` say which backend is running. The application
needs no code for any of this; the
[browser startup guide](browser-startup.md#renderer-backends) gives the
details.

An installable application can also receive Web Push notifications: the
page opts in with one tag, and `stobie_ui_egui::web_push::WebPush`, started
in `Application::started`, subscribes inside a tap and delivers tapped
notifications' routes as intents. [Web Push](web-push.md) describes it.

`WebHost` also mirrors the accessibility tree into a hidden ARIA layer beside
the canvas, so screen readers and accessibility-tree browser agents can read
the application. The [accessibility guide](ui-guides/accessibility.md#browser-aria-layer)
describes it. Return `false` from `Application::web_accessibility_layer` to opt
out.

## Choose dependencies

Depend on `stobie-core`, `stobie-ui-egui` and `eframe` with its `wgpu`
feature. Outside this repository, Stobie crates are git dependencies pinned
to one exact revision, as [Start a new app](start-a-new-app.md#upgrade-stobie)
describes. The UI crate's default `eframe` feature provides the native and
browser entry points but leaves GPU backends to the application, which gets
them from eframe's `wgpu` feature. In the browser that feature compiles both
wgpu's WebGPU and WebGL2 backends. Record Desk's
[`Cargo.toml`](../consumers/record-desk/Cargo.toml) is a working example.
Image applications add `stobie-imaging`, the tile runtime and renderer, and
the UI crate's `image-rendering` feature, as described in
[Compose an image application](image-application-composition.md).

An application that runs only in the browser declares eframe and the UI
crate's `eframe` feature under `[target.'cfg(target_arch = "wasm32")'.dependencies]`
and depends natively on the UI crate with `default-features = false`. Its
library then builds natively without a window system, for its tests, and
`web_handle!` remains its only entry point. `stobie new --browser-only` writes
this; see [Start a new app](start-a-new-app.md#a-browser-only-application).

## Generate and check the API documentation

```sh
cargo xtask docs
```

This generates the framework crates' rustdoc in `target/doc/`, runs the
selected examples, checks the `notes` example and the image guide's
`minimal-workspace` example natively and for WASM, and checks guide links. `cargo xtask verify` includes it. The
[documentation evidence](api-documentation-evidence.md) records exercised
environments.
