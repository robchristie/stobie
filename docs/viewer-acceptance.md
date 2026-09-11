# Viewer acceptance: diagnostic candidate

No technically accepted viewing configuration has been selected. The frozen
512-tile/D6 development bracket failed RGB16 quality, and the RGB8 regression
also failed. This change implements exact source-derived display masks and
bounded runtime diagnostics, retaining the established classic configuration
and all prior campaign outcomes. Owner review, canonical verification, landing
and the predeclared merged viewing qualification remain required.

## Configuration and evidence

| Predicate | Observed outcome | Evidence |
| --- | --- | --- |
| Source rights and original arrays | Existing approved inputs; identities retained | [Full-scene protocol](viewer-acceptance-full-scenes.md) |
| RGB16 4/8/12 total spatial bpp | All development rates fail frozen per-band RMSE <=3 or p99 <=12 gates | [Quality](viewer-acceptance-quality-results.json) |
| PAN16 4 bpp control | Frozen and full-scene numerical gates pass | [Full scenes](viewer-acceptance-full-scenes-results.json) |
| RGB8 4 bpp regression | Tok quality fails | [Encoding decision](viewer-acceptance-encoding-decision.md) |
| Independent encoder | Bounded comparison complete; no eligible encoding-policy selection | [Independent results](viewer-acceptance-independent-results.json) |
| Exact validity | Original-source masks, per-band ALL/ANY reductions, identity binding and explicit required-data failure | [Mask contract](viewer-acceptance-masks.md) |
| Browser pressure | Sole repair reserves required bins plus masks; oversized working sets fail explicitly within unchanged budget | [Repaired browser results](viewer-acceptance-browser-masks-repaired-completion-results.md) |
| Native resource cohort | Five repaired starts below unchanged ceiling; two detail failures and one missing external final bracket | [Repaired cohort](viewer-acceptance-native-repaired-cohort-results.md) |
| Completion diagnosis | Publication-to-UI waiting observed through the shared receipt path | [Completion results](viewer-acceptance-completion-results.md) |
| Scheduling candidate | Five-pair screen rejected; production cadence restored | [Scheduling results](viewer-acceptance-scheduling-results.md) |
| Native inspection | Actual captures opened; very dark views and intermediate frames recorded | [Inspection](viewer-acceptance-native-inspection.md) |
| Merged hardware viewing/recovery | Prepared, not yet measured | [Fixed qualification protocol](viewer-acceptance-merged-qualification.md) |

Total bpp is per spatial pixel, not component sample. The rate bracket uses
irreversible 9/7, one layer, 64-square blocks and the existing no-MCT colour
policy. Arrays retain their precision; no U8 stretch is baked into encoded data.
Mansfield is development, Boca reserved diagnostic validation and Tok regression;
all are previously used acquisitions, not untouched validation.

The historical 278,794,240-byte observation remains above the unchanged
243,269,632-byte ceiling. Lower current observations do not repair or relabel it.
Logical cache/GPU accounting is not an additive process-memory model. Native
current RSS, high-water RSS, bounded smaps/rollup, allocation and lifetime
observations retain separate meanings. AutoNoVsync remains rejected and the
worker's existing repaint request remains in use. Tok's historical absolute
detail failures are unresolved by another product's successful observation.

## Implementation boundary

Representation sidecars carry validity; this does not invent JPEG 2000 or JPIP
syntax or general alpha support. Framework pixel/upload APIs retain compatibility
through additive frame types. Polyorama's generic framework remains codec-neutral.
The runtime reserves bounded active compressed working sets, rejects stale scopes
and releases reservations on terminal paths. UI-owned publication/upload,
priorities, cancellation, stale-generation rejection and three-frame settlement
remain authoritative. Browser timing retains clock uncertainty and unavailable
cross-realm intervals rather than clipping inversions.

These are diagnostic results, not human acceptance, analytical/ML suitability,
independent reconstruction equivalence or a production speed claim. Rejected
assets remain diagnostic only. The workspace campaign owns the eventual terminal
configuration/rejection record and merged integration evidence.
