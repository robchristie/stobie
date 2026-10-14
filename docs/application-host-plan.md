# Framework-owned application host and derived actions

Status: active
Next action: Migrate the Lab to `AppHost` (increment 3b), following the Lab migration design below.

## Outcome and boundaries

An application declares its state, actions, UI and intent handling. Stobie owns
the per-pass plumbing: observation collection, the root node, clipping, text and
semantic audits, Tab counting, exact-modifier shortcuts, availability gating,
inspection startup, publishing and draining, and repaint decisions. The version 1
inspection protocol, capability IDs and semantic IDs stay unchanged. The host uses
no image-specific core type (`Document`, `Session`, `ImageIntent`,
`CommandHistory`, `Command`); making core generic is the next roadmap step.

## API shape

Checked against Record Desk, the Lab and the Gallery before implementation.

- `#[derive(Action)]` (crate `stobie-ui-egui-derive`, re-exported) implements
  `ActionKey` and `ActionSet::ALL` for an enum of unit variants. The enum takes
  `#[action(prefix = "…")]`. Each variant takes `label`, `description` and,
  optionally, `compact`, `shortcut` (such as `"Cmd+Shift+Z"`, parsed at compile
  time), `scope` (`application`, `pane` or `active_pane`), `id` and
  `physical_only = "reason"`. The default stable ID is `<prefix>.<kebab-case variant>`.
  Duplicate IDs or chords, data-carrying variants and malformed values are
  compile errors at the offending span.
- `Application` trait: identity, `availability(target)`,
  `ui(&self, ui, &mut UiState, &mut Frame)`, `apply(intent, &mut UiState)`,
  `perform(target, &mut UiState)` and `report_error`. Optional `meaning`,
  `observe` (facts and collections), `preferences` (the host applies the design
  system), `prepare` (pre-pass hook for layout adaptation and, later, worker
  polling), `completed`, `active_pane` and `action_panes`. `ui` borrows the
  application immutably, so panes cannot mutate the model.
- `Frame` is the UI sink: `emit(intent)`, `trigger(action)`, availability-aware
  `action`/`toggle`/`icon_action` buttons, `presentation(...)` and `tokens()`,
  `request_repaint(reason)` and `dock(...)`. The dock takes `&Workspace` and
  emits a core `WorkspaceChange` intent.
- Recipes register observations in a per-viewport, per-pass collector stored in
  egui context data. The collector is enabled only by a host. A new pass
  replaces the previous pass's observations, so discarded passes never
  accumulate. Observations made inside `observe_visible_only`, which wraps each
  dock pane, carry their clip rectangle and are published only where visible.
- Entry points: `AppHost<A>` (egui), `run_native` and a `web_handle!` macro
  over `WebHost<A>`, behind the default `eframe` feature.

## Decisions

- **Agent-invocable by default.** Every action is semantically invocable unless
  its declaration carries `physical_only = "reason"`. Value-taking actions are
  physical-only because arguments are not yet supported. Record Desk's Save,
  Restore and Reviewed toggle become invocable; its category and review choices
  stay physical-only.
- **One action path.** Clicks, shortcuts and semantic invocations reach
  `perform` through one host route that rechecks availability immediately before
  running. A pass applies data intents first, then actions in trigger order.
  Physical-route errors reach `report_error`. A rejected semantic invocation
  is reported only in its receipt, so a failed invocation leaves observed facts
  unchanged.
- **Meaning defaults to `stable`.** Applications override `meaning` for actions
  whose effect depends on observed state, such as a draft, a selection or history.
  A global state counter would make unrelated changes stale.
- **`completed` sees presented state.** The hook runs after publication and
  before this pass's intents apply. Record Desk's smoke snapshot therefore
  always pairs state with the geometry that presented it. This removes a race
  in which a smoke clicked stale coordinates after Reset filters, which failed
  increment 1's first post-merge native run. Every state change repaints, so the
  next pass observes it.
