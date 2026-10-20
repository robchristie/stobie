# Actions on rows

Status: active
Next action: Run full verification and the design critique, then open the pull request.

An application lists rows, such as people, accounts or invitations, and
offers the same action on each: Open on each person, Withdraw on each
invitation, Edit on each item. Stobie's actions could carry a typed argument
through `Frame::trigger_with`, but its recipes (`action`, `icon_action`,
`toggle`) always triggered without one, a control's semantic node carried no
argument, and a binding took any value. An application therefore built each
row's button by hand, triggered the action with the row's ID from its click,
observed the button as a `Button` node itself, and left agents to invoke the
action with an ID they had to find elsewhere; every row's button had the
same accessible name unless the application composed one.

## Decisions

- **A `BoundAction` carries the row's argument.** `BoundAction::new(action,
  argument)` (or `for_target` with an explicit pane target) holds the action,
  the argument its control carries and an optional subject, the row's name.
  It implements `IntoActionTarget`, whose new provided method `into_bound`
  keeps the argument, so every hosted `Frame` method that takes a target
  takes it: `action`, `icon_action`, `toggle`, the new `list_row`, `trigger`
  and `availability`. A click, Enter, Space or an AccessKit click triggers
  the action with the argument through the existing validated route to
  `Application::perform_with`. Existing calls with an action or an
  `ActionTarget` are unchanged.
- **Identity and names.** A control's identity combines its key with the
  argument's JSON value, so each row's control has its own stable semantic
  ID wherever the row moves; an argument-less control keeps its identity. A
  button's accessible name, AccessKit label and tooltip are the action's
  label followed by the subject ("Open Ada Lovelace"); the visible label stays
  the action's. A list row is named by its title.
- **A list-row trigger.** `Frame::list_row` (with
  `PresentationContext::list_row`, the low-level `list_row`,
  `list_row_semantic_node` and `list_row_height`) presents one full-width
  row whose surface performs its bound action: a `ListRowSpec` title in
  `ButtonLabel` and optional detail in `Secondary`, each one elided line,
  with quiet hover, pressed and focus treatments matching navigation items.
  It is a Button named by its title, described by its detail and the
  action's description. It has no current or selected state: destinations
  remain navigation items, and conversation rows (a later chat recipe) can
  build on it.
- **Semantics.** `UiNode.argument` publishes the JSON value an invocation
  would send. The host binds each control carrying an argument as its own
  capability, with `CapabilityDescription.argument` and
  `ResolvedTarget.argument` set; the action keeps its default binding, which
  takes any value its contract allows, unless a control without an argument
  presents it. All three fields, and `InspectionSelector.argument`, are
  omitted from the wire when absent, so existing clients and observations
  are unchanged.
- **Invocation by argument.** A selector's `argument` matches controls
  carrying an equal value, comparing numbers by value. A row's binding takes
  no value, meaning its own, or that same value; any other is
  `invalid_arguments`. A sent value also chooses among the bindings a
  selector matches (`select_invocation_target`, shared by the host and the
  Rust client): bindings carrying another value cannot take it, and the one
  carrying it is preferred over the action's own binding. `stobie invoke
  CAPABILITY --value V` therefore reaches the row carrying `V` when it is
  presented and the action's binding otherwise, and the new `--argument`
  selector flag names a row's control without a value. `observe` shows
  `carrying V` on a node and `discover` shows `CAPABILITY = V`.
- **Availability for an argument.** `Application::availability_with(target,
  argument)`, defaulting to `availability(target)`, decides availability for
  one row, such as Remove disabled on a pinned person. Controls and bindings
  carrying an argument use it, and the host checks it again before
  performing any action with an argument. A row argument hidden by it
  presents no control.
- **Stale and removed rows.** A removed row's discovered target no longer
  matches (`no_match`). One admitted before the row went is refused when it
  runs: `unavailable` when `availability_with` hides the argument, otherwise
  `stale_target`. A target whose control now carries another argument, or
  none, is `stale_target`. A click on a row that an earlier intent in the
  same pass removed is refused by `availability_with`. Invoking the action's
  own binding with a removed row's ID is refused the same way, before
  `perform_with`.
- **Compatibility.** Additive for applications: the argument-less `Frame`
  methods, `ActionTarget`, `ActionButtonSpec`, `ActionSpec`, `action_button`
  and `ActionKey`, which emuella-viewer uses, are unchanged. Struct literals
  of `UiNode`, `CapabilityDescription`, `ResolvedTarget`,
  `InspectionBinding` and `InspectionSelector` need the new field (or
  `..Default::default()` for a selector); no known consumer outside Stobie
  builds them, and Stobie's own applications are updated.
  `TextComponentKind` gains `ListRow`, `ListRowTitle` and `ListRowDetail`.
  No dependency is added.

## Design review contract

- **Task.** Scan a short list of people, open one, pin one and remove
  another, by pointer, keyboard, screen reader and agent alike.
- **Hierarchy.** Each row's title leads, its detail is secondary, and its
  actions follow as ordinary buttons beneath it; the whole row selects the
  person. Actions stay quieter than the row's name: Open and Pinned at
  normal emphasis, Remove as a quiet icon.
- **Reference.** Navigation items' row geometry and treatments, and the
  existing action button and toggle recipes.
- **States.** Normal and narrow (360 px) widths; hover, pressed and keyboard
  focus; a toggle pressed; a disabled row action with its reason; a removed
  row; an empty list is the application's.
- **Exclusions.** Selected or current rows, multi-line details, swipe
  actions, row menus and virtualised lists.

## Acceptance

- Host tests (`stobie-ui-egui`, `host::row_tests`): each row's Open, Remove,
  Pinned and list row dispatches its own argument by pointer and keyboard;
  each control publishes its argument under a distinct name and stable ID,
  with AccessKit parity; availability for an argument disables one row's
  control and hides another's; agents discover one binding per row plus the
  action's own and invoke a row by value, by selector argument and as text;
  a removed row's control is refused, as is a click on a row removed earlier
  in its pass.
- Inspection tests: a row's binding runs with the argument its control
  carries; a control carrying another argument, or none, is stale; a row
  removed before its invocation runs is stale; targets without an argument
  keep their wire form.
- Protocol, client and CLI tests: wire values and numeric comparison,
  selector matching, a row's binding taking no value or its own, the
  selection rule, the client's choice by value, and the CLI's `--argument`,
  `= V` and `carrying V` output.
- Outside application (`tools/row-actions-app`, run by
  `tools/row-actions-smoke.mjs` in the outside-app stage): natively and in
  Chromium, `stobie` discovers each row's control with its ID, invokes rows
  by value and by `--argument`, sees a pinned person's Remove disabled, and
  has a removed row's target and ID refused. In Chromium, the ARIA layer
  names every row's control after its person, each name once, with the
  inspection interface's ID, and a plain Playwright click on each name acts
  on that person.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Protocol, recipes, host bindings and clients | Active | Focused tests |
| Outside application, browser and agent checks | Active | Outside-app stage |
| Documentation, critique and full verification | Active | |
