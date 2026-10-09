# CI feedback time

Status: complete

Delivery: [PR #56](https://github.com/robchristie/polyorama/pull/56)

## Outcome and acceptance

Reduce feature-delivery CI latency towards a maximum of five minutes for cached
GitHub-hosted runs. Preserve the complete canonical verification surface and the
required `verify` check, including prose-only routing. Implement stable runner
toolchain selection, combined native application/example compilation and
parallel qualification. Profile changes require representative runtime evidence.

## Calibration

The smallest probe compares native build target selection using the pinned
Rust 1.99.0 toolchain, then exercises the parallel verifier on a fresh hosted
runner. The existing cached 1.98.1 run 37574216107 took 20m39s; it attributes
3m24s to workspace native release and another 1m20s to the example release.
Those older measurements guide partitioning but do not qualify Rust 1.99.0.
The successful 1.99.0 baseline is run 37769182622 on
`90c1d0baba78a0c68a488943f858763a36762cf2`; dependency recompilation makes its
30m28s job unsuitable as a warm comparison.

The owning PR retains exact run, cache, compiler and elapsed-span evidence.
Source and assurance invariants belong in verifier regressions; operating
guidance belongs in `docs/ci-cache.md`. Select a candidate only after the full
surface passes. Compare an unchanged fresh-run warm attempt and a representative
source change; report measured limits without turning a timing target into a
test-skipping rule.

## Delivery

| Phase | Owner | Status |
| --- | --- | --- |
| Preserve the canonical check inventory and split executable stages | Verifier | Complete |
| Normalise runner toolchains and give parallel jobs appropriate caches | Workflow | Complete |
| Probe combined native compilation and profile trade-offs | Calibration | Complete |
| Qualify the complete hosted surface and cached elapsed time | CI | Complete |
| Prepare the candidate and retain delivery evidence | Owning PR | Complete for product qualification; final review, merge and post-merge evidence belong to the PR |

The committed `d2b359e82eab876a313eed6c6985ea4c041a74fa` native-Lab stage passed
the batched release build, both existing native smokes and its interface journey
locally in 89.53s. This screens the partition; it is not hosted timing proof.
Hosted calibration run 37865870558 exposed missing `ripgrep` in the new native
UI setup after the icon interaction check passed. Add it to every UI lane and
retain the runtime-log scan. That failed stage cannot populate its archive;
successful lanes may populate their provisional PR-scoped dependency archives.


The partially warm repaired run 37866800165 passed checks (3m36s), native Lab
(3m55s), production (3m19s) and UI (2m55s); native-other and Record Desk were
still cold. Existing browser traversal raced slow input frames. Gallery's
failure retained repeated frame observations while the helper continued sending
Tab; Record Desk's failure retained five Tab keydowns followed by Enter on a
different focused node. A bounded local probe found no target-focus assertion
contradicting egui's current focused ID. The selected repair acknowledges each
Tab through a monotonic application diagnostic before another key, preserving
all focus/activation assertions and traversal limits. Both focused physical
browser journeys and the delayed-receipt regressions passed locally. Temporary
probe fields/logging were removed. Keep release profiles unchanged.

The changed-source warm run 37869437618 passed six complete lanes, with a
maximum lane duration of 4m04s. Native Lab failed before its window became
visible: the legacy smoke searched once after a fixed five-second sleep while
its runtime log still showed graphics startup. The selected repair waits up to
30 seconds for the owned application's visible window, fails if its process
exits, and keeps all existing interaction/persistence assertions. Gallery uses
the same helper. Readiness regressions cover delayed visibility, timeout and
application exit.

Run 37870185539 exposed the same queued-Tab condition in the retained icon
workflow's separate fixed-delay traversal. Icon, navigation and Record Desk
now share the receipt-aware helper while retaining their respective 40, 45
and 24 action limits and 60/60/50 ms settling intervals. The helper observes
the final allowed action. The focused icon journey and all caller-bound
regressions passed locally.

## Qualified outcome

The complete changed-source warm workflow on
`3049309088429a224a2e58d9cf12986a682e6651` passed in **4m34s**. An unchanged
fresh-runner repeat passed in **4m47s**. Both are
[run 37870954298](https://github.com/robchristie/polyorama/actions/runs/37870954298),
attempts 1 and 2. The metric spans `run_started_at` through terminal `verify`
completion and includes scheduling, setup, qualification, uploads and post
steps. The documentation job is deliberately skipped on this full route.
All seven archives restored exactly; all four observed interface journeys
matched their clean checked-out revision. The worst observed complete cached
workflow in this initial two-attempt cohort is below five minutes. Hosted queue/service variance and future
workload changes remain outside this small qualification cohort.

The owning PR retains independent exact-head review, final candidate CI, squash
identity, separate post-merge CI and main-scope warm restoration. This completed
product state does not assert that those delivery gates have already completed.

## Reopened performance qualification

Final candidate `da96f649867815556b46e22c31b7033770a6be40` received an
independent code-review PASS and passed all checks in run 37872275789, but the
complete warm workflow took **5m09s**. It is held for performance repair. The
browser lane took 4m43s; its four Gallery smokes account for approximately
47 seconds. Add an eighth Gallery browser lane, retaining the public build-web
behaviour and moving those smokes intact. Reuse the exact UI Gallery builder
and its compatible archive; UI remains the sole successful-main writer.


The unchanged eight-stage repeat retained a Results target receipt whose row
height changed from 21 to 20 points after the harness moved the pointer into
the accepted target. The frozen-geometry assertion correctly failed before
clicking. Pointer positioning now participates in the existing settling
window; target identity and all geometry remain exact at final dispatch, with
one physical click and the unchanged 200 ms stability/3500 ms budget.
Regressions cover hover-induced pose changes and slow pointer preparation.

## Final eight-stage qualification

The repaired eight-stage source
`edd16a292349713e70d9a9cff6925761eb35370e` passed the complete changed-source
warm workflow in **4m28s** and its unchanged fresh-runner repeat in **4m43s**
([run 37874654782](https://github.com/robchristie/polyorama/actions/runs/37874654782),
attempts 1 and 2). Both include scheduling, setup, all eight qualification
stages, uploads/post steps and terminal `verify`. All required checks passed.
The Gallery browser lane reused the exact compatible UI archive, with no
external dependency compilation and no additional cache writer.

Retain the eight-stage arrangement, receipt-aware physical input, pointer
settling before geometry freezing and bounded native readiness. Release
profiles and the complete assurance surface remain unchanged. Final reviewed
head CI, live merge checks, separate merge-revision CI and main-scope warm
restoration remain attributable delivery evidence owned by the PR.
