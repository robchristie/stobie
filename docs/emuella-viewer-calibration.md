# Composed viewer calibration

The viewer's checked-in `qualification-workload.json` drives the same native
and browser application actions. It requires a 43,008-wide parent first in the
catalogue, eight additional parent images, 10,000 virtualised detections, at least
five bookmarks, clustered and scattered detail, gallery scrolling, display stretch
and simultaneous distinct images. The complete first overview comprises 121
independent regions of at most 64 source tiles each; visible gallery demand adds
further regions. Source pixels remain native unsigned 8–16-bit samples until GPU
display mapping. Detection opening reads additional parent dependencies; it does
not load an independently encoded chip.

The observation phase first asks whether the complete composed workload finishes
within the established resource budgets on the selected local GPU. The smallest
representative probe includes the whole large overview and all nine images.
Polyorama owns application phases, presentation preparation and worker/resource
instrumentation. The codec owns reconstruction work and quality evidence. A probe
is retained or rejected from its actual result; failed directories are never reused.
The exit condition is five independent successful native and browser baselines,
a coordinator-selected threshold file frozen before final qualification, and final
runs against the merged codec and identified immutable input representations.
Reused preparations retain their original encoder identities and are hash-checked;
new source integration has its own preparation qualification.

## Run and retain evidence

Install the locked browser tooling with `npm ci`, build the release native viewer
and complete static root as described in the app README, and start the local
representation service with the large image first. Use an output path beneath
registered campaign scratch that does not yet exist:

```sh
python3 tools/viewer-composed-journey.py \
  --mode native --output "$SCRATCH/native-01" --url http://127.0.0.1:8124 \
  --native-bin "$NATIVE_VIEWER" --web-root "$WEB_ROOT" \
  --codec-repo "$CODEC_CHECKOUT" --benchmark-repo "$BENCHMARK_CHECKOUT" \
  --server-cache-state uncontrolled-first-observation
```

Repeat with `--mode browser` for actual Chromium/WebGPU and a real Web Worker.
The harness records the actual adapter; selecting Vulkan flags does not establish
hardware execution. Use `warm-server` only after the prior complete service journey.
A fresh application creates empty compressed, decoded and GPU state. Neither case
conditions physical original storage, the OS page cache or an NFS server cache:
those states are explicitly uncontrolled. The `warm-compressed` phase replaces
only the settled app runtime and GPU renderer while retaining the executor/cache;
the `warm-gpu-revisit` and bookmark phases retain current GPU resources. Resetting
an unsettled executor is rejected so cancellation reservations cannot disappear.

Each phase has a predeclared 60-second failure deadline, distinct from acceptance
latency limits. The outer process has a 660-second deadline. Three complete frames
allow egui's deferred scroll to establish the new demand before settlement. Named
latencies end when all distinct desired regions are GPU resident, or when the
first primary-view region becomes resident. These are CPU-observed rendering boundaries;
GPU completion timing, physical GPU allocation overhead and scan-out are unavailable.
The contract field `first_useful_ms` measures first **primary region** residency;
it may represent only one of the 121 overview chunks. It is not complete-overview
latency or a claim that the whole large image is already useful. Always report
`whole_overview_ms` alongside it; that boundary includes all primary overview and
current gallery demands.

The exporter writes the benchmark-owned `composed_journey_trace/1` format and binds
full source revisions, tracked source diffs, native/static build hashes, immutable
representation identities, workload bytes, raw snapshots and evidence hashes.
Each event identifies its actual parent source and consumer. The validator CLI is:

```sh
emuella-benchmark journey "$SCRATCH/native-01/composed-trace.json"
```

Exit 4 with admission and the calibration-only reason is expected before freeze.
For final qualification pass the already frozen file through `--thresholds`, then
supply that same file to the benchmark CLI. The exporter never selects or freezes
limits from the run being assessed.

## Boundaries and limits

Normal limits remain 64 MiB compressed payload, 16 MiB descriptor/index metadata,
16 MiB decoded reservations, 64 MiB logical GPU textures, 64 MiB codec workspace,
one worker and at most 64 source tiles per regional request. The codec workspace
observation is its maximum deterministic plan requirement. Process RSS includes
allocator/library/driver overhead and is measured separately. Linux exposes
`VmHWM` for the complete native process; a browser worker exposes its actual WASM
linear-memory byte length. The external runner samples Linux process descendants every 50 ms, excludes its
Node harness, and records the sum of current `VmRSS` plus the maximum observed
`VmHWM` for each PID, retained after process exit. The latter sum includes peaks
that may occur at different times; shared mappings can be counted repeatedly.
Processes that start and exit between samples may be absent. These are explicitly
a sampled sum and a high-water sum for observed processes, not a continuous true
process-group peak. Physical GPU allocations remain unavailable. WASM linear
memory does not substitute for these process observations.

