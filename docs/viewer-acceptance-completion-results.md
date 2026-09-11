# Authored native completion diagnostic

One fresh application start completed: exit **0**, audit **0**, **56.773 s** within
660 s, no warmups or retries. All 42 phases and ten authored release/revisit
cycles completed; 106 allocation/phase markers passed coverage, with no app
errors. This is completion-path diagnosis, not production speed, quality or
resource acceptance. Main owns candidate selection and commits.

Runtime `fe82f85984fba5c75d7a5a8f10b2d914b773a838`, tree
`6d3e409f9954f8bf12cf0ebc2b3898b3574470a0`; build record and harness HEAD
`e1e5ce738d7fb0452cd2e9d2ad8c30486ea6988d`. Retained binary SHA-256
`2603bf16880300e6375c6dfa97b7ad2e04086c18e8450ccf020afaefa38f4405`; authored workload SHA-256
`3c9d5fe71f37ea02c58d9eb9ebd6ac0cd4ea3e47d0ee1bc6b391b7455bae9705`. All 84 expected identity
entries passed before launch. Exact commands, input/source hashes and evidence
paths are in [the machine-readable report](viewer-acceptance-completion-results.json).

PID **3431750**, kernel start ticks **545128167**, launched
`2026-09-11T07:15:26.444927+00:00`. A fresh metadata-only service served the exact frozen
catalogue on port 8195. Its authored path has no protected pixels or payloads.
The owned free Xvfb display **:0** was queried as **1440×900×24**. Two prelaunch
service setup failures are retained: missing compiled-in xkbcomp path, then an
unsupported bwrap option. Neither started the app; those service exit codes
were not captured. Final setup supplied existing xkbcomp in an Xvfb-only mount
namespace, without acquisition or host installation.

The app reported **RTX 3090 / NVIDIA 610.43.03 / Vulkan**. Four observations of
its own `/proc` mappings identified 40 executable/library objects: 21 matched
expected hashes, 19 additional objects were hashed, zero mismatches. These
include the actual Vulkan loader and NVIDIA driver libraries. This is sampled,
non-atomic loading evidence, not a complete loading history. glibc `mallinfo2`
2.43 was available at every marker; no allocator interposition/change occurred.
Both owned services and the application stopped; their endpoints refuse
connections. An inert X11 socket pathname remains; no unrelated service was
inspected or stopped.

Across **2,119 requests**, cumulative worker execution was **2.547 ms**
(mean **0.001202 ms**), publication-to-receipt **53,077.481 ms**
(mean **25.048 ms**), and receipt-to-drain **1.605 ms**. The instrument recorded
**2,099 same-frame next dispatches**, with mean drain-to-dispatch **0.294 ms**.
This supports waiting after publication and before UI receipt with decode
removed. The source already calls `request_repaint` after publishing.

| Retained interval | Samples | Median ms | p95 ms |
| --- | ---: | ---: | ---: |
| Worker execution | 957 | 0.000920 | 0.002290 |
| Publication → UI drain | 957 | 25.411484 | 29.354393 |
| Wakeup request → UI drain | 957 | 25.411364 | 29.353413 |
| Publication → next dispatch boundary, same frame | 745 | 25.707877 | 30.298293 |
| UI drain boundary → upload start | 765 | 0.189098 | 0.613736 |
| Upload start → finish | 775 | 0.043832 | 0.121085 |
| UI drain boundary → render submission | 765 | 1.239403 | 2.202284 |

There were no invalid cumulative native pacing samples or missing wakeup stamps
among the retained samples. Publication precedes the recorded wakeup request by
median 0.000131 ms. Notification arrival, event-loop handling, redraw gating and
presentation are not separately measured: no particular internal cause is
assigned to the waiting interval. Upload finish means admission/CPU ownership
release; render submission means CPU preparation/callback enqueue, not GPU
completion, presentation or scanout.

The 128-event and 256-boundary rings overwrote **4,110** and **10,528** records.
Deduplicating stage/final snapshots recovers 1,914/4,238 event records and
4,001/10,784 boundary records. Distribution samples are incomplete; cumulative
counters cover all requests. Boundary pairs require contiguous retained indices
and the same frame; no missing-ring edge is inferred. Native pacing and Linux
marker clocks are not aligned by an estimated offset.

Observed peak outstanding reservations were **one**, with peak accounted
decoded bytes **1,048,576**, against the unchanged **16,777,216-byte** limit.
765 matched upload samples confirm U16 payload geometry. Representative actual
sample bytes: scalar 64×64 **8,192**, scalar 512×512 **524,288**, RGB 64×64
**24,576**. Their observed reservations were respectively 16,384–16,900,
1,048,576 and 49,152–50,700 bytes; conservative geometry may exceed twice actual
samples. RGB8 also uses U16 storage. Reservations were still charged at drain,
then transferred to upload accounting and released at acknowledgement. Time
blocked specifically by byte pressure is unavailable; sampled zero decoded
accounting does not mean no intervening payload ownership.

Epoch-aware totals show **2,119 completions and uploads**, zero cancellations,
stale results, worker aborts or app errors. These settled scripted actions do
not qualify adversarial cancellation/stale behaviour.

Readiness remains distinct from three-frame settlement. First overview became
ready and settled at **3,357.270 ms** after 147 completions; the frame-count gate
was already satisfied. In 22 phases with no new requests, readiness preceded
settlement: warm GPU revisit was ready at **1.151 ms**, settled at **50.825 ms**.
Readiness can describe an interim scroll demand set. The settlement predicate
was unchanged; no shorter latency is substituted for it.

**One proposed candidate, report only:** service the existing completion wake
notification in a bounded UI/event-loop turn before redraw/presentation gating,
then admit at most one next request through the existing priority scheduler.
Process pending intents/cancellations first and require the latest reconciled
generation; defer if reconciliation is pending. Retain reservations until actual
completion/cancellation acknowledgement, reject stale tokens before admission
and upload, and retain all decoded/upload accounting and limits. Keep one
execution worker, the one-job/two-event channels, bounded per-turn work and
UI-owned runtime/upload/render. No prefetch, extra threads, unbounded queues or
weakened limits. Preserve interactive priorities and three-frame settlement.
Main must select and separately qualify this candidate; its benefit is unmeasured.
The **AutoNoVsync rejection remains**; another repaint request is not the candidate.

All authored artefacts are under
`/nvme/development/emuella/.build-targets/viewer-acceptance/native-frozen-v1/authored-probe-01`.
The JSON report links a SHA-256 evidence manifest, compact derived analysis,
owned-process identities, source hashes and preserved failures. No real-scene
cohort, protected-asset read/copy, application code edit, downstream agent,
commit or push occurred. Full verification would add unauthorised starts and
remains with main; this operation ran the frozen coverage audit and checked the
report derivation and identities.
