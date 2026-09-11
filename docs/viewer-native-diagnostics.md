# Native completion and allocation instrument

Status: implementation/protocol candidate, unmeasured. Starting checkpoint:
`4a1594b0840eadae26a4d6005d50d9201734a1a7`. The coordinator must commit and
freeze the final source/tree, binary and configuration before any timing run.
Instrument identity is `native-completion-memory-v1`; default construction
remains available. No scheduling repair, presentation change, speed result or
resource acceptance is selected by this implementation. The historical
AutoNoVsync rejection remains. Main owns canonical checks, independent review,
CI and landing. The separate independent worker's files are outside this scope.

The exploration question is whether native completion delivery, UI drain or
subsequent dispatch explains the observed waiting with decode removed, and
which retained lifetimes explain current RSS across repeated releases. The
smallest probe is the authored immediate-result workload through the actual
one-worker executor and upload route, followed only in an allocated window by
an identified full-scene memory cohort. Polyorama owns detailed evidence in the
approved store; the workspace acceptance plan owns selection. Exit this
implementation phase when the bounded mechanisms, authored tests and protocol
are ready. Cause attribution and any repair require the later measured evidence.

## Completion mechanism and boundaries

`--native-diagnostics` enables a 256-record boundary ring with an explicit
overwrite counter. The existing 128-event request ring and cumulative pacing
counters remain. Trace dispatch, completion UI drain, upload start/finish,
render submission, data readiness and settlement; request pacing records carry
worker start/finish, publication, UI receipt and optional wakeup-request stamps.
The native publication stamp is immediately before channel send. Wakeup is
stamped immediately before the existing `request_repaint` call, after send.
The UI can race ahead of that stamp; `null` means unavailable, never zero delay.
Render submission observes CPU preparation/callback enqueue, not GPU execution,
presentation or scanout. Upload finish observes renderer admission and released
CPU payload ownership, not completion of GPU work.

Native pacing uses one process-wide Rust monotonic origin. External memory
markers use actual Linux `CLOCK_MONOTONIC` and are **not** aligned by subtracting
an estimated application-start offset. Browser worker intervals and UI intervals
are accumulated only within their respective realms. Cross-realm duration fields
remain zero with `cross_realm_intervals_unavailable=true`; they must be treated
as unavailable. Raw stamps and `cross_realm_inversions` retain the uncertainty.
Native inversions remain invalid observations, without clipping.

`phase_data_ready_ms` records the first all-current-demands-resident observation
without the three-frame requirement. It is separate from first-useful and
complete-visible settlement, and may describe an interim scroll demand set.
The original three-frame settlement, desired-count, residency and no-outstanding
reservation predicates remain unchanged. The `worker_concurrency` measurement
continues to mean outstanding reservations. It has not been relabelled as the
number of executing threads.

The executor still has exactly one execution worker, a one-job channel, the
same cancellation and priority reconciliation, and UI-owned completion/upload.
Its event channel is bounded to two slots (catalogue plus the single reserved
completion). No prefetch, additional execution threads or changed dispatch
cadence are introduced. The one optional wakeup observation is released with
its receipt. No completion-driven repair has been applied.

`--authored-immediate-completion` additionally requires diagnostic mode, scripted
output, an explicit workload with `authored_completion=true` on every step,
and a catalogue whose source and encoding-contract fields both equal
`authored-completion-diagnostic-v1`, with no validity metadata. Every demand
still uses the normal reservation and channel path. A constant U16 output with
the requested dimensions, reduction, channels and precision is allocated only
after admission. It reserves the existing pair-of-output budget; no full raster
or precomputed result cache exists. It produces no decoded quality evidence or
fake codec counters. Real data, source samples and masks use the unchanged
normal fetch/decode/validity path. The composed runner explicitly disqualifies
authored immediate results from quality and production performance acceptance.

## Actual memory boundaries and ten cycles

`EMUELLA_VIEWER_MEMORY_MARKERS` selects exclusive marker output; the composed
runner sets it when `--memory-diagnostics` is supplied. `startup` precedes
graphics, `graphics-ready` is emitted from the creation context before app-owned
GPU resources, and each script phase emits actual start/settled markers. The
scalar schema and Python bracketing rules are in
[the memory contract](viewer-acceptance-memory.md). Each output file has a
256-record, 256-KiB cap. Invalid labels, clocks, exhaustion or writes produce an
explicit error through startup stderr or application `errors`; the failed
producer stops. It does not retry or add repaint requests. Partial files remain
for diagnosis and cannot establish complete coverage.