The loopback counting proxy observes application TCP bytes including HTTP headers,
static assets and retries, excluding TCP/IP framing. Service counters separately
report positioned payload and descriptor reads and whole-process read observations.
Viewer operations do not reopen the original source; original preparation costs
belong to the separate preparation record. Returned native pixel bytes are not
original storage read bytes. Physical NFS cache state and physical traffic cannot
be inferred from a process restart or logical read counters.

The application retains 128 work events and 64 decoded checksum records; cumulative
overwrite counts explicitly mark truncation. Native snapshots are captured at phase
boundaries; browser snapshots are additionally sampled every 100 ms. Their union
is useful bounded evidence, never a claim that the full underlying event stream
was retained. Exact code blocks and entropy coefficients, actual inverse-transform
work counters, native output samples and deterministic workspace are separate
observations. Failed decodes do not increment completed synthesis counters.

## Threshold selection

Use `tools/viewer-calibration-summary.py` with five trace paths for one runtime.
It reports every observation, nearest-rank p95 (the maximum for five runs), and a
proposal with a predeclared 25% allowance, rounded upwards to 10 ms or 1 MiB.
The allowance covers the measured local host scheduling variation and modest
instrumentation overhead for this fixed local proof. It is not a browser, network
or remote-storage service-level guarantee. The coordinator must review measured
variance and the interaction requirements, then freeze the selected limits before
final runs. A later failure remains a failure; thresholds are not expanded from
that failed final run. Runtime-specific latency limits share the same workload and
resource budgets.

The selected encoded profile has one genuine quality layer and resolution
progression. Resolution discards are not extra quality layers. The codec-owned
[quality calibration](https://github.com/emuella/emuella-j2k/blob/3afcfabb24282645c3e101ab3495810d28212dfd/docs/ht-quality-calibration.md) compares authored small bright objects, faint structure and
crossing edges, including the measured storage/work cost of an additional genuine
quality set. The app does not claim new standards conformance or quality solely
from a screenshot. Numerical reconstruction correctness and opened visual evidence
remain separate acceptance observations.

## Recovery and appearance probes

`--mode recovery` runs a separate workload with its own workload identity. It
interrupts a real JPP response after a nonempty strict body prefix, waits until
outstanding work stops, then retries. It separately destroys new connections,
restores transport and retries. This models transport reconnection; it does not
claim the service process restarted. Another fresh client delays one actual Worker
completion message, changes image, confirms the reservation remains charged,
then delivers that original payload and observes stale rejection. Pixels and
request tokens are not fabricated. A final fresh client aborts a delayed actual
HTTP response, observes worker acknowledgement, and exercises representation/GPU
eviction under 1/4/16 MiB compressed/decoded/GPU limits. The raw record distinguishes
these interventions and their actual events. It does not infer same-source bin
eviction when that counter remains zero.

Recovery states identify their fresh browser context and page, and the raw record
reports the launched browser's version. Worker creation events count total workers
created across the three sequential contexts; they do not measure concurrency.
Recovery cumulative worker counters (including received bodies and completed codec
work) and overwritten app-event counts sum the maximum sampled value in each
context. Worker resource, WASM-memory and app decoded/GPU peaks remain maxima
across snapshots, never sums of per-context peaks; process-memory accounting keeps
the separate boundaries described above. These sampled cumulative totals can omit work
after the last snapshot of a context. The recovery harness does not collect CDP
HTTP byte totals; its body counters and outer TCP observation retain their distinct
boundaries. Normal-journey aggregation and frozen limits are unchanged. Earlier
retained recovery records remain observations of their original exporter.

`node tools/viewer-browser-visuals.mjs "$URL" "$NEW_OUTPUT"` opens the authored
large overview, a reduced 2048-square centre, a full-resolution 512-square centre
crossing source tile boundaries, and an aggressive display-only stretch. The
probe also holds one real additional-detail response to show cached coarse fallback,
then releases it and waits for settled detail. The record binds the deterministic
pattern, immutable parent identity, app snapshots
and image hashes. It asserts that stretch causes no additional JPP or decode work.
Screenshots support appearance inspection; they do not establish numerical pixel
identity or codec quality tolerances.

The fixed retained calibration corpus is indexed in
[`emuella-viewer-evidence/index.json`](emuella-viewer-evidence/index.json).
It preserves the exact ten original baseline contract traces, five additional
browser process-memory traces, selected raw recovery observations and failed-probe
records. Intermediate high-frequency browser snapshots and executables are
explicitly ephemeral provenance artefacts. The initial baseline's first-ready
measurement was guaranteed to be primary-view work by the fresh large-view
scheduler order (visible discard 6 before gallery discard 2); the current app
also explicitly filters primary-view residency, and its regression test preserves
that ordering. Dirty calibration checkout observations are not an independent
attestation of binary build inputs. Final qualification requires the coordinator's
committed source/build record and merged codec representation identities.
