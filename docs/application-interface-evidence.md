# Application interface qualification

The [public adoption guide](application-interface.md) owns connection, protocol,
integration and failure semantics. The [execution plan](application-interface-plan.md)
owns package acceptance. Detailed generated observations, receipts, command logs
and capture metadata stay under ignored `.tools/runtime/`.

## Design and calibration

The source-grounded advice at base
`57e5d906fe7f40521049560d20605d47cbc6d724` selected the existing UI semantic owner,
application-owned typed bindings and validated mutation routes. It identified
legacy snapshots' pre-output geometry/post-output state mixture and widget
action variants with no semantic handler. The new interface stages coherent
presentation-state observations before outputs and publishes from egui's
non-discarded end-pass hook. Receipts separately describe queued or validated
dispatch; a subsequent observation proves the requested result.

The initial four-host slice queried and discovered Record Desk Arrange and Lab
Link views, invoked their normal routes, observed a layout/link transition,
replayed identical requests without a second toggle and rejected stale guards.
Both applications/hosts settled and retained an unchanged 700 ms idle window
while the common client inspected them. Captures were bracketed by observations,
with exact GPU/OS frame correspondence explicitly unavailable. Calibration
artefacts are in `.tools/runtime/interface-slice/application-interface/` and
are attributed to dirty prototype builds, not final qualification.

## Shared and application-owned work

`polyorama-ui-egui::Inspection` owns the versioned serialisation, retained
publication, query/discovery bounds, request ledger, execution revalidation and
private native transport. The common JavaScript client owns transports, waits,
physical target stability/recheck, keyboard focus evidence and capture reports.
The Record Desk target helper is now a compatibility adapter to that shared
targeting implementation; its first-action fallback now rejects ambiguity.
Existing physical regression journeys still send real host input.

Each app supplies typed `ActionKey` bindings, current availability, semantic
invocability, meaning guards, collection metadata and bounded primitive facts.
Record Desk keeps draft validation and record history; Lab keeps image-intent
validation and command history. Physical and semantic Fit generate the same
intent. The independent consumer still resolves only core/UI framework crates,
with image rendering disabled; no image runtime is required by inspection.

Legacy hooks remain for a bounded purpose: Lab `test_action` supplies deterministic
worker/camera/error fixtures and old specialised regressions; Lab `test_snapshot`
and native F12 snapshots retain render/runtime qualification; Gallery snapshot,
manifest and fixture tooling owns catalogue baselines; Record Desk's snapshot
hooks retain domain/persistence fault fixtures and established physical journeys.
New shared journeys use no application-specific snapshot parser or JavaScript
hook. There is no requirement to migrate every application in this package.

## Fresh-context usability

One fresh agent read the product rules and public interface guide, without
application source or old hook formats. Using the common CLI it discovered and
queried Record Desk Arrange and Lab Fit view, invoked Arrange and discovered
Toggle diagnostics, then verified public facts through common waits. Both tasks
passed. Record Desk initially retained a queued receipt while the Lab window
covered it; exposing the native window allowed completion. The agent reconciled
the original request rather than retrying. Guidance now explains native host
visibility and the difference between discovered capabilities and rendered
controls. Exact commands, responses, timed failures/recovery and binary hashes
are retained in `.tools/runtime/application-interface-usability/`.

## Qualification state

