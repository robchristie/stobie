# Framework-owned application host and derived actions

Status: active
Next action: Land increment 1, the derive macro and Record Desk's action migration, then build the host and collector.

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
  `observe` (facts and collections), `prepare` (pre-pass hook for layout
  adaptation and, later, worker polling), `completed`, `active_pane` and
  `action_panes`. `ui` borrows the application immutably, so panes cannot
  mutate the model.
- `Frame` is the UI sink: `emit(intent)`, `trigger(action)`, availability-aware
  `action`/`toggle` buttons, `request_repaint(reason)` and `dock(...)`. The dock
  takes `&Workspace` and emits a `WorkspaceChange` intent.
- Recipes register observations in a per-viewport, per-pass collector stored in
  egui context data, with each node's clip rectangle. The collector is enabled
  only by a host. A new pass replaces the previous pass's observations, so
  discarded passes never accumulate.
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

## Increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 1. Derive macro and Record Desk actions | Active | Compile-fail and expansion tests; Record Desk tests and smokes unchanged |
| 2. Host, collector, Record Desk migration, composition guide and authoring probe | Planned | Framework kittest coverage, Record Desk smokes, line counts and probe report |
| 3. Lab and Gallery migrations | Planned | Lab and Gallery smokes |

## Lab and Gallery follow-up

The Lab needs pane-scoped availability and meaning (already part of the target
signature), `action_panes` for its eight panes, and a pre-pass hook with context
access for worker polling and texture upload. It also needs gesture previews
moved out of `Session` into UI state or intents before `ui` can borrow the
application immutably. Its in-pass render-plan submission can run at the end of
`ui` from UI-owned outputs. The Gallery's stories mutate fixtures inline and
need intents first.
