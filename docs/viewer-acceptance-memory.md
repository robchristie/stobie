# Bounded viewer memory diagnostics

This is an instrumentation candidate, not a resource acceptance result. The
question is which process mappings and retained lifetimes account for the
native RSS failure, and whether settled memory grows over repeated eviction and
revisit. The smallest representative measurement is one identified full-scene
configuration covering startup/graphics, first view, peak phases and ten fixed
eviction/revisit cycles. Polyorama owns these observations; the workspace
`docs/plans/active/viewer-acceptance.md` owns selection and terminal acceptance.
The original Python collector checkpoint ran no viewer or build; authored native
validation is now recorded in the linked native instrument protocol.

Use the existing [reproduction route](real-scene-viewing-reproduction.md) and
[calibration boundaries](emuella-viewer-calibration.md). Add
`--memory-diagnostics` to the composed runner only in the coordinator's allocated
measurement window. It adds `process-memory-diagnostics.json` beside the existing
run files and preserves the diagnostic source there. All outputs, including
`process.log`, optional marker records and pixels, must remain at that actual
runner output path in a fresh attributed/lineaged group in the approved store.
Do not copy raw process logs, application events, images or protected excerpts
to scratch, repository reports or review transcripts. This work authorises no
acquisition, deletion of protected material, redistribution, standards changes
or external codec source consultation.

## Frozen measurement and repair policy

Per immutable binary/configuration, perform exactly **five fresh native starts**,
with **ten predeclared eviction/revisit cycles per start**, zero warmups, zero
retries and no discarded failures. Preserve startup failures and incomplete
cycles as failures; a timeout is not a measurement. Bind the binary and harness
hashes, linked libraries, graphics driver/backend, display geometry, workload,
representation identities and service/cache state before the cohort. A rebuild
has a new identity. Quality-rejected assets support diagnosis only.

The runner still executes one supplied workload once. It does not manufacture
cycles, repeat starts, change the 660-second outer deadline, change phase
deadlines or weaken three-frame settlement. The owner must freeze the actual
ten-cycle workload and provide its identity before measurement; the historical
workload is not claimed to contain that new experiment. If it cannot finish
within the fixed limits, retain the incomplete result instead of silently
extending the run. Benchmark workload admission and measurement contracts remain
with their existing owners.

Retain the historical **278,794,240-byte** observation and unchanged
**243,269,632-byte** native ceiling. `process-memory.json`, application
`process_peak_rss_bytes`, existing process-group observations and benchmark
thresholds keep their acceptance definitions. The new identity-aware diagnostic
table does not replace the historical per-PID high-water sum. Do not substitute
PSS, private bytes, allocator estimates, a selected low repeat, or cache/GPU
logical counts for the process RSS gate. Shared pages may appear in multiple
process RSS values; per-process high-water peaks need not be simultaneous.

Allow **at most one cause-supported resource repair**, followed by five new fixed
starts with ten cycles each under the new immutable identity. Attribute the
cause before selecting the repair. Unexplained variation is not a repair basis;
do not shrink caches without cause, raise the ceiling or retry until a pass.
Retain or reject the sole repair against the unchanged gates and complete cohort.
If the evidence cannot establish a cause, report missing evidence and the next
separate action. Instrumentation alone is not a completed bounded rejection.

## Diagnostic observation boundaries

The opt-in collector uses the same discovered application descendants as the
existing sampler, excluding Node in browser/recovery mode. It reads Linux
`stat` before and after each process observation, binds PID to kernel start-time
ticks, and records the host boot identity when available. PID reuse across
rounds creates separate lifetime records. Reuse or exit during a read discards
the mixed record. Missing identity prevents reporting unattributed memory.
Each lifetime retains first/last sample references and its maximum observed
`VmHWM`, even after exit; start-time ticks are not wall-clock timestamps.

Each selected process supplies current `VmRSS` and kernel `VmHWM`, in bytes.
One process per round additionally supplies `smaps_rollup` and a `smaps` category
summary. The fixed categories are heap, stack, anonymous, shared library, file,
device and special. They are pathname heuristics, not allocator or driver
ownership claims. Mapping counts and virtual address bytes are distinct from
resident bytes. RSS, PSS, private/shared clean/dirty, anonymous and swap fields
are reported separately; several overlap. Do not add these fields to RSS or
infer physical GPU allocation from a device mapping. No mapping filenames,
addresses, raw proc text, allocation contents or application event payloads are
included in the diagnostic report.

Missing fields, absent/exited processes, permission failures, malformed data and
bound exhaustion carry explicit unavailable reasons. A missing field is not
zero. A missing field in any mapping makes that category's field unavailable;
an over-limit read discards the whole corresponding summary instead of exporting
a partial sum. `smaps` and rollup are sequential observations and need not match
each other or the status read while the process changes. Identity stability does
not make those reads atomic or detect `exec` within the same lifetime.