- **Publish as presented; filter only scrolling regions.** Clipping every
  observation to its region hid real defects from the audits, such as text
  outside its clip or allocation and nodes outside the root. Across 540 Gallery
  renders it changed the published text in 66 and the nodes in 47 (found in
  review of PR #65). The host therefore filters only inside
  `observe_visible_only` regions, where scrolled-out rows are expected. Dock
  panes are such regions. Record Desk wraps its whole UI, which keeps its
  pre-host behaviour: its top bar is clipped on the first frame until the panel
  has measured its content.

## Increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 1. Derive macro and Record Desk actions | Landed | [PR #63](https://github.com/robchristie/stobie/pull/63): compile-fail and expansion tests; Record Desk tests and smokes unchanged |
| 2. Host, collector, Record Desk migration, composition guide and authoring probe | Landed | [PR #64](https://github.com/robchristie/stobie/pull/64): host kittests; Record Desk tests and smokes; glue 962 to 526 lines, accepted by the user; probe below |
| 3a. Gallery migration | Landed | [PR #65](https://github.com/robchristie/stobie/pull/65): Gallery tests, deterministic UI snapshots and Gallery-driven smokes; published snapshots equal base's in 540 renders apart from the root name |
| 3b. Lab migration | Active | Lab tests, native and browser smokes, and the application-interface smoke |

## Authoring probe

A fresh-context agent given only the public guide, its links and rustdoc built
a single-pane task list with four actions in three builds and no compile
errors. Natively, the CLI observed clean audits, discovered every action with
correct availability, completed enabled invocations, waited on facts and
rejected a disabled action. The app contains no inspection code.

These stumbles were fixed in increment 2:
- The crate rustdoc started from the low-level dock. It now starts from the host.
- There was no docked example or small-layout constructor. `notes` now uses a
  single-pane dock, and `Workspace::with_tabs` builds small layouts; workspace
  fields are documented.
- The pane content parent was undocumented, and buttons lacked their pane.
  `Frame::pane_presentation` now sets both.
- Status lines could not be waited on. `PresentationContext::status` now
  publishes a status node.
- Integer facts needed casts. `usize`, `u32`, `i64` and `i32` now convert.
- The guide and rustdoc were silent or wrong on several points, now covered:
  - key spellings and the text-focus rule for shortcuts without Cmd;
  - recording native text fields and roles for list rows;
  - the browser lib/bin layout and the `web_handle!` example;
  - socket path length and stale sockets;
  - the CLI's `--timeout`.
- Automation startup failures appeared only in the app's status line. They now
  also go to stderr (found while running the example before the probe).

Recorded for later work:
- a generic selectable or checkable list-row recipe and role;
- null or optional facts;
- removing a stale socket after a signal;
- CLI diagnostics: the reported deadline, unknown condition fields, error
  output and a way to pass an expected target;
- marking shortcuts that text focus blocks in discovery;
- action arguments, without which agents cannot fill a draft.

## Gallery migration

- The Gallery's model is the selected story, its configuration and the
  effective theme. Story fixtures, the dock scene, keyboard-focus state and the
  appearance workbench are UI state, because they exist to demonstrate
  components. The chrome emits `Select`, `Configure` and `Theme` intents.
- Stories keep their low-level recipe vectors, which the dispatcher registers
  against the story area. Presentation contexts and choice and range recipes
  register themselves, so stories no longer merge them. Stories report
  selectable text and fixture repaints through a `StoryReport`.
- `GalleryAction` stays a demonstration-only key, now derived with its IDs
  unchanged. The hosted action set is `GalleryCommand::OpenWorkbench`.
- The host gained:
  - `Application::theme` for custom themes, which the Gallery's workbench uses;
  - UI state in `completed`, for the Gallery's fixture snapshot;
  - `AppHost::submit` and `request_repaint`, plus `WebHost::with_host_mut`, for
    page hooks;
  - a root node fixed to the viewport, even when content overflows.
- Equivalence: 30 stories, 9 configurations and 2 window sizes, each after 4
  passes, publish the same nodes, text, audits and coverage as base, apart from
  the root's name (now the application name) and publication order. The
  Gallery's own unit test renders each story alone at its configured width, as
  before.

## Lab migration

The Lab keeps its image model and runtime; only its frame lifecycle moves to the
host. No host type names an image concept.

- **Gesture previews stay in `Session`.** `AGENTS.md` places gesture previews in
  the session and requires them to render in the frame that changes them. An
  earlier note proposed moving them to UI state, which would break that rule.
  Instead, `ui` edits a pass-local copy of `session.gesture`, renders the
  overlay and render plan from that copy, and emits a `Gesture` intent first.
  A completed gesture still emits one command and one undo record.
- **One action path.** Toolbar clicks trigger actions, so clicks, shortcuts
  and semantic invocations all reach `perform` and are rechecked for
  availability. Availability and meaning use the context the Lab's inspection
  bindings already build. `action_panes` keeps every pane-scoped action
  discoverable for panes 1 to 8, as before.
- **Invocable by default.** As decided above, every action becomes semantically
  invocable except menu anchors, which open UI rather than change state
  (`ViewPanels`, `AppearanceSettings`, `DisplaySettings`), and
  `CopyDiagnostics`, which writes the system clipboard. Each declares its reason.
- **Layout.** The host dock emits `WorkspaceChange`. A resize stays an undoable
  `ResizeSplit` command; activation and moves apply directly, as before.
- **UI state.** Camera gestures, the thumbnail cache (egui textures), virtualisation
  metrics and the Lab's physical-test geometry are UI state. `UiGeometry` keeps
  only physical-test geometry; semantic nodes and text go to the host collector.
- **Host additions.**
  - `prepare` receives UI state, so runtime polling can insert thumbnail
    textures.
  - Dock pane, tab and splitter nodes carry pane and dock-node domain
    references, as the Lab's always did.
  - Applications can persist to eframe storage and ask for an immediate save.
  - `Frame::request_repaint_after` schedules a delayed repaint for a recorded
    reason, for the Lab's wheel-gesture timeout.
- **Equivalence.** Lab tests, native and browser smokes and the
  application-interface smoke pass. Expected snapshot differences are the
  root's name and publication order; any other difference is explained in the
  pull request.
