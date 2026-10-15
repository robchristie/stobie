# Inspect and exercise a Stobie application

The shared application interface lets a client read current semantics and text,
discover typed capabilities, dispatch a validated action, wait for an observed
result and exercise the same host through physical input. Record Desk, the
component Gallery and Analytical Workspace Lab expose the same version 1 JSON contract on an explicitly
enabled local Unix socket or browser page. Rust types in
[`stobie-protocol`](../crates/stobie-protocol/src/inspection.rs) are
authoritative, and `stobie-ui-egui` re-exports them; the JavaScript client
does not maintain a separate action registry or schema.

The [execution plan](application-interface-plan.md) and
[qualification evidence](application-interface-evidence.md) own the current
delivery state and exercised environments. Host examples below describe the
shared client; application startup, persistence and prerequisites remain in the
[Record Desk README](../consumers/record-desk/README.md) and
[browser startup guide](browser-startup.md).

## Enable a host

The [`stobie` command](agent-cli.md) is the agent CLI: it launches or attaches
to an application and reads and operates it. The JavaScript client in
`tools/application-client.mjs` serves the Playwright test harness; install its
tooling with `npm ci`. Native physical input and capture require X11,
`xdotool` and ImageMagick's `import`; they target an explicit host window,
without shell evaluation. Unix socket observation does not require X11.

Native applications enable automation only when `STOBIE_AUTOMATION_SOCKET`
supplies an absolute path. `stobie launch` does this with a fresh private
directory; by hand, launch the host with that environment:

```sh
stobie launch desk --manifest-path consumers/record-desk/Cargo.toml
# or
AUTOMATION_DIR=$(mktemp -d)
export STOBIE_AUTOMATION_SOCKET="$AUTOMATION_DIR/record-desk.sock"
cargo run --release --manifest-path consumers/record-desk/Cargo.toml
```

The host retains a private socket and removes its own socket on ordinary
shutdown. A process killed by a signal leaves its socket behind; the next start
removes a socket that nothing answers on, and refuses to replace a live socket
or any other file. Keep the path short: Unix socket paths are limited to about
100 bytes. Restart creates a new instance identity. Non-Unix native hosting is
unavailable. Starting without the variable disables automation; closing the
enabled host ends that instance.

The native window must be mapped and unobscured for eframe to process the
UI-thread invocation queue. A fully covered or minimised window can retain a
responsive read-only socket while an admitted action stays `queued`. Make the
host window visible (for X11, `xdotool windowraise X11_ID`), then look up the
original receipt and wait for the outcome. Do not submit the mutation again.
The client deadline reports uncertain completion while work remains queued.

Browser applications enable the fixed dispatcher only when their startup URL
contains `?automation=1`. Open the built app with that query string, then attach
`stobie` or Playwright to its Chromium DevTools endpoint. Removing the parameter and reloading
starts a host without the dispatcher. The public `BrowserTransport` encapsulates
`window.__STOBIE_AUTOMATION.request`; callers do not evaluate application
globals or parse application-specific test snapshots.
The maintained bootstraps pass `crypto.randomUUID()` to
`handle.enable_automation(instance)` so each browser startup has a distinct host
nonce. Clients obtain it through `hello`; no nonce argument is needed in the CLI.

```sh
stobie attach web --cdp http://127.0.0.1:9222 --page 'http://127.0.0.1:8080/?automation=1'
stobie hello --session web
```

`--page` must select exactly one existing page. The JavaScript client's native
physical adapter needs the actual host window's X11 ID. Its browser adapter
needs exactly one canvas. The native configured window must cover the
application's client viewport: the client rejects mismatched root dimensions or
scale.

## Read, discover and invoke

Each request has `version`, a unique `request_id`, the bound host `instance` and
an `operation`. `hello` returns application/build identity, supported operation
names and limits and binds
the client. Other calls require the same instance. The CLI performs `hello`
before its selected command; a JavaScript caller performs it once explicitly.

Selectors use exact AND matching across any supplied `id`, `role`, `name`,
`capability`, `pane` and `domain`. Roles use the Rust `UiRole` snake-case values.
Domains use the existing tagged `DomainReference`, for example
`{"kind":"pane","value":1}`. An empty selector supports bounded inspection;
invocation and physical targeting require one unambiguous result. Use a stable
identity, pane or domain to resolve ambiguity.

