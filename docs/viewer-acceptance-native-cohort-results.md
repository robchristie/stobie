# Frozen native resource cohort results

Exactly **five fresh starts**, no warmups, retries or replacements, completed the
fixed cohort. Cohort exit **4** is retained. Every application and harness exited
**0**, each completed **40 phases / ten release/revisit cycles / 102 allocation
markers**, and all ten scheduled captures exited **0**. Application exit 0 follows
from the frozen harness recording every nonzero wait result as a trace failure;
all five completed traces have no failures. No resource or human acceptance.

Protocol **e38cacdfea09d0bd3dc2b871e5c20ec66007cc90** ran from the clean detached
`/nvme/development/emuella/.build-targets/viewer-acceptance/native-cohort-protocol-e38cacd`.
Runtime **fe82f85984fba5c75d7a5a8f10b2d914b773a838**, binary SHA-256
**2603bf16880300e6375c6dfa97b7ad2e04086c18e8450ccf020afaefa38f4405**.
The registered `native-cohort-grant.json` records main's explicit authority,
`native-cohort-execution` ownership and **2026-09-11T07:31:48.720365+00:00**.
Preflight started no app. Existing xkbcomp was supplied at its missing compiled-in
path through a temporary `/usr/bin` mount overlay around the runner; no acquisition,
host installation, source or binary changes. Phase/process deadlines stayed
**60/660 seconds**, capture offsets **2/5 seconds**.

Actual display **:177, 1440×900×24**, service port **8194**; every app reported
**NVIDIA GeForce RTX 3090 / NVIDIA 610.43.03 / Vulkan**. Each run bound 42 mapped
files by hash, including eight frozen graphics matches; three NVIDIA device nodes
were unhashable. This is sampled loading coverage. glibc **mallinfo2 2.43** was
available at every marker; no preload or allocator change. The exact environment,
namespace command, library evidence and owned PID/start-time identities are linked
in [the JSON report](viewer-acceptance-native-cohort-results.json).

All bytes below are absolute. Historical **278,794,240 > 243,269,632** remains
unchanged; this cohort does not replace that observation.

| Run | App HWM | Sampled descendant RSS peak | Observed per-PID HWM sum | Benchmark exit |
| --- | ---: | ---: | ---: | ---: |
| native-01 | 189,370,368 | 191,152,128 | 198,373,376 | 4 |
| native-02 | 185,962,496 | 198,832,128 | 198,832,128 | 0 |
| native-03 | 186,204,160 | 187,981,824 | 196,198,400 | 0 |
| native-04 | 190,152,704 | 203,018,240 | 203,018,240 | 0 |
| native-05 | 189,403,136 | 191,201,280 | 200,437,760 | 0 |

Every RSS/HWM observation is below **243,269,632 bytes**. The app HWM snapshot
precedes some later lifetime peaks; the sampler's HWM sum remains separately
visible. All five benchmark traces were **admitted**; run 1 was **unqualified**
(target detail **155.093 ms > 133.58994625 ms**); runs 2–5 qualified mechanically.

| Run | Graphics current RSS | Overview HWM | Cycle 1 current | Cycle 10 current | Marker growth |
| --- | ---: | ---: | ---: | ---: | ---: |
| native-01 | 160,874,496 | 175,554,560 | 188,616,704 | 189,370,368 | +753,664 |
| native-02 | 161,046,528 | 175,497,216 | 185,110,528 | 185,962,496 | +851,968 |
| native-03 | 160,935,936 | 175,525,888 | 185,339,904 | 186,204,160 | +864,256 |
| native-04 | 160,968,704 | 175,521,792 | 189,366,272 | 190,152,704 | +786,432 |
| native-05 | 160,813,056 | 175,620,096 | 188,641,280 | 189,403,136 | +761,856 |

Startup current RSS was **4,698,112–4,857,856 bytes**. The direct current-RSS
marker differences are supplementary. Required external cycle-10 after-brackets
are **missing in runs 1, 2 and 5**: no same-identity read wholly after the marker
within 1,500 ms. Runs 3/4 retain ten brackets and report **−11,702,272 /
−39,247,872 bytes**; final samples follow the final marker and may include lifecycle
release during export/teardown. They do not establish a settled-workload plateau.

| Run | Cycle-10 arena in use | Free retained arena | Allocator mmap | In-use growth, cycles 1→10 |
| --- | ---: | ---: | ---: | ---: |
| native-01 | 28,614,096 | 8,679,984 | 15,040,912 | +1,026,096 |
| native-02 | 28,655,280 | 4,739,408 | 15,040,912 | +1,051,216 |
| native-03 | 28,650,224 | 4,916,496 | 15,040,912 | +1,050,640 |
| native-04 | 28,638,304 | 8,733,600 | 15,040,912 | +1,052,448 |
| native-05 | 28,622,688 | 8,704,160 | 15,040,976 | +1,032,560 |

