# Text hierarchy and attention UI

Status: complete

Delivery: [Terminal coordination PR #31](https://github.com/robchristie/polyorama/pull/31), following [library #30](https://github.com/robchristie/polyorama/pull/30) and [Bokkie #17](https://github.com/robchristie/bokkie/pull/17).

## Delivered outcome

Polyorama provides distinct regular/semibold faces, dense and reading semantic
typography, content-sized labels, explicit fixed slots and observable layout
failures. Bokkie consumes the merged contract, prioritises operator actions,
derives virtual row geometry from typography/density, and renders complete
selectable evidence in a scrollable reader.

This is the single cross-repository coordination record. The repositories own
their detailed source, calibration and qualification evidence. Both are public
source; only synthetic fixture data was used. No release, deployment, operator
database or gardener runtime operation is included.

## Delivery checkpoint

| Increment | Owner revision | Consumer revision | Aggregate result | Status | Evidence |
| --- | --- | --- | --- | --- | --- |
| Library typography/layout/diagnostics | `d10b6864ef278fe98fa927111f97d6d142344aab` | — | Full canonical verification; four visible hierarchy probes; independent PASS; PR CI passed; reviewed/landed trees equal | Landed | [Polyorama #30](https://github.com/robchristie/polyorama/pull/30), [owner evidence](text-hierarchy-evidence/README.md) |
| Attention composition and evidence | `d10b6864ef278fe98fa927111f97d6d142344aab` | `35a5a0df9eee13ea39a41f216446e120e071e858` | Locked backend/UI checks; 42 UI tests; seven browser journeys; native smoke; independent PASS; all PR CI passed; reviewed/landed trees equal | Landed | [Bokkie #17](https://github.com/robchristie/bokkie/pull/17), [consumer evidence](https://github.com/robchristie/bokkie/blob/35a5a0df9eee13ea39a41f216446e120e071e858/docs/text-hierarchy-evidence/README.md) |

## Acceptance and calibration decisions

- Actual font faces, sizes, line heights and emphasis distinguish roles. Reading
  uses 21-point application titles, 18-point pane titles, 15-point sections,
  14-point body and 12.5-point metadata. Dense defaults retain 13-point body.
  Native font choices match; exact native line height/emphasis uses `rich_text`.
- A one-line content label with a two-line maximum consumes its measured height;
  fixed-slot labels deliberately reserve lines. Tests cover 100%, 125% and 150%.
- Invalid requests, including 24-line bounded labels, paint a diagnostic
  fallback, emit a typed layout error and fail the audit. Independent attempted,
  successful and failed component inventory survives observation filtering.
  Required semantic content has explicit regression assertions.
- Bokkie places current legal actions before technical disclosures, preserves
  exact confirmation provenance and stale-state guards, omits absent metadata
  noise and distinguishes approval attention from failure emphasis.
- Virtual row recipes pass allocated/painted bounds checks at 100% and 150% in
  both densities. The selected evidence reader retains a 12-line minimum
  viewport after rejecting a collapsing nested-scroll probe.
- Real browser input selects the synthetic completed obligation, opens Raw
  durable evidence and scrolls until `BOKKIE_EVIDENCE_TAIL_7F39`, after paragraph
  119, is present in the painted galley's visible rows and inspected screenshot.
- Library calibration rejected black headless captures despite valid semantic
  snapshots. Four headful captures were inspected and accepted; the five frozen
  baselines preserve semantic identity/name/role/actions/enabled state.

## Qualification limits and lifecycle

Ordinary native labels are outside structural text measurement. Bokkie's
browser keyboard-search entry remains unqualified; the reader journey uses
physical virtual-ledger scrolling. Native evidence proves selection,
confirmation inspection and keyboard focus, with its durable mutation submitted
through a conditional harness HTTP request. Browser evidence separately proves
UI submission. These results do not establish screen-reader certification,
physical-GPU performance or deployment.

Native/browser qualification was repeated successfully from merged Bokkie
`35a5a0df9eee13ea39a41f216446e120e071e858`, consuming merged Polyorama
`d10b6864ef278fe98fa927111f97d6d142344aab`. The qualification report identifies
the merged source revision, seven passing browser journeys and native smoke.

The task worktrees, implementation/coordination branches and generated probes
are ephemeral campaign resources. Final exact-head review, PR and post-merge
CI, tree equality, default-branch synchronisation and four-state Git/scratch
cleanup belong to each pull request's landing evidence. Shared build caches,
Node dependencies and the existing Linux UI sysroot are preserved.