Discovery describes supported capabilities even when their individual controls
are absent from the current UI pass. `rendered` and `control_enabled` distinguish
that observation from application-owned `availability` and
`semantic_invocable`. For example, Lab Diagnostics is a supported semantic
action when its menu is closed; querying its control can return zero nodes.
Hidden availability still rejects invocation. Physical targeting always
requires a currently rendered, enabled control.

```sh
stobie observe
stobie query --role text_input
stobie discover record-desk.apply
stobie invoke record-desk.edit-category --value ideas
stobie set-text record-desk.notes.1001 'Checked against the survey'
stobie invoke record-desk.apply
```

`observe` returns the latest completed, non-discarded UI pass: its instance and
application/build identity, monotonically
increasing observation identity, `UiSnapshot`, bounded primitive application
facts and collection metadata. Reading an idle host may repeatedly return the
same observation. Reads never request repaint or enumerate off-screen domain
data. Collection metadata describes what was retained, whether it is
virtualised and whether its observed collection is complete. A missing total
does not imply an empty domain collection.

`query` returns bounded semantic nodes and their resolved action targets.
`discover` returns capability ID, label/description, scope, shortcut,
availability, argument contract, semantic-invocation support and a resolved
target. Semantic invocation is available only where the application has opted
in a typed action. Physical-only controls remain discoverable. Disabled and
hidden availability remain distinct.

An action takes at most one argument. Its contract is `"none"` or an object
tagged by `kind`:

- `{"kind":"text","max_length":N|null}`: a string of at most `N` characters;
- `{"kind":"boolean"}`;
- `{"kind":"number","minimum":A,"maximum":B,"integer":bool}`;
- `{"kind":"choice","options":[{"id":…,"label":…}]}`: one option ID.

Send the value as `"arguments": {"value": …}`. A no-argument action accepts
only a missing, `null` or empty `arguments`, as before. Invalid values are
rejected with `invalid_arguments` and a message naming what is accepted,
before the application sees them, and are checked again against the current
contract when the action runs. Record Desk's category edit and its category
and review filters are choice actions, for example
`{"capability":"record-desk.edit-category"}` with `{"value":"ideas"}`.

Every framework text field, an egui text edit presented through
`PresentationContext::native` and observed as a `text_input` or
`multiline_text_input`, is bound to the framework capability
`stobie.set-text`. Its identity is the field's semantic ID and its contract
is text. Invoking it replaces the field's text through the same edit route as
typing: the host focuses the field, selects its text and pastes the new text
over it (or deletes it), then returns focus to where it was. A single-line
field therefore turns newlines into spaces, as a paste does, and a field's
character limit applies. A field that held focus sees focus leave and return,
as when a user clicks into another field and back. A disabled or password
field is never set. Its receipt stays `queued`, and can no longer be
cancelled, until the edit has applied and the intents it caused have settled;
it then completes with the observation that presents them. If the field is no
longer presented by then, the receipt is rejected. Hosts publish each text
field's text as its node's optional `value`; a combo box's `value` is its
selected option's label.

Queries and discovery default to 64 entries and accept at most 256. JavaScript
callers may pass `{limit, cursor}`. A returned `next_cursor` binds to the same
observation; an intervening publication can return `stale_cursor`. Start a new
bounded query when that occurs.

`invoke` discovers exactly one target, sends its current resolved identity and
meaning once, then reads the correlated receipt while it is queued. A
completed receipt carries `after_observation`, the first observation that
includes its effect. Wait for an observation at least that new before reading
the outcome or discovering a target whose meaning the action changed. An explicit
`expected` target may be supplied to exercise a previously discovered target.
The application rechecks identity, meaning, current availability and arguments
at its normal validated mutation route. A `completed` receipt establishes that
dispatch; observe or wait for the task's intended result afterwards.

If a reply is lost or the wait times out, `uncertain_completion` retains the
original `request_id`. Read that receipt instead of repeating the mutation:

```sh
stobie receipt REQUEST_ID
stobie cancel REQUEST_ID
```

Explicit cancellation can cancel a queued invocation. `too_late` means the
action has begun or finished; it does not undo it. Local `AbortSignal`
cancellation stops a client wait and cannot establish that an already sent
mutation was cancelled. Receipt lookup is read-only. Unknown requests are
reported explicitly. Host mutation history retains up to 256 IDs without
eviction; reaching the limit fails closed rather than forgetting duplicate
protection. A restart creates a new instance, so old receipts cannot be looked
up in the new instance.