Retained diagnostic stages grow **30→39**. All ten release markers per run show
zero runtime entries, decoded payloads, reservations and upload/sample/validity
capacity; each revisit restores 32 resident entries with zero outstanding payload
ownership. Cumulative display release is **416 items / 27,288,064 logical bytes**,
including the ordinary release. Rings retain 128 event / 256 trace slots; trace
overwrite counts are **2,173 / 2,167 / 2,167 / 2,171 / 2,168**.

Last smaps RSS values are **189,370,368 / 185,962,496 / 173,740,032 /
150,179,840 / 189,329,408 bytes**. Private-clean spans **99,057,664–106,569,728**,
private-dirty **45,490,176–79,511,552**, anonymous **45,490,176–55,574,528**;
swap is zero. All phase high waters, cycle markers/brackets and intermediate
smaps remain in the approved evidence. **Smaps, allocator, logical resources and
RSS overlap: never add or subtract them.** Tcache/foreign allocations and physical
GPU allocation remain incompletely attributed. No cause-supported resource repair
is proposed or implemented.

| Run | First useful | Overview | Detail | Thumbnails | GPU revisit | Compressed revisit |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| native-01 | 74.112 | 720.957 | 155.093 | 559.480 | 60.672 | 704.827 |
| native-02 | 75.832 | 740.227 | 128.293 | 597.917 | 63.831 | 708.103 |
| native-03 | 74.042 | 718.443 | 110.269 | 610.766 | 49.533 | 637.474 |
| native-04 | 82.841 | 714.299 | 129.467 | 556.333 | 49.479 | 619.797 |
| native-05 | 79.295 | 720.014 | 110.022 | 561.694 | 53.768 | 689.690 |

Latencies are milliseconds, without overhead subtraction. Every run received
**2,691,804 TCP bytes**, sent **52,414**, delivered **1,963,454 JPP + 30,208 descriptor
bytes**, and recorded **0 original-storage bytes**. Peak decoded/GPU/compressed/
descriptor/codec workspace bytes were **1,572,864 / 17,026,200 / 2,566,649 /
1,083,232 / 6,489,012**, within unchanged limits **16/64/64/16/64 MiB**; concurrency
was **1**. First service observation is uncontrolled; later runs share that service.
Another worker could compile concurrently; background load was not isolated or
measured over time. No speed comparison or scheduling-candidate claim.

All protected evidence stays beneath:

`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-native-cohort-mansfield-01`

| Run | Scheduled screenshot paths relative to that group |
| --- | --- |
| native-01 | `native-01-captures/capture-02s.png`, `native-01-captures/capture-05s.png` |
| native-02 | `native-02-captures/capture-02s.png`, `native-02-captures/capture-05s.png` |
| native-03 | `native-03-captures/capture-02s.png`, `native-03-captures/capture-05s.png` |
| native-04 | `native-04-captures/capture-02s.png`, `native-04-captures/capture-05s.png` |
| native-05 | `native-05-captures/capture-02s.png`, `native-05-captures/capture-05s.png` |

All PNGs are 1440×900; capture durations **103.59–304.78 ms**, lateness
**1.02–20.00 ms**. Full absolute paths, SHA-256s and timings are in JSON.
**Main must inspect the images**; this owner made no visual/content/mask finding
or human acceptance. Image existence is insufficient. Quality rejection and the
separate seven-event pressure/recovery requirement remain unchanged.

Evidence IDs: `execution.json` SHA-256
`f6bf64d5b2d76e3e0abc291de581e1f54b4e9ac4c2b5fbd5eb1930d34619eb40`;
`evidence-manifest.json` SHA-256
`e4082504d752c1794895ca63e7154e892973ae4495cc077d9d3d7219cccd2874`.
The manifest binds 176 retained files. JSON links all five trace/summary identities
and `derived-resource-analysis.json`. Before/after representation inventories match.

Owned native identities are absent. Service exited **−15**, Xvfb **0**;
service connection refused, display socket/lock absent. Logs and worktree retained;
no explicit deletion, commits, pushes or agents. Only the two results files were
written in the main worktree; concurrent edits preserved. Report derivations,
identities, JSON and whitespace were checked. Full verification would introduce
extra starts and remains outside this exactly-five operation.
