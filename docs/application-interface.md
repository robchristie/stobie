# Inspect and exercise a Stobie application

The shared application interface lets a client read current semantics and text,
discover typed capabilities, dispatch a validated action, wait for an observed
result and exercise the same host through physical input. Record Desk, the
component Gallery and Analytical Workspace Lab expose the same version 1 JSON contract on an explicitly
enabled local Unix socket or browser page. Rust types in
[`stobie-protocol`](../crates/stobie-protocol/src/inspection.rs) are
authoritative, and `stobie-ui-egui` re-exports them; the JavaScript client
does not maintain a separate action registry or schema.

The [execution plan](plans/application-interface-plan.md) and
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
the client. It also carries an optional `renderer`, absent when unknown: the
running GPU `backend` (`webgpu` or `webgl2` in a browser, wgpu's backend name
natively), its `adapter`, a `selection` of `preferred`, `fallback` or `forced`,
and an optional `reason`. Other calls require the same instance. The CLI performs `hello`
before its selected command; a JavaScript caller performs it once explicitly.
An application that has just started answers `missing_observation` until its
first pass publishes; the Rust client, `stobie` and the JavaScript client wait
for it within each read's deadline.

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
facts and collection metadata. A hosted application's facts also carry the
framework's: `stobie.theme`, `light` or `dark`, and `stobie.contrast`,
`standard` or `high`, name the theme the pass was drawn in, the
application's appearance preference resolved against the system's
(`THEME_FACT` and `CONTRAST_FACT`). They replace an application fact of the
same name, so `stobie wait fact --key stobie.theme --value dark` waits for
the page to be drawn dark. When a pass exceeds the publication bounds below,
or its snapshot is invalid, the host publishes an observation in its place
that presents only the root node, the two theme facts and
`stobie.publication` (`PUBLICATION_FACT`), which says why: `too_large:`
followed by what was over its limit, such as `too_large: 5000 nodes, over
the limit of 4096`, or `invalid:` followed by the reason. It offers none of
the application's capabilities, so an agent reads the failure rather than
the previous pass, and the fact is gone once a pass publishes again. The
host also reports the first failing pass through `Application::report_error`.
Reading an idle host may repeatedly return the
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

A control can carry its action's argument, such as Open on each row of a list
carrying that row's ID. Its node publishes the value as `argument`, and the
host binds it as its own capability whose `argument`, and its target's, is
that value; the action keeps its own binding, without an `argument`, which
takes any value the contract allows. A selector's `argument` matches only
nodes and capabilities carrying an equal value (numbers compare by value).
Invoke a row's control with no `arguments`, which means its value, or with
that same value; any other value sent to a selector that matches that
control alone is `invalid_arguments`. A value also chooses among the
controls a selector matches: a control carrying another value cannot take
it, the one carrying it is preferred over the action's own binding, and a
value none of several can take is `no_match`. The Rust client and `stobie`
apply the same choice, then address the chosen control by its ID; the
JavaScript client's `invoke` needs a selector that discovers one binding,
such as one with `argument`, or several that perform the same thing. So `{"capability":"people.open"}` with `{"value":2}` invokes the
row carrying 2 when it is presented, and otherwise the action's binding:

```sh
stobie discover people.open                # one per row, `= 2`, and the action's own
stobie invoke people.open --value 2        # the row carrying 2
stobie invoke people.select --argument ada # the row carrying "ada"; no value needed
```

Several controls can present one action with one value, such as Open for
one row in two lists, or a dialog's dismiss action on its close control
beside a Cancel button. Each is a binding of its own, with its own ID, so a
caller can address any of them. Bindings that perform the same thing, which
share their capability, pane, carried value, meaning, availability and
semantic-invocation support (`CapabilityDescription::performs_same_as`), are
one choice rather than `ambiguous`: their domains and labels may differ, and
invocation resolves to the one with the least semantic ID. An `expected`
target naming another of them keeps that one. Either way the action runs
once, its receipt's `target` names the binding that ran, and its
`after_observation` is the first observation that presents the effect, which
is the same whichever control was chosen. Framework capabilities, such as
two text fields' `stobie.set-text`, never perform the same thing. The Rust
client's `discover_one`, and both clients' `invoke` without a target and
`capability_enabled` wait, make the same choice.