The collector is capped at one round per second, 661 rounds, 32 PIDs per round,
256 retained identities and 8,192 process rows. It rotates PID selection when
necessary and selects only one deep PID read per round. File read limits are
16 KiB for stat, 64 KiB each for status and rollup, and 2 MiB/4,096 mappings for
smaps. Marker input is capped at 256 KiB and 256 records. Skipped round, PID,
identity and row slots are reported; bounds never create a passing claim.
These caps bound input and retained diagnostic structures, not total runner
RSS, kernel read time or the existing application evidence files.

Collection occurs inside the existing polling loop after its acceptance sample.
Deep reads add overhead and can widen the nominal 50 ms sampling gap in an
instrumented run. Each diagnostic read retains its actual Linux monotonic start
and finish. Compare instrumentation configurations explicitly; do not claim a
precise one-second cadence, an instantaneous group peak or an uninstrumented
latency from these observations. Short-lived, reparented or unselected processes
can be missed. Non-Linux reports mark proc diagnostics unavailable.

## Actual phase attachment and native producer

The runner sets `EMUELLA_VIEWER_MEMORY_MARKERS` to
`memory-phase-markers.jsonl` inside its exclusive output directory when the
option is enabled. The opt-in native producer now emits one bounded JSON line
at each actual
startup, graphics-ready, phase-start, phase-settled, cycle-evicted and
cycle-revisited boundary. The [native instrument protocol](viewer-native-diagnostics.md)
defines the implemented workload, allocation observations and ready commands.
This instrumentation remains unmeasured. The scalar schema is:

```json
{"schema":"viewer_memory_phase_marker/1","clock":"linux_monotonic","pid":42,"start_time_ticks":100,"monotonic_ns":2000000000,"sequence":0,"phase_label":"cycle-01","kind":"cycle-revisited"}
```

The numbers above are authored examples, not observations. `monotonic_ns` must
come from Linux `CLOCK_MONOTONIC`, in the runner's clock namespace, at the actual
boundary. A process-relative Rust `Instant` elapsed value or browser
`performance.now()` is not that clock. Bind PID and `/proc/self/stat` field 22;
use a strictly increasing sequence and nondecreasing timestamps. Labels contain
1–96 ASCII letters, digits or `_.:-`; kinds are exactly those listed above.
Emit at most 256 records and 256 KiB, including failure records in the existing
owner failure channel if marker production fails. Keep phase markers optional
and bounded; do not block settlement, alter workload dispatch or add worker
threads, retries or unconditional repaints. A browser producer requires its own
validated clock and process attribution; it cannot reuse native assumptions.

The report attaches references to same-PID/start-time samples wholly before and
after each validated marker, within 1,500 ms. These are explicitly bracketing
observations, not instantaneous RSS or interpolated phase peaks. Missing sides
remain unavailable, including short phases and process exit. Existing stage
snapshots separately retain their reported application high water under the
actual phase label and stage index. Their relative timestamps are never aligned
to the runner by guessing an application-start offset. No marker file, invalid
clock, invalid ordering or over-limit input leaves external phase attribution
unavailable; ordinary workload completion does not establish marker coverage.

For cycle-growth analysis, retain all ten actual eviction/revisit markers and
the same-identity current-RSS brackets at comparable settled boundaries. Report
the actual first/last differences and every intervening cycle, including gaps
and failures. Rising `VmHWM` alone does not establish live retained growth.
No repeated-cycle growth result is claimed by this unmeasured implementation.

Native allocation/lifetime attribution remains unavailable from proc alone.
The native producer adds separate optional glibc `mallinfo2` observations and
owner counters for decoded payload capacity, reservations, upload handoff,
container slots and explicit display release, with their limits and reset
semantics in the linked protocol. These are not complete allocation attribution.
An externally available allocator profiler needs an identified allocator/build,
its actual live/retained definitions and approved output retention; do not invent
an allocator sum from cache capacities or subtract logical GPU bytes from RSS.
Record graphics/library identities and physical allocation unavailability
separately. The real cohort is still needed before cause or repair selection.

## Authored checks

The original collector checks are these focused Python regressions:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/tests -p 'test_viewer_memory_diagnostics.py' -v
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/tests -p 'test_viewer_composed_journey.py' -v
```

The fixtures are authored temporary proc records; they contain no real-scene
pixels or process captures. They cover identity parsing/reuse/exit, bounded
reads and retention, missing fields, mapping summaries and marker attribution.
Main owns final integration, representative runs, canonical verification and
independent review. These checks establish parser behaviour, not native memory
acceptance, ten-cycle coverage, sampling overhead or a resource repair.
