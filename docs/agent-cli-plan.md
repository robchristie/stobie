# Agent CLI, action arguments and browser-agent operability

Status: active
Next action: Increment 4, the client library and the `stobie` CLI.

## Outcome and boundaries

Any agent with a shell can launch or attach to a Stobie application, natively
or in a browser, read its UI compactly and operate it, including entering
text, through one Rust `stobie` binary. A general browser agent can use plain
Playwright `click()` and `fill()`, or Lantern role/name targets, on the hidden
ARIA layer. The inspection snapshot and the ARIA layer are checked to agree.

- The version 1 protocol changes only additively. Existing capability IDs,
  semantic IDs, smokes and UI snapshot baselines stay unchanged.
- `?automation=1` and `STOBIE_AUTOMATION_SOCKET` remain the only automation
  opt-ins. Neither set text nor the click change lets a page script do
  anything a user cannot already do through the UI.
- One completed gesture or edit is one command, IDs stay stable and nothing
  repaints unconditionally. Typed arguments pass the same availability checks
  and validated intents as clicks.
- No MCP server, no eframe or egui patch and no change to the Lantern
  repository. The client library and the CLI's command layer are separate
  crates, so an MCP mode can be added later.

## Decisions

### Typed action arguments

- `ArgumentContract` gains `Text { max_length }`, `Boolean`,
  `Number { minimum, maximum, integer }` and `Choice { options }`, where each
  option has a stable `id` and a `label`. `None` still serialises as `"none"`;
  the new contracts serialise as objects tagged by `kind`.
- An action takes at most one argument. The wire form is
  `"arguments": {"value": …}`. A no-argument action still accepts only a
  missing, `null` or empty `arguments`, unchanged.
- Each contract validates JSON in Rust: type, length in characters, range,
  integrality and option membership. Admission rejects an invalid value with
  `invalid_arguments` and a message naming what was expected. Execution
  revalidates against the current contract.
- `#[derive(Action)]` takes `argument = "text" | "boolean" | "number" |
  "choice"` with `max_length`, `minimum`, `maximum`, `integer` or
  `options = Type`. `Type` implements a small `ChoiceArgument` trait (also
  derivable for unit enums) that lists option IDs and labels and parses an ID.
  `ActionKey::argument()` publishes the contract through `discover`.
- `Application::perform_with(target, argument, state)` receives a typed
  `ActionArgument`. Its default runs `perform` for no argument and rejects
  anything else, so existing applications are unchanged. `ActionArgument`
  offers typed accessors, such as `choice::<Category>()`.
- Record Desk's category edit and its category and review filters, today
  physical-only because they choose a value, become choice actions. Their
  semantic route applies the same intent as the combo box. The Lab and the
  Gallery have no naturally parameterised action, so they gain none. Tests in
  `stobie-ui-egui` cover every kind.

### Framework set text

- **Route.** Set text is an AccessKit `SetValue` request on a text field's
  node, handled by the host. At the start of a pass, the host focuses the
  field, selects all of its text and inserts the new value as egui paste
  input; an empty value deletes the selection. egui's `TextEdit` applies it
  exactly as typed or pasted input, so the application sees its ordinary
  change, draft, validation and intents. A single-line field turns newlines
  into spaces, as a paste does. A value equal to the current text changes
  nothing. One field is edited per pass; consecutive edits of one field
  coalesce to the latest, since each carries the whole text. A semantic edit
  returns focus to where it was, so no caret is left blinking and the
  application can idle; assistive technology editing in place keeps focus.
- **Which fields.** A framework text field is an egui `TextEdit` presented
  through `PresentationContext::native` with `NativeTextControlKind::TextEdit`
  and observed as a `TextInput` or `MultilineTextInput` node whose AccessKit
  `author_id` is the node's semantic ID, as the parity audit already
  requires. No application code is needed. Native hosts enable AccessKit
  generation while automation is on, so the host can pair fields and read
  their values.
- **Protocol.** Each rendered framework text field is bound to the
  framework capability `stobie.set-text`, whose identity is the field's
  semantic ID and whose contract is `Text`. Invoking it goes through the
  ordinary invocation ledger, receipts, expected-target and availability
  checks. A disabled field rejects with its reason. `UiNode` gains an
  optional `value`, published for text fields and omitted otherwise, so
  `observe`, `wait` and the agreement check can read the text. `UiNode`
  struct literals in the Lab, the Gallery and the framework gain
  `value: None`; nothing else in an application changes.