A row removed after discovery no longer matches (`no_match`). One removed
after its invocation was admitted is refused when it runs: `unavailable`
when the application's availability for its argument hides it, otherwise
`stale_target`. A target whose control now carries another value, or none,
is `stale_target`. The [interactions guide](ui-guides/interactions.md#actions-on-rows)
describes the application's side.

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
field is never set; an agent in a browser fills a password by focusing the
field and typing (see the [`stobie` guide](agent-cli.md#browser)). Its receipt stays `queued`, and can no longer be
cancelled, until the edit has applied and the intents it caused have settled;
it then completes with the observation that presents them. If the field is no
longer presented by then, or it did not take the text, the receipt is
rejected and says so; it never completes without the edit. Hosts publish each text
field's text as its node's optional `value`; a combo box's `value` is its
selected option's label.

Every copy control, presented through `PresentationContext::copy`, is bound
to the framework capability `stobie.copy-text`. Its identity is the
control's semantic ID and it takes no argument. Its meaning is
`text-revision:N`, a counter that moves on whenever the control's text
changes, so a target discovered before a change is `stale_target`; neither
the meaning nor any other observation carries the text. Invoking it copies
the text as a press does, on the host's next pass and outside any user
gesture: a browser that refuses that write leaves the control showing its
failure, which the next observation presents as a `status` node. A disabled
control is `unavailable`, and a hidden one has no capability. The
[interactions guide](ui-guides/interactions.md#copying-to-the-clipboard)
describes each route.

While a modal dialog is presented, the UI snapshot holds the root, the
dialog and what it presents, and only its controls and the actions it
presents are enabled. Discovery reports every other action `disabled` with the reason
`The dialog "<title>" is open`, and invoking one is rejected as
`unavailable`; close the dialog, through its dismiss action or its own
controls, first. The
[interactions guide](ui-guides/interactions.md#dialogs) describes the rule.

Queries and discovery default to 64 entries and accept at most 256. JavaScript
callers may pass `{limit, cursor}`. A returned `next_cursor` binds to the same
observation; an intervening publication can return `stale_cursor`. Start a new
bounded query when that occurs.

`invoke` discovers exactly one target, or one among bindings that perform
the same thing, sends its current resolved identity and
meaning once, then reads the correlated receipt while it is queued. A
completed receipt carries `after_observation`, the first observation that
includes its effect; the host runs the pass that publishes it even when the
action changed nothing. Wait for an observation at least that new before reading
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
reported explicitly, as an `unknown` receipt.

The host keeps the receipts of the 256 most recent invocations, rejected ones
included. Admitting another evicts the oldest receipt that is neither queued
nor running, so a long session never runs out of invocations. Duplicate
protection holds within that window: a request whose ID is still retained is
answered with its receipt or `request_id_conflict` and never runs again, while
one whose receipt was evicted is admitted as a new invocation. Once any
receipt has been evicted, an `unknown` receipt carries an `unknown_request`
error saying that older receipts were evicted, because the ID may have been
one of them. Read a receipt soon after an uncertain outcome rather than after
hundreds of further invocations. Only when all 256 retained invocations are
still queued or running is a new one refused with `history_full`. A restart
creates a new instance, so old receipts cannot be looked up in the new
instance.

## Reach controls out of view

Inside a visible-only region (`observe_visible_only`), a control or text
wholly scrolled out of view is not observed, so neither a query nor
discovery finds it. A host that enables inspection offers the framework
capability `stobie.reveal`, the application's, with its own ID. It takes the
semantic ID of a node as its text argument and scrolls that node into view:
on each pass that presents the node, the innermost scroll area around it
scrolls until the node is wholly in view, or as far as it scrolls. Its
receipt completes with the first observation that presents the node in
view, and is rejected, saying why, when no pass presents the node, a modal
dialog covers it, or the node cannot be scrolled into view at all.

The Rust client's `discover_for_invocation`, and so `invoke` and
`set_text`, `stobie invoke` and `stobie set-text`, and the JavaScript
client's `invoke` reveal a selector's `id` when it discovers nothing, then
discover again, all within one deadline; a failed reveal's reason appears
in the `no_match`. `Client::reveal`, `stobie reveal ID` and the JavaScript
`reveal(id)` reveal a node directly. A row's control has a derived ID, so
invoke a row's action through its value instead (see above).

```sh
stobie set-text record-desk.notes.1013 'Revealed notes'   # reveals it first
stobie reveal record-desk.notes.1013
```

A host test reveals a node with `AppHost::reveal` and runs passes until
`AppHost::reveal_outcome` reports the outcome, then acts on it, for example
by clicking it; the reveal works in any harness of the test's size. A
reveal finds a node that a presentation recipe draws, including a control
wholly clipped away, and an application node registered with
`observe_node` at the rectangle it gives. It does not find a row that a
virtualised list never lays out. Only the innermost scroll area around a
node scrolls for it, and an invocation made while the view moves can still
meet `stale_target`, which asks for discovery again.

## Content text

A semantic node names a control; the words a page presents are content text.
Each measured text observation (`snapshot.text`) of a content kind carries a
`content` field: a heading (`section_heading`), a content label, status or
fixed-slot text (`content_label`), reading text and rendered Markdown blocks
(`reading_text`), a Markdown list number (`list_marker`) and a link's text
(`link`). The order of `snapshot.text` is presentation order. Other kinds, the
text of controls, rows and chips, publish no `content`: their words are their
semantic nodes' names and values.

`content` is `{"kind":"excerpt","text":...,"truncated":...}` or
`{"kind":"withheld"}`. An excerpt is the text as laid out, Markdown's
presented words rather than its source, cut to the first 256 characters
(`CONTENT_EXCERPT_CHARS`) and then ending in `…`
(`CONTENT_TRUNCATION_MARKER`) with `truncated` true. Text elided on screen
still publishes its excerpt, since the elision is presentation. With the
bound of 512 text observations, content adds at most about 128 Ki characters
to an observation. Only text in view publishes content: text wholly scrolled
out of view or clipped away publishes none, and empty or zero-size text
publishes none either. Partly visible text publishes its whole excerpt,
inside a visible-only region (`observe_visible_only`, which the host applies
to each dock pane, navigation body and dialog body) as elsewhere. Its
observation keeps the clip it was drawn within, so `snapshot.text` tells how
much of it is in view. The host's audit (`snapshot.text_audit`) treats a
region's edge cutting bounded text, such as a heading or label, as scrolling
rather than clipping, and still checks everything else about its layout;
outside visible-only regions partly visible bounded text is audited as
presented. Auditing `snapshot.text` afresh with `audit_text_layouts` does not
know which text was in such a region and so reports that cut as
`text_outside_clip`.

Text presented inside `withhold_content_text(ui, |ui| ...)` publishes
`{"kind":"withheld"}`, with neither its words nor its length; its layout is
still measured and audited. Use it for what an application shows a person
but must not publish to an inspecting client, such as a one-time code.
Withholding governs observation only: the text keeps its accessibility node,
so assistive technology, and in a browser the page's ARIA layer, still
present it. A region governs content text only: a control's name, or a text
field's value, belongs to its semantic node and is published as before. A
link's semantic node keeps its text and URL, so withhold sensitive text
outside links, including links in withheld Markdown. A
password field needs no region: a password's text is never observed.

Rust callers read the excerpts with `UiSnapshot::content_text()` or a text's
`content_text()`; the CLI with `stobie observe --content`; and a JavaScript
caller from `snapshot.text[].content`. The `content` wait condition waits
until a published excerpt contains its `value`.

```sh
stobie observe --content
stobie wait content --value 'Record detail'
```

## Wait for outcomes and use physical input

`ApplicationClient.wait` polls completed observations with a monotonic deadline
and optional `AbortSignal`. The CLI's deadline is `--timeout MS` (default
15000); condition objects take no timeout field, and unknown condition fields
are rejected. Conditions include `present`, `absent`, `enabled`,
`capability_enabled`, `focused`, `selected`, `checked`, `name`, `value`, `status`, `selection_changed`, `fact`,
`audits_clear` and `content`. Checked compares a boolean `value`; audits-clear waits for
empty semantic/text findings with retained coverage. Name, value and
status conditions compare exact text with `value`; value compares a text
field's published text or a combo box's selected label; status restricts the role to
`status`. Selection change compares selected stable IDs, against `previous` or
the first observation. Fact conditions compare a named application-published
primitive value; the available names are visible in `observe`. Content
conditions take no selector: one waits until any published
[content](#content-text) excerpt contains `value`, and withheld text never
matches.
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
is published with one binding for each rendered control that presents an
action, whose identity is the control's semantic ID and whose domain is that
control's domain reference or, for a pane-scoped target, the pane. An
application-scoped action with no rendered control keeps its default
identity. A control carrying an argument has a binding of its own, and the
action keeps its default identity unless a control without an argument
presents it. Controls that present one action with one argument perform the
same thing, and invoking them together resolves to one of them, as
[above](#read-discover-and-invoke) describes. A pane-scoped
action is discoverable where its control is rendered with a pane, and in the
panes `Application::action_panes` lists. Facts come from
`Application::observe`. Queued invocations drain on the application thread
before the UI runs. Bindings are recalculated before each one, and availability
is rechecked immediately before `perform`. Reads never request a repaint.

Applications decide three things. `availability_with` decides availability
for one argument, such as one row's Remove, and hides an argument that names
nothing, such as a removed row's. `meaning` names the assumptions whose change
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

The bounds are 4,096 retained nodes, 512 measured text observations, 256
characters of each content excerpt, 2 MiB
publication JSON, a 256 KiB request, 256 entries per query/discovery, 16 queued
invocations and the receipts of the 256 most recent invocations per instance. Native request
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