The [qualification manifest](application-interface-evidence/qualification.json)
binds all four journeys to clean source checkpoint
`60c286a8fe7b5e81dbbc69d3cd86f114cce131c5`, executable/WASM hashes, application
instances, observation identities, input receipts and capture timing/hashes.
Full reports remain in ignored `.tools/runtime/interface-final-source/application-interface/`.
Canonical verification writes the same reports beneath
`.tools/runtime/verification-evidence/application-interface`; CI retains them
on success and failure for 14 days. [PR #48](https://github.com/robchristie/polyorama/pull/48)
owns exact-head review, required checks, squash identity and post-merge CI.

| Surface | Result |
|---|---|
| Shared Rust contract | 23 focused tests: negotiation, strict/correlated failures, bounds/cursors, repeated capabilities, current availability/meaning, duplicates/conflicts, queue/history limits, cancellation/restarts, missing observations, discarded passes, physical geometry and private native transport |
| Common client and compatibility targeting | 47 tests: bounded/cancelled waits, idle conditions, capabilities without rendered controls, moving/disabled/clipped targets and scale, uncertain completion without replay, focus/keyboard evidence, browser readiness, unchanged native pointer placement with one click, and partial capture/metadata diagnostics |
| Record Desk integration regressions | Current narrow-window policy before queued dispatch; draft-field capabilities hidden without selection; retained Apply/Undo targets survive Search focus and layout resize while genuine draft/history changes become stale; ordinary consumer domain/store/UI/dependency checks retained |
| Lab integration regressions | Polygon discovery and rendered availability agree at two and three vertices; partially visible row targets use the scroll interaction clip and completely clipped valid allocations are omitted; shared AccessKit bounds match; collection materialisation remains bounded |
| Record Desk, both hosts | Discover/query; physical title and Reviewed edit; retained semantic Apply target survives Search focus and yields one transaction; replay yields no second entry; undo/redo availability and state; invalid, unavailable, stale and unsupported-argument rejection without mutation; matching physical Apply/undo/redo workflow |
| Lab, both hosts | Fit changes observable scale through the normal validated camera intent; semantic and physical Link changes observed; pane/domain query and two-row continuation over the rendered subset of one million logical rows |
| Cross-host failures and idle | Unsupported version/operation, invalid selector, stale target and ambiguous pane failures; absent query, timeout/cancellation; unchanged warmed presentation frames during read-only inspection |
| Canonical verification | `cargo xtask verify` passed: format, native/WASM lint, tests, architecture, docs, release builds, packaged startup, five immutable UI fixtures, existing physical workflows and the shared four-host journeys |

Local canonical execution used the maintained headed-browser/Xvfb route and
pinned sysroot libraries. An earlier headless attempt timed out in the existing
packaged-startup journey; an earlier socket fixture exceeded Unix path length
under the canonical temporary directory. The latter was repaired without
weakening its assertions. These failed attempts are retained; success is from
the completed campaign and applicable focused repairs, not a summary of them.

Independent review rejected the earlier candidate's incomplete Lab polygon
context and Record Desk guards tied to unrelated UI changes. Both integrations
now derive capability availability and meaning from the relevant application
state, with direct service/UI regressions and the retained-target host journey.
The earlier candidate CI also exposed older `xdotool` waiting indefinitely for
a movement event when the cursor was already at its target. The common adapter
now verifies the resolved pointer position before one click. A later canonical
scroll journey exposed a zero-area interaction rectangle at a clip boundary;
only valid fully clipped row allocations are now omitted, preserving invalid
allocation diagnostics. The final canonical run and all four clean-checkpoint
journeys passed after these repairs.

## Opened-image inspection

Opened all four qualified host captures. Record Desk shows the committed physical
edit, Reviewed state, history controls and unchanged list/detail composition.
Lab shows the fitted primary view, linked view controls and bounded Results
pane with its logical collection size. No overlapping primary controls or new
presentation defect was found in these ordinary fixtures. Geometry/parity checks
own the clipped-row claim; screenshots do not prove exact frame correspondence.

- [Record Desk native](application-interface-evidence/record-desk-native.png)
  and [browser](application-interface-evidence/record-desk-browser.png).
- [Lab native](application-interface-evidence/lab-native.png)
  and [browser](application-interface-evidence/lab-browser.png).

## Capture and host limits

The configured private HTTPS preview was ready on the owned generation, but
its identity gate returned HTTP 403 to the test browser. That failed access is
retained separately; it does not establish preview browser verification.
The maintained local browser test route supplies application qualification.
Native transport is Unix; native physical input/capture is X11. Software Mesa
llvmpipe and Chromium WebGPU SwiftShader establish functional behaviour, not
hardware performance or other platform support. Observations cover current
rendered nodes only, never materialise the million-row/100,000-thumbnail domain,
and retain collection/audit coverage bounds. Duplicate guarantees are bounded
to one live instance, not durable exactly-once execution across restarts.
