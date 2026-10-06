# Independent Record Desk consumer

Status: complete
Delivery: [Polyorama PR #47](https://github.com/robchristie/polyorama/pull/47)

## Outcome and scope

Deliver a maintained independent Cargo workspace in `consumers/record-desk`
using public Polyorama APIs for a useful record review/edit workflow on native
and browser. Deliver its demonstrated framework boundary repairs, documentation,
regression and runtime evidence, one fresh-context extension probe and reviewed
PR landing. No backend, image pipeline, publication, general application
framework, accessibility adapter or agent benchmark programme.

## Decisions and invariants

- Own record schema, validation, drafts, selection and transaction history in the
  consumer. Use stable record IDs and pane IDs, never image annotations.
- Use core `Workspace` as the only layout tree. The compatible layout-only dock
  output removes the image document/session from this composition path.
- Persist committed records and layout explicitly; drafts/history are transient.
  Reject malformed/unsupported state visibly and block overwriting it.
- Keep pane interfaces narrow: read views and output intents. Use shared tokens,
  measured recipes, native-control coverage and event-driven repaint.

## Acceptance and design contract

| Requirement | Evidence owner | State |
|---|---|---|
| Separate manifest/workspace/lockfile; no application dependencies | consumer checks and README | Complete |
| Search/filter/stable selection; validated Apply; cancel; one transaction; undo/redo | consumer model/UI tests | Complete |
| Save committed records/layout; restart/reload; visible failures | store tests and runtime journeys | Complete |
| Native and browser documented builds and actual execution | consumer runtime evidence | Complete |
| Physical keyboard/pointer; stable semantics and measured text | consumer smokes and UI tests | Complete |
| Ordinary/narrow/empty/error/selected/disabled/focus presentation | opened captures and report | Complete |
| Fresh-context extension with public source/docs | bounded probe report | Complete |
| Canonical verification, independent exact-head review, CI, landing | owning PR and landing comment | Complete for canonical product qualification; PR retains review and landing gates |

Representative task: find a synthetic record, review its detail, edit and Apply,
undo/redo, save, then resume after restart/reload. List/navigation comes first;
detail editing and Apply form the main content hierarchy; persistence/history
status and recovery stay visible. Reference the framework's analytical design
tokens and production action/property/text recipes, without decorative cards.
Ordinary layout shows linked list and detail. Narrow layout retains both panes
through a deliberate compact dock layout with local scrolling and reachable
primary actions. Required states: no matches, selected/filtered-out selection,
dirty draft, invalid edit, saved/unsaved, persistence error, unavailable undo/redo
and keyboard focus. No asynchronous loading or partial-data state applies.

## Calibration and sequence

Question: can public layout/presentation APIs support a non-image domain without
application implementation dependencies? Smallest probe: two dock panes, one
validated record transaction and native/browser startup. Evidence belongs in
`docs/record-desk-evidence.md` and ignored runtime artefacts. Exit when that slice
works with one canonical layout and no fabricated image state; reconsider only
if current public contracts obstruct the slice.

1. Select boundary and implement domain/store plus native/browser composition.
2. Exercise representative workflow, visual contract and bounded authoring probe;
   repair material gaps and wire separate workspace into canonical verification.
3. Reconcile docs/evidence/plan, complete verification, independent review and land.

## Current evidence

Base: `5d31a4790b3a212153a0b9faa2b52428ec88c25d` (clean `main`). The consumer
uses only core/UI; optional image rendering and layout-only resize remove the
demonstrated dependencies. Session's analytical defaults and four-image-camera
validation remain compatible because the non-image consumer does not need them.

The separate consumer checks pass: metadata, 26 model/store/UI tests, native and
WASM lint/build and browser packaging. Physical native/browser journeys pass
including filtering a stable selection, one Apply transaction, undo/redo,
persisted layout/records after restart/reload and visible storage failures.
Ordinary and 390-point narrow images have been opened and judged. The fresh
authoring probe added Reset filters with four passing UI tests using public
source/documentation. [Evidence and decisions](record-desk-evidence.md) own the
details. Full local `cargo xtask verify` passes, including preserved existing
applications, production startup, UI snapshots and both consumer journeys.
Qualification harnesses retain bounded idle observations and parsed native
snapshots, with regression coverage. Browser pointer targeting also requires
bounded, stable enabled geometry, covered by observer regressions and a slowed
physical journey. AccessKit parity tests cover clipped
targets, full records and
open/closed options at the runtime viewport sizes. The product state is complete;
the owning PR retains the exact reviewed head, required CI, squash revision and
cleanup evidence under the reviewed delivery procedure. Public GitHub-hosted CI
is retained.

First-presentation regressions also cover completely clipped toolbar actions:
visibility composition remains consumer-owned, keeps invalid allocation evidence
and requires principal chrome before host workflow readiness.