## Wait for outcomes and use physical input

`ApplicationClient.wait` polls completed observations with a monotonic deadline
and optional `AbortSignal`. The CLI's deadline is `--timeout MS` (default
15000); condition objects take no timeout field, and unknown condition fields
are rejected. Conditions include `present`, `absent`, `enabled`,
`capability_enabled`, `focused`, `selected`, `checked`, `name`, `value`, `status`, `selection_changed`, `fact`
and `audits_clear`. Checked compares a boolean `value`; audits-clear waits for
empty semantic/text findings with retained coverage. Name, value and
status conditions compare exact text with `value`; value compares a text
field's published text or a combo box's selected label; status restricts the role to
`status`. Selection change compares selected stable IDs, against `previous` or
the first observation. Fact conditions compare a named application-published
primitive value; the available names are visible in `observe`.
`enabled` tests a rendered control; `capability_enabled` tests the discovered
application availability, including a supported action whose menu is closed.
Timeout/cancellation diagnostics retain the last observation and relevant
failure reason in both the JavaScript error and machine-readable CLI output.

```sh
stobie wait enabled --capability record-desk.apply
stobie wait fact --key undo_entries --value 1
stobie wait value --id record-desk.title.1001 --value 'Revised title'
```

Physical input is a JavaScript client feature (see the script below); the
`stobie` command uses the semantic route.

Physical pointer targeting requires one enabled node with finite positive
geometry inside the completed root. It confirms the same node, root, scale and
host geometry in three samples spanning at least 100 ms within 15 seconds, then
rechecks them immediately before input. Movement, disappearance or disabled
state resets confirmation; ambiguity and invalid/clipped geometry fail
explicitly. An idle observation can supply all three samples without new frames.
Coordinates account for the logical root origin and viewport/device scale.

The adapters send actual pointer, keyboard and text input. For keyboard/text,
an optional selector requires one currently enabled, focused node and retains
that observation in the physical dispatch evidence. A dispatch record reports
`outcome: "unverified"`; target stability and successful input submission do not
establish the product outcome. Observe or wait for it separately. An uncertain
physical operation likewise requires observation before another input attempt.
Physical request IDs correlate client evidence; they are not retained in the
semantic invocation ledger and cannot be recovered using `receipt`. Physical
input has no replay guarantee. Observe its result after an unconfirmed failure
before deciding whether another input is appropriate.

This script uses the same client on either host, edits a field physically, then
invokes Apply through the semantic action route:

```js
import {
  ApplicationClient, SocketTransport, NativePhysicalAdapter,
} from './tools/application-client.mjs';

const client = new ApplicationClient(new SocketTransport(process.env.STOBIE_AUTOMATION_SOCKET));
const input = new NativePhysicalAdapter(process.env.STOBIE_WINDOW_ID);
await client.hello();
const field = { id: 'record-desk.title.1001' }; // Resolve the selected record's actual ID first.
await client.click(field, input);
await client.wait({ condition: 'focused', selector: field });
await client.key('Control+a', input, { selector: field });
await client.text('Revised title', input, { selector: field });
await client.wait({ condition: 'fact', key: 'draft_title', value: 'Revised title' });
const receipt = await client.invoke({ capability: 'record-desk.apply' });
if (receipt.state !== 'completed') throw new Error(JSON.stringify(receipt));
await client.wait({ condition: 'fact', key: 'draft_dirty', value: false });
```

For a browser host, construct `ApplicationClient(new BrowserTransport(page))`
and `new BrowserPhysicalAdapter(page)`. The remaining client calls are identical.
Use a receipt/postcondition pair appropriate to the application rather than
assuming a particular initial history depth or record identity.

## Capture evidence and retain failures

`capture` brackets the host canvas/window screenshot with completed semantic,
text and fact observations. It retains instance and application/build identity,
root rectangle, host dimensions, scale, input route, wall/monotonic capture
times, before/after observation IDs and each failed stage. Browser capture uses
the host canvas; native capture uses only the configured X11 window.

```sh
stobie capture --output /absolute/path/host.png
```

