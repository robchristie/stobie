# Inspect and exercise a Polyorama application

The shared application interface lets a client read current semantics and text,
discover typed capabilities, dispatch a validated action, wait for an observed
result and exercise the same host through physical input. Record Desk and
Analytical Workspace Lab expose the same version 1 JSON contract on an explicitly
enabled local Unix socket or browser page. Rust types in
[`inspection.rs`](../crates/polyorama-ui-egui/src/inspection.rs) are authoritative;
the JavaScript client does not maintain a separate action registry or schema.

The [execution plan](application-interface-plan.md) and
[qualification evidence](application-interface-evidence.md) own the current
delivery state and exercised environments. Host examples below describe the
shared client; application startup, persistence and prerequisites remain in the
[Record Desk README](../consumers/record-desk/README.md) and
[browser startup guide](browser-startup.md).

## Enable a host

Install the repository's pinned browser tooling with `npm ci`. Native observation
uses Node's standard library. Native physical input and capture additionally
require X11, `xdotool` and ImageMagick's `import`; they target an explicit host
window, without shell evaluation. Unix socket observation does not require X11.

Native applications enable automation only when `POLYORAMA_AUTOMATION_SOCKET`
supplies an absolute path. Use a fresh private directory and launch the host with
that environment:

```sh
AUTOMATION_DIR=$(mktemp -d)
export POLYORAMA_AUTOMATION_SOCKET="$AUTOMATION_DIR/record-desk.sock"
cargo run --release --manifest-path consumers/record-desk/Cargo.toml
```

From another terminal, pass that same socket path to the CLI. The host refuses
to replace an existing path, retains a private socket and removes its own socket
on ordinary shutdown. Restart creates a new instance identity. Non-Unix native
hosting is unavailable. Starting without the variable disables automation;
closing the enabled host ends that instance.

The native window must be mapped and unobscured for eframe to process the
UI-thread invocation queue. A fully covered or minimised window can retain a
responsive read-only socket while an admitted action stays `queued`. Make the
host window visible (for X11, `xdotool windowraise X11_ID`), then look up the
original receipt and wait for the outcome. Do not submit the mutation again.
The client deadline reports uncertain completion while work remains queued.

Browser applications enable the fixed dispatcher only when their startup URL
contains `?automation=1`. Open the built app with that query string, then attach
Playwright to its Chromium CDP endpoint. Removing the parameter and reloading
starts a host without the dispatcher. The public `BrowserTransport` encapsulates
`window.__POLYORAMA_AUTOMATION.request`; callers do not evaluate application
globals or parse application-specific test snapshots.
The maintained bootstraps pass `crypto.randomUUID()` to
`handle.enable_automation(instance)` so each browser startup has a distinct host
nonce. Clients obtain it through `hello`; no nonce argument is needed in the CLI.

```sh
node tools/application-cli.mjs --cdp http://127.0.0.1:9222 \
  --url 'http://127.0.0.1:8080/?automation=1' hello
```

`--url` must select exactly one existing page. Native physical commands require
`--window X11_ID`, obtained from the actual host window. Browser physical input
requires exactly one canvas; use `--canvas SELECTOR` when the page contains other
canvases. The native configured window must cover the application's client
viewport: the client rejects mismatched root dimensions or scale.

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
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock observe
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  query '{"role":"text_input"}'
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  discover '{"capability":"record-desk.apply"}'
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  invoke '{"capability":"record-desk.apply"}'
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
hidden availability remain distinct. The current integrations support
no-argument actions; an unsupported argument object is rejected.

Queries and discovery default to 64 entries and accept at most 256. JavaScript
callers may pass `{limit, cursor}`. A returned `next_cursor` binds to the same
observation; an intervening publication can return `stale_cursor`. Start a new
bounded query when that occurs.

`invoke` discovers exactly one target, sends its current resolved identity and
meaning once, then reads the correlated receipt while it is queued. An explicit
`expected` target may be supplied to exercise a previously discovered target.
The application rechecks identity, meaning, current availability and arguments
at its normal validated mutation route. A `completed` receipt establishes that
dispatch; observe or wait for the task's intended result afterwards.

If a reply is lost or the wait times out, `uncertain_completion` retains the
original `request_id`. Read that receipt instead of repeating the mutation:

```sh
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock receipt REQUEST_ID
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock cancel REQUEST_ID
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
and optional `AbortSignal`. Conditions include `present`, `absent`, `enabled`,
`capability_enabled`, `focused`, `selected`, `checked`, `name`, `status`, `selection_changed`, `fact`
and `audits_clear`. Checked compares a boolean `value`; audits-clear waits for
empty semantic/text findings with retained coverage. Name and
status conditions compare exact text with `value`; status restricts the role to
`status`. Selection change compares selected stable IDs, against `previous` or
the first observation. Fact conditions compare a named application-published
primitive value; the available names are visible in `observe`.
`enabled` tests a rendered control; `capability_enabled` tests the discovered
application availability, including a supported action whose menu is closed.
Timeout/cancellation diagnostics retain the last observation and relevant
failure reason in both the JavaScript error and machine-readable CLI output.

```sh
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  wait '{"condition":"enabled","selector":{"capability":"record-desk.apply"}}'
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  wait '{"condition":"fact","key":"undo_entries","value":1}'
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  --window X11_ID click '{"id":"record-desk.title.1001"}'
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  --window X11_ID key 'Control+a'
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  --window X11_ID text 'Revised title'
```

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

const client = new ApplicationClient(new SocketTransport(process.env.POLYORAMA_AUTOMATION_SOCKET));
const input = new NativePhysicalAdapter(process.env.POLYORAMA_WINDOW_ID);
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
node tools/application-cli.mjs --socket /absolute/path/record-desk.sock \
  --window X11_ID --capture /absolute/path/host.png \
  --output /absolute/path/host.json capture
```

JavaScript callers may supply `inputRoute` to record the preceding exercise,
for example `physical_pointer_keyboard` or `semantic`. If a screenshot fails,
the after-observation is still attempted and the partial semantic evidence is
retained. If a bracket observation fails, a successful image remains available
with that diagnostic. A metadata-write failure also returns partial evidence,
retaining the observations and capture report in the returned object. The CLI
returns status 2 for partial evidence. Host
capture cannot identify the exact GPU/OS frame for a completed UI submission;
the report states that limit and never claims an atomic screenshot/snapshot.
`BuildIdentity.source_revision` is optional: build with
`POLYORAMA_BUILD_REVISION` to supply it, and retain binary/WASM hashes with the
qualification evidence when exact artefact attribution is required.

## Adopt the interface in another application

Keep the existing application-owned `ActionKey` enum, typed targets,
availability and validation route. Create an `Inspection` with application/build
identity and install its completion hook once before the first app update,
after other UI plugins. Stage each pass's owned snapshot, current
`InspectionBinding<A>` values, bounded `CollectionMetadata` and primitive
`ObservationValue` facts using `stage_completed`; discarded UI passes are not
published. The completed snapshot is an observation, not a second state owner.

A binding marks whether its typed action is semantically invocable and supplies
an application-owned `meaning` guard. Include assumptions whose change makes a
previously resolved target stale, such as selected domain identity or editing
revision; do not use unrelated repaint/focus counters as meaning. Bind exact
semantic IDs and domain references to the same controls used for physical input.
Discoverable physical controls can set `semantic_invocable` to false.

Native hosts call `start_native` with the UI waker, then drain queued actions on
the app thread through the existing validated route. Recalculate current
bindings before every `drain`, including between queued actions. Browser hosts
opt in before exposing the fixed `request(JSON-string)` hook and use the same
typed request/reply envelope. `dispatch` provides app-thread dispatch. Ordinary
reads must retain the completed observation and must not request repaint.
Browser integrations construct `Inspection::with_instance(application, build,
nonce)?` with an opaque host nonce that changes on each startup; the maintained
bootstraps generate it using `crypto.randomUUID()`. Do not derive it from a WASM
process ID, wall clock or resettable counter, which can repeat after reload.
See both maintained applications' `inspection.rs` modules for integration.

The bounds are 4,096 retained nodes, 512 measured text observations, 2 MiB
publication JSON, a 256 KiB request, 256 entries per query/discovery, 16 queued
invocations and 256 non-evicting mutation IDs per instance. Native request
reading has a two-second deadline. These are engineering limits, not evidence
that a whole domain collection was inspected. Unknown versions, stale
instances/cursors/targets, unsupported arguments and unavailable actions fail
explicitly. This interface provides no remote service, browser accessibility
adapter, arbitrary evaluation or unrestricted state setter.