The companion `memory-phase-markers.allocations.jsonl` joins each marker exactly
to a separately timed observation: current `VmRSS`, `VmHWM`, optional identified
glibc-version `mallinfo2`, runtime ownership and retained container slots.
`mallinfo2` is dynamically resolved; unsupported platforms/symbols and allocator
interposition indicated by `LD_PRELOAD` are unavailable. Arena bytes, in-use
arena bytes, free arena bytes, mmap bytes/count and top releasable bytes retain
their allocator definitions. Free arena blocks may remain retained; the top
figure is a subset. Tcache, foreign allocators and driver allocations are not
fully attributed. These are neither exact Rust live bytes nor RSS. No trim,
allocator replacement or allocation hooks are installed.

Runtime diagnostics distinguish current decoded sample/validity capacity,
outstanding worker reservation bytes, transferred upload bytes, map entry
counts and resident/failed entries. Upload bytes remain charged until the
existing release acknowledgement, even after payload ownership leaves runtime.
Map-node overhead is unavailable. Event/trace/stage slots count outer container
capacity, not all nested allocations. Retained stage snapshots contain bounded
copies of evidence; their overhead is part of this instrument. Runtime counters
reset at the existing display-reset epoch. Explicit released item/logical byte
totals are cumulative; they are not physical GPU bytes and are never added to or
subtracted from RSS. No physical GPU allocation claim is made.

`native-memory-workload.json` preserves all 29 ordinary real-scene actions and
appends exactly ten predeclared `ClearDisplayCache` actions. Each cycle drops
actual resident display resources at a settled executor boundary, then reloads
and uploads the same complete demand set through the existing path. An eviction
marker requires nonzero prior texture items/bytes. A revisit marker requires
exact demand-key equality, actual new uploads and the original settlement gates;
cycle numbering must be consecutive. The runner requires all ten completed.
The epoch and cumulative released counters make the lifetime boundaries visible.
These are **explicit display release/revisit cycles**, not claims of natural LRU
or compressed-representation eviction. They preserve compressed resources and
cache ceilings. The inherited pressure/recovery journey and all seven recovery
events remain separate mandatory gates; these cycles do not substitute for them.
No cache is shrunk or ordinary workload/threshold changed.

The authored workload additionally exercises RGB8 before the cycles and uses
three 43,008-square authored PAN16/RGB16/RGB8 catalogues. The fixture server
serves metadata only, with no real pixels, descriptors, masks or codec source.

## Ready commands and freeze protocol

Build and authored probes use only the registered directory below. Development
builds from this uncommitted candidate are **not frozen diagnostic binaries**.
After main freezes the committed identities, build and retain the release binary
before canonical workspace feature unification can change it:

```sh
export VIEWER_BUILD=/nvme/development/emuella/.build-targets/viewer-acceptance/native-diagnostics
export CARGO_TARGET_DIR="$VIEWER_BUILD/cargo"
cargo build --locked --release -p emuella-viewer
```

Freeze source commit/tree, Cargo.lock, native binary SHA-256, compiler, exact
commands/options, both Python diagnostic scripts and composed harness hashes,
workload hash, representation/mask identities, linked-library files/hashes,
allocator availability, driver/backend, display geometry and service/cache state.
Record this separately from production identity. Retain a uniquely named binary
copy before rebuilding; bind that path in the execution capsule. WASM changes
need the existing `cargo xtask build-viewer-web` route for later browser runs;
no browser immediate-result mode is added.

Prepare authored inputs in a fresh registered child (this is not a timing run):

```sh
python3 tools/viewer-native-diagnostics.py author --output "$VIEWER_BUILD/authored-inputs-01"
python3 tools/viewer-native-diagnostics.py serve \
  --catalogue "$VIEWER_BUILD/authored-inputs-01/catalogue.json" --port 8195
```

In the **later allocated diagnostic window**, an authored run uses the frozen
binary, supplied workload and markers. `AUTHORED_RUN` must be a fresh registered
build-store child; create it exclusively. The service above is separately owned
and stopped after the cohort. Bind display and Vulkan environment before launch.
The known native environment is:

```sh
export LD_LIBRARY_PATH=/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib
export WGPU_BACKEND=vulkan
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json
```

The available display executable is
`/nvme/development/polyorama/.tools/sysroot/usr/bin/Xvfb`. Allocate and record an
exclusive display number with `-screen 0 1440x900x24 -nolisten tcp`, retain its
logs with the run, and export that `DISPLAY`; do not reuse an unidentified display.
Use the composed runner's fixed 660-second deadline for a formal cohort; the
following direct invocation is for a bounded authored mechanism probe, with an
explicit 660-second shell deadline and preserved failure exit:

```sh
mkdir "$AUTHORED_RUN"
EMUELLA_VIEWER_MEMORY_MARKERS="$AUTHORED_RUN/memory-phase-markers.jsonl" \
  timeout 660 "$FROZEN_NATIVE_BIN" --server http://127.0.0.1:8195 \
  --native-diagnostics --authored-immediate-completion \
  --workload "$VIEWER_BUILD/authored-inputs-01/workload.json" \
  --script-output "$AUTHORED_RUN/app.json" >"$AUTHORED_RUN/process.log" 2>&1
python3 tools/viewer-native-diagnostics.py audit --run "$AUTHORED_RUN"
```

No protected output may use that authored path. For actual evidence,
`SCENE_EVIDENCE` must be a fresh attributed/lineaged group under
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`.
Use the existing verified service and reproduction arguments, once for each of
exactly five predeclared fresh starts, with `RUN` fixed to 01 through 05:

```sh
python3 tools/viewer-composed-journey.py \
  --mode native --output "$SCENE_EVIDENCE/native-$RUN" --url "$VIEWER_URL" \
  --native-bin "$FROZEN_NATIVE_BIN" --web-root "$FROZEN_WEB_ROOT" \
  --codec-repo "$CODEC_CHECKOUT" --benchmark-repo "$BENCHMARK_CHECKOUT" \
  --server-cache-state "$FROZEN_SERVER_CACHE_STATE" \
  --catalogue-contract "$FROZEN_CATALOGUE_CONTRACT" \
  --thresholds apps/emuella-viewer/qualification/native-memory-thresholds.json \
  --workload apps/emuella-viewer/native-memory-workload.json \
  --native-diagnostics --memory-diagnostics
python3 tools/viewer-native-diagnostics.py audit --run "$SCENE_EVIDENCE/native-$RUN"
```

The native-memory threshold file changes only the workload binding and freeze
metadata; every inherited numeric bound, hardware requirement and required event
is unchanged and checked by a regression.

These commands are ready, **not executed measurement evidence**. The next
execution capsule binds the exact committed candidate and fixed run budget.
Five fresh starts per immutable diagnostic binary/configuration, zero warmups,
zero retries, no discarded failures, ten cycles each, unchanged 60-second phase
and 660-second outer deadlines. Retain startup errors, absent brackets,
allocator unavailability, ring loss and incomplete cycles. Audit checks coverage
only; RSS growth still requires all same-lifetime current-RSS observations and
brackets, not high-water differences alone. Keep 278,794,240 bytes as the original
observation and 243,269,632 bytes as the unchanged ceiling. At most one
cause-supported resource repair then five new starts; separately, at most one
completion-driven scheduling repair may be selected after diagnosis under the
workspace's frozen platform comparison gates. No cause, repair or acceptance
claim is established here.

## Authored validation of the implementation candidate

Focused viewer and runtime library tests cover actual native catalogue/receipt,
reservation retention, decoded/upload backpressure, output sizing, global reduced
grid, authored-only input admission, three-frame settlement, ordered cycle
restoration and browser realm separation, plus the existing cancellation/stale
and compatibility behaviour. The Python native diagnostic tests cover catalogue
geometry, preserved ordinary actions, ten ordered resets, marker/allocation
coverage and bounded failure. The existing memory and composed-journey Python
regressions remain applicable. Native all-target and WASM lint/build checks use
the registered build root. These authored checks are not measured qualification.

The existing explicit regional GPU readback regression passed on NVIDIA GeForce
RTX 3090, Vulkan, driver 610.43.03, with scalar/RGB precision, masks and actual
texture eviction. It used authored pixels only; it is separate from actual
full-scene completion, ten-cycle execution and resource/latency acceptance.