JavaScript callers may supply `inputRoute` to record the preceding exercise,
for example `physical_pointer_keyboard` or `semantic`. If a screenshot fails,
the after-observation is still attempted and the partial semantic evidence is
retained. If a bracket observation fails, a successful image remains available
with that diagnostic. A metadata-write failure also returns partial evidence,
retaining the observations and capture report in the returned object. `stobie
capture` exits with status 5 for partial evidence. Host
capture cannot identify the exact GPU/OS frame for a completed UI submission;
the report states that limit and never claims an atomic screenshot/snapshot.
`BuildIdentity.source_revision` is optional: build with
`STOBIE_BUILD_REVISION` to supply it, and retain binary/WASM hashes with the
qualification evidence when exact artefact attribution is required.

## Adopt the interface in another application

Host the application with `AppHost` (see the
[composition guide](application-composition.md)). The host creates the
`Inspection`, installs its completion hook, and starts the native socket when
`STOBIE_AUTOMATION_SOCKET` is set. In the browser, `web_handle!` exposes
`enable_automation` and `automation_request`. Each completed, non-discarded pass
is published with one binding per action target. Its identity is the first
rendered control that presents the target, and its domain is that control's
domain reference or, for a pane-scoped target, the pane. An application-scoped
action with no rendered control keeps its default identity. A pane-scoped
action is discoverable where its control is rendered with a pane, and in the
panes `Application::action_panes` lists. Facts come from
`Application::observe`. Queued invocations drain on the application thread
before the UI runs. Bindings are recalculated before each one, and availability
is rechecked immediately before `perform`. Reads never request a repaint.

Applications decide two things. `meaning` names the assumptions whose change
makes a previously resolved target stale, such as selected domain identity or
editing revision. Leave unrelated repaint and focus counters out of it. An
action declared `physical_only` stays discoverable with `semantic_invocable`
false.

An application that wires its own pass uses the same public API. Stage each
pass with `stage_completed`, drain with current bindings through the validated
route, and construct browser services with `Inspection::with_instance(application,
build, nonce)?`. The nonce must be an opaque value that changes on each startup:
the maintained bootstraps use `crypto.randomUUID()`. Never derive it from a WASM
process ID, wall clock or resettable counter, which can repeat after reload.

The bounds are 4,096 retained nodes, 512 measured text observations, 2 MiB
publication JSON, a 256 KiB request, 256 entries per query/discovery, 16 queued
invocations and 256 non-evicting mutation IDs per instance. Native request
reading has a two-second deadline. These are engineering limits, not evidence
that a whole domain collection was inspected. Unknown versions, stale
instances/cursors/targets, unsupported arguments and unavailable actions fail
explicitly. This interface provides no remote service, arbitrary evaluation or
unrestricted state setter.

## Use the browser accessibility tree

Without `?automation=1`, a hosted browser application still exposes a hidden
[ARIA layer](ui-guides/accessibility.md#browser-aria-layer). A general browser
agent can use it with Playwright role locators or an accessibility-tree
snapshot. It offers nothing that a pointer or keyboard user cannot do.

- Find controls with `page.getByRole(role, { name, exact: true })`. Names are
  the controls' accessible names, and states such as `pressed`, `selected` and
  `disabled` are ARIA states.
- Activate an enabled control with `locator.click()`. Each enabled control
  holds a one-pixel hit target at its centre, so the click is a physical
  click there. `locator.dispatchEvent('click')` and `element.click()` become
  an AccessKit click, which egui handles as a click on that control. A
  disabled control has no hit target, so Playwright waits for it to be
  enabled.
- Lantern's role/name targets work the same way, for example
  `lantern click --role button --name Apply --strict`. A name shared by two
  controls, such as one tool button per image pane, is ambiguous; use
  `--selector '[data-stobie-id="…"]'` instead. Type with
  `lantern type --role textbox --name "Record notes" --text … --strict`
  without clicking the field first: a click focuses it in egui, whose text
  agent then shares the field's name.
- Elements carry the stable semantic ID as `data-stobie-id`, so
  `page.locator('[data-stobie-id="record-desk.apply"]')` finds the control the
  inspection interface calls `record-desk.apply`.
- Read a text field's value from the snapshot, for example
  `textbox "Record title": Survey notes`.
- Edit text with `locator.fill(text)`, or `locator.focus()` and then
  `page.keyboard.type(text)`. The layer field keeps focus, and each edit
  reaches egui as typed input, so the application sees the same change as
  typing. Escape or Tab returns focus to the application.
- Action buttons list their shortcuts in `aria-keyshortcuts`, for example
  `Control+S Meta+S`.
- Read results from the tree again after a pass. A disabled control's
  `aria-description` gives its reason.