- **Observing the effect.** A completed receipt gains `after_observation`,
  the first observation that includes the invocation's effect. A client
  waits for it before discovering a target whose meaning the action changed;
  without it, the interface smoke discovered Apply from the observation
  before a category choice and was correctly rejected as stale. Set text
  completes later: an executed invocation may return `Execution::Deferred`,
  its receipt stays queued, and the host calls `Inspection::complete` after the
  pass that applied the edit has applied its intents. That pass's staged
  publication lacks them, so `after_observation` names the one after it.
- **Queued edits.** In-place edits from the ARIA layer or assistive
  technology go before queued semantic edits. A semantic edit applies only
  while its widget still presents the semantic ID it was queued for, and
  waits a pass when an action changed state earlier in the same pass, so a
  widget an application reuses for another selection is never written by
  mistake. Password fields are never paired.
- **ARIA layer.** A layer text field that receives DOM focus keeps it, so
  `locator.fill()` and Lantern `type` can edit it. An `input` event on it
  becomes a `SetValue` request with the field's whole value. If egui does
  not take the value, the next pass restores the mirrored text. Tab and
  Escape in a layer field return DOM focus to the canvas and reach egui as
  the same keys; Cmd or Ctrl chords other than editing keys reach egui as
  application shortcuts. Other layer elements still return focus to the
  canvas. A screen reader user's caret and IME composition stay in the
  browser field; each composed change reaches egui as a whole-text edit.
- **Native assistive technology.** Framework text fields advertise
  `SetValue`, and an AccessKit `SetValue` request from the platform adapter
  takes the same route, where the adapter delivers one.

### Plain browser click

Playwright's `click()` and Lantern's `--strict` targets hit-test the centre
of the element's box with `elementFromPoint`. Layer elements pass the pointer
through to the canvas, so they always fail that test.

- Each enabled layer element whose node supports AccessKit `Click`, except
  text fields (`input` and `textarea` cannot hold children), gets a 1 CSS px
  child at its centre with `pointer-events: auto`. Only that pixel becomes a
  hit target.
- The layer re-dispatches every pointer, mouse, touch, wheel and context-menu
  event that lands on such a pixel to the canvas, at the same coordinates,
  and preserves `preventDefault`. It stops the canvas's `mouseleave` and
  `pointerleave` that moving onto the pixel would cause. egui therefore sees
  the same input stream: hover, drags, wheel and touch starting there behave
  as before. A `click` event targeting the pixel is not turned into an
  AccessKit click, so a physical click activates the control once.
- A plain `click()` is therefore the same physical click at the control's
  centre as `click({force: true})`. `dispatchEvent('click')` and
  `element.click()` still use the AccessKit route.
- eframe accepts a pointer move only where `elementFromPoint` finds the
  canvas, so the layer turns hit targets off, through a stylesheet rule that
  writes nothing to the DOM, while it dispatches each copy. Without that, the
  icon-actions smoke showed hover lost at a control's exact centre.
- The known difference for real input is the pointer's cursor shape over
  that single pixel, which shows the default cursor rather than egui's.
- A disabled control has no hit pixel, so Playwright reports it disabled.
- egui reports a widget's whole rectangle even where a list or scroll area
  clips it. Review found that an over-rendered row's pixel then sat over
  another control, so a plain click acted on the wrong one. The projection
  now carries the clip of scroll areas, lists, panes and regions, and marks a
  control only when its centre is visible. It also withholds the pixel where
  the visible part of a later control, which egui would put on top, covers
  the centre; dock splitters register before their panes, so their grab areas
  lie beneath, and rows a list has scrolled away contest nothing.
  The browser check sweeps every control for these rules, including after
  scrolling the Lab's thumbnails.
- File drag events are forwarded too. Safari gesture events are not, a touch
  that outlives its control's element stops at the pixel, and egui popups
  drawn over a control do not remove its pixel; these are documented
  limits.

### Semantic agreement and stable IDs

- A shared projection describes each identified node by role (mapped to
  ARIA), name, enabled, focused, selected, checked, expanded and value. One
  function projects the snapshot, another the ARIA tree projected from
  AccessKit, and a comparison reports disagreements by semantic ID.
- Rust tests run the comparison for the Lab, the Gallery and Record Desk in
  representative states, so CI fails on a real disagreement. Each one found is
  fixed or justified in this plan.
- Layer elements carry `data-stobie-id` with the semantic ID, which browser
  agents and CLI users share.

### Protocol crate

- `stobie-protocol` holds the inspection wire types, limits, argument
  contracts and their validation, and the semantic types they carry: snapshot,
  nodes, roles, rectangles, IDs, domain references, text observations and
  audit findings, and action metadata (scope, shortcut, availability). It
  depends on `stobie-core` for pane and dock IDs and on serde, never on egui.
- `stobie-ui-egui` re-exports everything applications use and keeps the
  `Inspection` service, the bindings it serves and the egui conversions. This
  increment changes no behaviour.
