# Shared application inspection and exercise

Status: complete
Delivery: [PR #48](https://github.com/robchristie/polyorama/pull/48)

## Outcome and boundaries

Deliver one public, versioned Rust integration surface and machine-readable
client for Record Desk and Analytical Workspace Lab on native and browser hosts.
Keep application rules and validated mutation routes application-owned, retain
completed observations without inspection repaint, preserve physical regression
journeys and the independent consumer's public-only, non-image composition.
No remote service, MCP server, arbitrary evaluation, raw model access,
unrestricted setter, accessibility adapter or exhaustive migration.

## Contract and integration

The UI crate owns semantic/action presentation contracts. Extend that owner with
retained observation identity, bounded selectors, typed action descriptions,
request correlation and receipts. Applications supply current availability,
validated action bindings and bounded collection metadata. Native local transport
and the browser execution context share serialisation and operation meanings.
The reusable client owns deadline/cancellation waits, physical synchronisation
and host capture provenance. Inspection observes rendered nodes only.

## Calibration

Question: can existing completed semantics and typed actions support a useful
shared contract without a competing state/mutation architecture?
Smallest probe: inspect one current control, discover its capability, invoke
through the normal validated route and observe the outcome in both applications.
Evidence owner: `docs/application-interface-evidence.md` and ignored runtime
artefacts. Exit: both probes pass with idle inspection and unambiguous identity;
otherwise reconsider the service boundary before expanding.

## Increments and acceptance

| Increment | Dependency | Acceptance | State |
|---|---|---|---|
| Contract and representative slice | Existing core/UI/application contracts | Both applications query/discover/invoke/observe through shared types | Complete |
| Hosts and reusable client | Stable slice | Same protocol, explicit opt-in, lifecycle, bounded queues/history; common query/invoke/wait/physical/capture | Complete |
| Real journeys and migration | Both adapters | Record physical draft → semantic Apply once → undo/redo; invalid/unavailable no mutation; corresponding physical workflow. Lab Fit/display result, pane/domain and bounded virtualised query. Both hosts | Complete |
| Qualification and delivery | Complete integrations | Contract regressions, opened captures, fresh-context usability, full verify; exact-head review, CI, merge and post-merge CI owned by the delivery PR | Complete for product qualification |

Consequential tests cover negotiation, bounded empty/ambiguous/repeated queries,
stale/disappearing targets/restarts, current availability and arguments,
duplicates/uncertain completion, timeout/cancel/missing observation, moving or
clipped physical targets, idle read-only behaviour, cross-host parity,
virtualisation bounds and honest partial capture diagnostics.

## Qualification and closeout

Base: `57e5d906fe7f40521049560d20605d47cbc6d724` (PR #47).
Existing evidence informs design but cannot qualify changed code. Canonical
verification remains `cargo xtask verify`, GitHub-hosted for this public repo.
Independent architecture advice is read-only and separate from final review.
The [evidence report](application-interface-evidence.md) retains calibration and
fresh-context usability; all four representative host slices passed. The fresh
agent completed both native tasks using only the public guide/common CLI and
exposed a visibility/queued-receipt documentation gap, now repaired. Canonical
verification and all four journeys passed; the checked-in manifest binds clean
source checkpoint `60c286a8fe7b5e81dbbc69d3cd86f114cce131c5` to actual artefacts,
receipts, observations and captures. The product implementation and qualification
are complete, including review repairs for polygon capability availability and
action-specific Record Desk draft/history guards, and verification repairs for
native pointer placement and fully clipped virtual rows. The delivery PR records
independent exact-head review, required and post-merge CI, final merge identity
and cleanup; this status does not assert that the PR has already merged.
