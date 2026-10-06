# Record Desk consumer evidence

The [execution plan](record-desk-plan.md) owns acceptance and the short design
contract. The [consumer README](../consumers/record-desk/README.md) owns executable
commands, lifecycle, extension and persistence. Detailed generated logs,
metadata, step snapshots and captures stay in ignored runtime evidence.
The [qualification manifest](record-desk-evidence/qualification.json) records
source/build/capture hashes. [PR #47](https://github.com/robchristie/polyorama/pull/47)
owns exact-head review, candidate and post-merge CI, and terminal landing evidence.

## What the consumer establishes

Record Desk is an excluded, single-package Cargo workspace with its own manifest,
lockfile and profiles. `tools/check-record-desk.py` retains `cargo metadata
--locked` and checks its workspace root/member and resolved Polyorama crates:
only `polyorama-core` and `polyorama-ui-egui`. No Lab, Gallery, Emuella, worker or
image-renderer package is present; there is no source inclusion shortcut.
Eframe/egui and storage/serialisation libraries are ordinary declared dependencies.

The application owns record identity/schema, validation, filters, selected
identity, transient drafts and history. Public pane recipes present narrow views
and emit intents. One changed Apply commits one validated transaction; invalid,
cancelled and normalised unchanged edits produce none. Filters preserve a hidden
selection and draft. Explicit Save stores committed records and the only dock
tree, excluding draft/filter/selection/focus/history. Versioned native JSON and
browser localStorage restore records/layout without fabricating image state.

## Demonstrated framework decisions

- UI's default-enabled `image-rendering` feature preserves existing image APIs
  while allowing this consumer to resolve core/UI without the image renderer or
  runtime. Host GPU ownership stays with eframe.
- `dock_workspace_layout` emits `WorkspaceResize`, whose `apply` validates the
  current split before mutation. The existing `dock_workspace` still returns
  `Command::ResizeSplit`. Record history remains consumer-owned. Restore now
  rejects invalid split fractions and tab groups through `Workspace::validate`.
- Physical keyboard layout adjustment exposed missing splitter focus after a
  pointer click. The shared dock now requests focus on click/drag start, with a
  regression covering the compatible resize command.
- AccessKit parity review exposed a consumer splitter name/description mismatch
  and partially clipped action targets. Consumer observations now match shared
  dock semantics; shared action buttons expose the visible interaction bounds
  consistently in AccessKit and `UiSnapshot`, with an enabled/disabled clipping
  regression. Consumer-owned row and editable-field bounds follow the same rule.
- `choice_control_with_options` adds stable, currently visible popup identities
  and geometry to the existing choice recipe. Floating option observations are
  not clipped to a pane. Native editable fields have explicit text-input roles
  and coverage exclusions; egui's internal editing layout remains unmeasured.

Session defaults and `validate_image_cameras` remain analytical-image contracts.
They do not obstruct a consumer that owns its non-image domain state. No generic
session, application history or preference framework was introduced. The
[composition guide](application-composition.md) is the authoritative public route.

## Behaviour and execution evidence

| Surface | Evidence |
|---|---|
| Direct domain/store tests | 12 model and five store tests: stable filtering/selection, draft rejection/cancellation, transaction/redo behaviour, identity/layout/envelope validation, bounded input, atomic replacement and protected malformed/unsupported bytes; one visibility guard regression preserves invalid allocation evidence |
| Public egui input tests | Eight integration tests, including the four retained authoring-probe tests: editing/history, every startup presentation, ordinary/narrow/empty observations, open/closed filter options, Reset button/shortcut/availability and preservation invariants |
| Native physical workflow | Release executable under Xvfb, wgpu GL/Mesa llvmpipe; xdotool pointer/keyboard searches, traverses/selects a record, chooses filters, rejects invalid Apply, commits title/reviewed together, undoes/redoes, adjusts the splitter by keyboard and saves/restarts |
| Browser physical workflow | Release WASM in headful Chromium/WebGPU SwiftShader; Playwright mouse/keyboard performs the same workflow, saves/reloads, restores records/layout and excludes an uncommitted draft |
| Failure fixtures | Malformed native file/localStorage bytes are supplied directly, then visible feedback and blocked Save are observed physically. A direct browser quota fixture exercises visible Save failure/retry; native write failure is a store test |
| Observation and repaint | Each retained step asserts clean semantic/text audits. Stable record/pane/field/option identities target real geometry. Both hosts settle without another presentation frame during a 350 ms idle observation after startup deadlines |

Public UI tests additionally compare actual egui/AccessKit node data with the
consumer snapshot at 1080×760 and 390×844, using the full synthetic collection,
open/closed options, hidden selection, invalid/committed edits, undo/redo and an
empty collection. The harness consumes egui updates; the audit traverses its
resulting AccessKit tree rather than deriving nodes from the consumer model.
The regression initially reproduced splitter name/description and clipped
button/record-row bounds findings, then passed after their repair.

The read-only snapshot hooks supply state and current geometry, with no command
injection. Direct tests/fault setup are separate from physical interaction proof.
Native/browser reports record source revision, dirty status, binary/WASM SHA-256,
backend/browser configuration, input route and step observations. Standalone
smokes write `.tools/runtime/record-desk-evidence/record-desk`; canonical verification
writes `.tools/runtime/verification-evidence/record-desk`, including metadata,
workflow JSON, runtime logs and PNGs. Exact committed-candidate journeys and CI
are attributable through the owning PR and its landing record. Full CI retains
the `record-desk-evidence` artefact for 14 days on success and failure.

Final CI qualification exposed a legacy Lab idle-check timing failure: one
additional frame after a fixed 250 ms delay was labelled continuous repaint.
A bounded headed-browser diagnostic probe observed 1.8 seconds of stable frames,
no new repaint requests and empty work queues. The Lab smoke now first requires
700 ms of observed quiescence within a 3.5-second settling budget, then retains
the separate unchanged 700 ms idle window. Six observer regressions reject
continuous/periodic painting, painting during that final window and invalid
observations, while accepting one deferred interaction frame. Failure bundles
also retain application diagnostics. This repairs qualification timing without
changing the application repaint policy.
The local qualification also exposed the minimal example's in-place snapshot
write race: the reader could see an empty file after its initial valid check.
Its native smoke now retains one parsed observation for geometry/evidence and
resolves the current action before each click; application behaviour is unchanged.

The next CI run exposed a transient popup target between the browser smoke's
initial presence check and its fixed-delay geometry read. Its retained failure
image and following snapshot showed the open, enabled option. Pointer targeting
now requires three identical enabled geometry observations across 100 ms within
a 15-second budget, retaining the actual frame/rectangle used for each click.
Six regressions cover transient absence, moving geometry, exact identity,
unavailable targets and invalid bounds. A bounded physical journey with fourfold
Chromium CPU throttling passed after this repair. This is read-only input
synchronisation; application state still changes through physical events.

The following CI run passed that browser journey, then exposed five invalid
native toolbar targets in the first panel sizing pass. A regression capturing
every presentation reproduced the same bounds (`min_y=35`, `max_y=33`). The
consumer now omits a toolbar action only when its allocated rectangle is finite
and positive and its current interaction bounds prove complete finite clipping.
Partially visible actions remain; invalid allocations/non-finite observations
remain audit failures. Shared recipes and repaint policy are unchanged. Startup
tests cover every presentation at both widths, and host journeys require all
principal toolbar targets before declaring readiness. The minimal native smoke's
error-log scan also uses a portable grep and fails closed if inspection fails.

The browser host reserves only registered application chords while its canvas
or text input owns focus: browser Find/Location/Reload defaults otherwise steal
focus before egui consumes them. The Linux consumer smoke supplies bundled
Source Sans 3 to Chromium when the minimal host has no system fonts. Traversal
and popup placement are observed after their following egui pass before the
next physical input; no state-setting hook is used.

## Opened-image review

The short contract in the plan was compared with the maintained
`application-shell-dark` reference and opened native/browser captures at
1080×760 and 390×844. List/navigation, linked detail and commit actions are clear;
selection, disabled history/actions, field labels and persistence feedback are
visible. Shared analytical tokens/recipes produce aligned controls without
decorative containers. At narrow width the canonical split stacks; both panes
scroll locally, Apply/Cancel remain reachable and long storage descriptions
wrap/ellipsise. Partial bottom list rows and the keyboard caption indicate local
scrolling rather than horizontal overflow. The no-results state preserves the
selected editor with an explanation; narrow malformed-state feedback wraps
without covering the primary actions.

The first review found overly tall filter chrome; compacting it improved the
stacked list. The final review found no blocking overlap, truncation or hierarchy
defect in these states. The captures below are opened inspection artefacts rather
than new golden-image compatibility requirements:

- [Native ordinary](record-desk-evidence/native-ordinary.png) and
  [native narrow](record-desk-evidence/native-narrow.png).
- [Browser ordinary](record-desk-evidence/browser-ordinary.png) and
  [browser narrow](record-desk-evidence/browser-narrow.png).
- [No matching records](record-desk-evidence/browser-no-results.png) and
  [protected malformed state](record-desk-evidence/browser-error.png).

Lantern's fixed live adapter currently targets Gallery. This consumer uses its
maintained physical harness and shared semantic/text audits; attempts to consume
ad hoc local bundles were unavailable, so no Lantern adapter success is claimed.

## Bounded fresh-context authoring proof

An independent agent received this consumer, public composition/UI documentation
and one bounded task: add Reset filters with correct availability and keyboard
behaviour. It did not require existing application implementation or private
framework APIs. Its retained changes add the action specification, pane intent,
availability/handler, Ctrl/Cmd+Shift+F and four passing public UI tests.

The probe exposed undocumented orchestration knowledge: egui's logical shortcut
matcher accepts extra Shift/Alt modifiers, so a disabled shifted shortcut could
fall through to the plain Search chord. Exact registered modifiers now guard
routing, with a disabled-input regression and extension guidance. It also
corrected the application's action-target construction to respect public action
scope, and confirmed typography installation needs a presentation pass before
font-dependent observations. The README documents the complete extension path.
Normal consumer/canonical checks and independent PR review cover these retained
changes; this was one authoring probe, not an agent benchmark programme.

## Limits

This qualifies a small local non-image workflow, software-rendered Linux native
and Chromium WebGPU execution. It does not establish every platform/GPU/browser,
a browser accessibility adapter or screen-reader support. Storage is local,
explicit and version 1; no migration, backend, backup or cross-device sync is
provided. History is transient and bounded. Empty saved collections are valid
and explain how to restart the synthetic collection without silently deleting
saved data. The private HTTPS preview was ready, but its identity gate returned
403 to the test client; the documented local browser route supplies runtime
qualification. Preview readiness is not claimed as browser verification.