- Rust cannot add inherent methods to another crate's types, so two egui
  methods became extension traits re-exported by `stobie-ui-egui`:
  `TextRoleStyles::style` and `ShortcutEgui::egui`. The two Gallery stories and
  the Record Desk test that call them import the trait; nothing else in an
  application changes. `UiRect` and `TextRect` convert from egui's `Rect`
  through the protocol's optional `emath` feature, which `stobie-ui-egui`
  enables. `ShortcutKey` names its key (`0`, `PageUp`, `Comma`), and a test
  checks every name parses as a distinct egui key.
- `cargo xtask architecture` checks that the protocol resolves no egui,
  eframe, wgpu, web-sys, winit or `stobie-ui-egui`. It resolves serde,
  serde_json and `stobie-core`.

### Client library and CLI

- `stobie-client` (library) holds the transports, `hello`, observe, query,
  discover, invoke with receipts and cancel, set text, wait conditions and
  capture. Native capture uses the existing X11 route (`xdotool` and
  ImageMagick `import` on the host window), so the protocol needs no image
  operation; browser capture uses CDP `Page.captureScreenshot`.
- The browser transport uses `lantern-core`'s blocking CDP client to call
  only the fixed `window.__STOBIE_AUTOMATION.request` dispatcher with
  `Runtime.evaluate`.
- `stobie-cli` builds the `stobie` binary from the library. Neither crate is
  in any application or WASM dependency graph; `cargo xtask architecture`
  enforces that.
- **Sessions.** `stobie launch NAME (--manifest-path PATH | --package PKG |
  -- COMMAND…)` builds with Cargo when needed and starts a native application with a fresh private socket directory, waits for
  `hello`, records the session and returns. A detached supervisor removes the
  directory when the application exits, and `stobie stop NAME` ends it.
  `stobie attach NAME --cdp URL [--page URL | --target ID]` records a
  browser page and accepts the endpoint `lantern browser endpoint` prints.
  Commands take `--session NAME`, `--socket PATH` or `--cdp URL --page …`.
- **Commands.** `observe`, `query`, `discover`, `invoke` (typed `--value`,
  waits for the receipt), `set-text`, `wait`, `capture`, `receipt`, `cancel`,
  `sessions` and `capabilities --json`, which describes commands, exit codes
  and output schemas without connecting.
- **Output.** Text by default: a compact outline of role, name, state,
  value, stable ID and available capabilities, plus facts. `--json` emits one
  object with `schema_version`, `command` and `ok` first, fixed field order,
  `limit` and `cursor` pagination, explicit `truncated` markers and an
  `omitted` list of anything left out. Budget: Record Desk's default
  `observe` stays under 8 KB.
- **Exit codes.** 0 success; 1 the application rejected or did not match;
  2 usage; 3 connection or attach failure; 4 deadline or uncertain
  completion; 5 partial evidence. Errors go to stderr with a code, a message
  and the next step.
- **Robustness.** The native host removes a stale socket only when nothing
  answers on it. Diagnostics name the deadline, reject unknown condition
  fields and report target mismatches. Optional facts may be `null`.
- **Node client.** The Rust CLI becomes the documented agent CLI. The Node
  client stays for the Playwright harness, and a parity test runs the same
  scenarios through both. `tools/application-cli.mjs` is retired, since only
  documentation refers to it.

## Increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 0. README native Orca claim | Landed | [PR #77](https://github.com/robchristie/stobie/pull/77) |
| 1. Typed action arguments and set text, including `fill()` | Landed | [PR #78](https://github.com/robchristie/stobie/pull/78): protocol tests for each argument kind and the set-text binding; derive expansion and compile-fail tests; host tests showing set text gives the same state and history as typing, AccessKit `SetValue`, coalescing and focus return; Record Desk choice and notes tests; the Record Desk interface smoke sets notes and a category on both hosts; the ARIA layer check fills a single-line and a multiline field. Sizes and startup below. |
| 2. Plain browser click and the semantic agreement check | Landed | [PR #79](https://github.com/robchristie/stobie/pull/79): projection tests for hit markers and semantic IDs; agreement unit tests for each field; agreement over every Gallery story, four Record Desk states and three Lab states, with no disagreement found; the ARIA check sweeps every visible enabled control for a centre hit, moves a real pointer over a hit target and plain-clicks controls in all three applications; the Lab, Gallery, icon, navigation and status-chip pointer, drag, wheel and keyboard smokes pass unchanged. Real Lantern `--strict` role/name clicks are recorded below. |
| 3. Protocol crate extraction | Complete | This PR: `stobie-protocol` holds the wire, argument, semantic, action and text types; the full test suite, UI snapshot baselines and smokes pass unchanged; the architecture check covers the protocol's dependency graph. |
| 4. Client library, `stobie` CLI, parity tests, documentation and probes | Planned | |

Each increment lands through review in order, rebased on the previous merge.

## Size and startup

Production builds, Brotli quality 11, against `b3abff7`. Startup is one
local pair from `cargo xtask verify`, so it shows only that the gates pass.

| Increment | Lab WASM (Brotli) | Gallery WASM (Brotli) | Lab cold / repeat content | Gallery cold / repeat content |
| --- | --- | --- | --- | --- |
| Baseline `b3abff7` | 2,960,391 B | 2,834,735 B | 171.1 / 125.4 ms | 94.2 / 66.0 ms |
| 1. Arguments and set text (first candidate) | 2,968,398 B (+0.27%) | 2,842,978 B (+0.29%) | 179.1 / 120.8 ms | 86.0 / 54.6 ms |
| 2. Plain click and agreement, on 1 as landed | 2,971,329 B (+0.37%) | 2,846,395 B (+0.41%) | 171.2 / 126.7 ms | 98.6 / 66.4 ms |
| 3. Protocol crate, on 2 as landed | 2,974,065 B (+0.46%) | 2,848,476 B (+0.49%) | 168.5 / 113.7 ms | 100.6 / 53.8 ms |

## Lantern strict targets

`lantern click --role R --name N --strict` (Lantern `c160146`) against each
application's ARIA layer in Chromium, with the outcome read from the
accessibility tree:

| Application | Targets | Outcome |
| --- | --- | --- |
| Record Desk | option "Prepare review agenda · Operations · Reviewed", button "Reviewed", "Apply", "Undo" | Row selected, Reviewed unpressed, Apply disabled after applying, Redo enabled after Undo |
| Gallery | tab "Overview", button "button/default" | Tab selected, toggle pressed |
| Lab | tabs "Thumbnails" and "Results", result row "#0000002…" | Tabs selected in turn, row selected |

Every click was acknowledged. The Lab's "Polygon" is one button per image
pane, so Lantern correctly refuses it as ambiguous.

## CLI size and output

The `stobie` release binary is 2,991,544 B (2,430,480 B stripped), and a clean
release build of `stobie-cli` and its dependencies takes 15.3 s on a 32-core
host. No application or WASM build includes it.

Output per command, in bytes, from the CLI smoke against its budget:

| Command | Record Desk | Lab |
| --- | --- | --- |
| `observe` | 3,721 / 8,192 (browser 3,733) | 8,726 / 16,384 |
| `observe --json` | 11,536 / 24,576 | 23,102 / 49,152 |
| `discover` | 1,839 / 6,144 | 6,528 / 12,288 |
| `discover --json` | 10,376 / 49,152 | 35,347 / 98,304 |
| `query --role result_row` / `query --role tab` | 1,121 / 4,096 | 390 / 2,048 |
| `hello` | 126 / 512 | 139 / 512 |

## Semantic agreement findings

The comparison found no disagreement in the tested states. Every Gallery
story, Record Desk (initial, focused search, filtered with a draft, narrow with
another record) and the Lab (initial, Tab focus, a selected result without
Diagnostics) agree on role, name, enabled, focused, selection, checked,
expanded and text values. Roles the layer flattens or presents as text
(application, toolbars, scroll areas, sections, status chips) are not compared.

## Acceptance

- `discover` publishes argument contracts. Valid typed arguments reach the
  application, invalid ones are rejected clearly and no-argument invocation is
  unchanged. Each argument kind has tests.
- Set text works for a framework text field through the CLI (native and
  browser), the protocol and `locator.fill()`, and gives the same application
  state and history as typing.
- A plain Playwright `click()` and Lantern role/name targets under `--strict`
  activate enabled controls in all three applications. The existing pointer,
  drag, wheel and keyboard smokes pass unchanged.
- The agreement check runs in CI for all three applications and passes.
- The Rust and Node clients pass the same parity scenarios.
- Fresh-agent probes, native, browser and Lantern-only, complete the Record
  Desk task (find a record, set its notes, choose its category, Apply, Save,
  Undo). Their stumbles are recorded and fixed.
- CLI output sizes per command on Record Desk and the Lab are reported against
  the budgets, with WASM size, startup, CLI binary size and build time before
  and after.
- `cargo xtask verify` passes, including the production stage.

## Lantern licence

Lantern carried no licence text, only `license = "MIT"` in its Cargo metadata.
Its owner chose Apache-2.0 to match Stobie, and
[robchristie/lantern#25](https://github.com/robchristie/lantern/pull/25)
added the licence text and metadata. `stobie-client` depends on `lantern-core`
as a Git dependency pinned to that merge, `2debc7e`, behind its default
`browser` feature.
