# Native completion pump candidate

Status: **rejected; production completion-pump code removed**. The one fixed
screen is retained in [the results](viewer-acceptance-scheduling-results.md).
Warm-compressed improvement did not compensate for detail, warm-GPU and other
latency regressions or the conditioning absolute failure. No retuning, second
candidate or twenty-pair confirmation occurred. The mechanism and commands below
are historical reproduction instructions for the pinned candidate source/binary;
the current production executable does not expose `--completion-pump`.


Status: provisional implementation, authored checks only; no timing, application,
GPU or browser launch in this work package. Main owns independent review, source
freeze and the separate timing authorisation. There is exactly one candidate,
selected from runtime baseline `fe82f85984fba5c75d7a5a8f10b2d914b773a838` and
completion diagnosis `d982de71ca20611a467bd4b0bb83281570968a31`.
The [diagnosis](viewer-acceptance-completion-results.md) records approximately
25 ms publication-to-receipt, existing worker repaint and 0.29 ms same-frame
post-drain dispatch. It does not establish a particular event-loop cause or
predict this candidate's benefit. The workspace active viewer-acceptance plan
owns selection; this document owns the candidate mechanism and experiment protocol.

## Mechanism and limits

`--completion-pump` or `--completion-pump=true` enables the sole native candidate.
`--completion-pump=false` explicitly selects control in the **same immutable
binary**. Omission remains false. Duplicate or malformed values fail before
application creation. Native snapshots record the boolean independently of
`native-completion-memory-v1`; diagnostic options can be identical for A and B.
The browser has no flag or waiting channel API. Its constant-false app branch
keeps the ordinary cadence; cross-realm timing code is untouched.

Eframe supplies no independent before-redraw user callback here. The candidate
runs inside the existing `eframe::App::ui` update callback, after current demand
reconciliation and before renderer preparation. It replaces no framework or
event loop. On a turn containing newly collected UI intents, apply those intents,
reconcile an empty obsolete demand set and forward cancellations before any
initial dispatch. Defer fresh demand derivation to the next UI pass. Script and
previously submitted intents already precede demand derivation. Never dispatch
after failed reconciliation. During the bounded pump no new OS intent can be
processed until the callback returns.

Catalogue admission keeps its original nonblocking pre-layout drain: no request
can exist before that admission. The pump first services at most one retained
decoded upload and dispatches through the normal runtime scheduler. While a
reservation exists, receive from the existing two-event channel using
`recv_timeout` with only the remainder of one absolute four-millisecond deadline;
return immediately when no reservation exists. After each
receipt use the **same** receipt stamps/cancellation cleanup, app event handler,
`complete_frame`, stale-token check, frame-with-validity upload, ownership release,
eviction accounting and priority dispatch as control. No UI-frame roundtrip is
required for an immediate result while budget remains. Cancellation and failure
acknowledgements also count against the 64-event cap. No spinning, prefetch,
extra execution worker, additional channel or unbounded trace is introduced.

The existing one execution worker, one-job channel, one outstanding reservation,
16 MiB default decoded ceiling, 4 MiB upload scratch and all other existing
limits remain. One turn makes at most 64 receives, 65 single-upload attempts and
65 normal dispatch calls (initial service plus one per event). Each dispatch can
reserve at most one request; publication does not refund its reservation. The
last admitted event completes its bounded upload/refund/dispatch transaction,
even if that work crosses the deadline. No further receive starts at/after the
deadline or count cap. An upload failure retains the normal refund/retry contract,
including the same fixed per-turn bounds if failures repeat.

Four milliseconds is the **requested wait/work budget**, not hard real-time:
synchronous upload/admission work, scheduler work and OS scheduling may overshoot.
The app cannot pre-empt a single upload or guarantee OS wakeup latency. The next
blocking receive always recomputes remaining time; waits cannot each restart a
four-millisecond allowance. Bounded scalar maxima record event batch size, whole
pump duration and receive-call duration. Receive-call duration includes receipt
bookkeeping; it is not an isolated kernel blocked-wait measurement. These values
are diagnostic only. Existing request timing fields, unclipped inversions, rings
and overwrite counters retain their meanings.

Three-frame settlement, first-useful and complete-visible definitions remain;
data readiness stays separate. No presentation/vsync setting, repaint repair,
quality policy, metric name, benchmark admission or comparison implementation is
changed. The source-quality rejection and historical AutoNoVsync rejection remain.

## Frozen native screen and decision

Use only the actual development representations from the active plan: Mansfield
PAN16 4 / RGB16 12 and Tok RGB8 4 regression, including exact required validity.
These remain quality-rejected diagnostic assets. No authored immediate-completion
mode, pixel substitution, mask removal or protected acquisition is permitted.
This scheduling result cannot confer source-quality or cross-platform acceptance.

Before any launch, main reviews and freezes the source revision/tree plus scoped
diff identity, binary path/SHA-256, Cargo.lock, compiler, complete command and
flags, normal workload/threshold hashes, harness identity, exact representation
and mask identities, native libraries, driver/backend, display geometry and
service/cache state. Build scratch is exclusively
`/nvme/development/emuella/.build-targets/viewer-acceptance/scheduling-candidate`.
Protected run evidence belongs only in a newly attributed/lineaged approved store
group selected by main; it must not be written into build scratch. Rebuilding
creates a new identity and cannot silently replace a frozen cohort binary.

Screen: five pairs, alternating order **AB, BA, AB, BA, AB**, ten fresh native
processes. A is explicitly false, B explicitly true, both the same binary and
otherwise identical configuration. Before the whole screen, one separately
identified A=false service-conditioning journey uses the same normal workload;
it is excluded by this predeclaration. No other warmups, retries, discarded
failures or conditioning between cells. Preserve every failure and original
60-second phase / 660-second outer deadline. Keep the ordinary real-scene workload,
all absolute latency/resource bounds and the inherited pressure/recovery journey
and seven recovery events unchanged. Recovery remains a mandatory separate gate;
this protocol does not implement or change its admission.

Promising means **greater than 5% descriptive warm-compressed improvement** across
the five paired observations, no latency degradation greater than 5% for any
inherited latency (including first-useful, detail and complete-visible), and no
failed absolute journey, resource or recovery gate. Keep every absolute resource
bound, including RSS <=243,269,632 bytes; the 5% descriptive non-regression screen
also covers comparable resource and recovery observations. Missing observations
cannot pass. The existing Tok absolute-detail failures remain until proved resolved.

Only if promising, main may authorise **20 new** pairs, alternating AB/BA, with
one separately identified, predeclared A=false service-conditioning journey before
that phase. Screening observations are excluded from confirmation. Zero other
warmups/retries/discarded failures. Use the inherited conservative **99%** comparison
and **5%** practical gate: warm-compressed upper ratio < -5%; all other latency
upper ratios <= +5%; every absolute latency/resource/recovery gate still passes.
No new metric, shorter settlement, changed confidence method or raised ceiling.

There is no second candidate or tuning after results. Reject a failed, unsupported
or non-promising candidate and restore production: default is already false;
remove the production candidate code if not retained while retaining diagnostic
data and its identities. Main records disposition with the semantic evidence
owner and reconciles the workspace plan.

## Ready commands, not executed

Compile and authored checks require no display or GPU. Pin `TMPDIR` as well as
Cargo output to the registered build child. Use cached dependencies, no acquisition:

```sh
export VIEWER_BUILD=/nvme/development/emuella/.build-targets/viewer-acceptance/scheduling-candidate
export CARGO_TARGET_DIR="$VIEWER_BUILD/cargo"
export TMPDIR="$VIEWER_BUILD/tmp"
export CARGO_NET_OFFLINE=true
cargo fmt --all --check
cargo test --locked -p emuella-viewer --lib --bins
cargo test --locked -p polyorama-runtime --lib
python3 -B -m unittest discover -s tools/tests -p 'test_viewer_acceptance_scheduling.py'
cargo clippy --locked -p emuella-viewer --all-targets -- -D warnings
cargo clippy --locked --target wasm32-unknown-unknown -p emuella-viewer -- -D warnings
cargo build --locked --release -p emuella-viewer
cargo build --locked --release --target wasm32-unknown-unknown -p emuella-viewer
```

`cargo xtask verify` also launches applications/browser smoke and performs setup;
it is outside this no-launch work package. Main retains that integration obligation.
Authored tests use synthetic clocks to check count/deadline boundaries and normal
channels to check receipt/cancellation cleanup, priority, reservations, backpressure,
invalid validity rejection and UI-thread upload/refund. They do not establish
wall-clock bounds, GPU upload correctness or measured acceptance.

After coordinator inspection, immutable freeze and a bounded timing grant,
this is the ready command for one screen cell; `PUMP=false` for A and `PUMP=true`
for B in the exact sequence above. Main must bind `FROZEN_NATIVE_BIN`, `VIEWER_URL`,
`SCREEN_CELL` (fresh approved evidence directory) and the frozen native environment.
Use the normal service, display and inherited surrounding measurement/recovery
harness. This launch fragment alone is not a complete admission runner; the composed harness accepts explicit native-only `--completion-pump false|true`
and records the arm in its existing run identity without changing admission.

```sh
timeout 660 "$FROZEN_NATIVE_BIN" --server "$VIEWER_URL" \
  --completion-pump="$PUMP" --native-diagnostics \
  --workload apps/emuella-viewer/real-scene-workload.json \
  --script-output "$SCREEN_CELL/app.json" >"$SCREEN_CELL/process.log" 2>&1
```

No launch, service preparation or measurement is authorised by these commands.
The following machine-readable freeze covers protocol choices and unchanged local
workload/threshold inputs, not representations or measured acceptance. The authored
Python regression verifies these hashes without accessing protected assets.

```json
{
  "schema": "viewer-acceptance-scheduling/1",
  "candidates": 1,
  "native_only": true,
  "default_completion_pump": false,
  "turn_budget_ms": 4,
  "completion_limit": 64,
  "screen_pairs": [
    "AB",
    "BA",
    "AB",
    "BA",
    "AB"
  ],
  "confirmation_pairs": [
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA",
    "AB",
    "BA"
  ],
  "conditioning_journeys_per_phase": 1,
  "conditioning_arm": "A",
  "warmups": 0,
  "retries": 0,
  "confidence_percent": 99,
  "practical_percent": 5,
  "inputs": {
    "apps/emuella-viewer/real-scene-workload.json": "9c11bead76a2c556dd49c13164dd7a462aa01e325ed6d3fb03510054e17f50dd",
    "apps/emuella-viewer/qualification/real-scene-native-thresholds.json": "ad6d9e596dbc9e2aa5142973433e7bb4b184498a98c4dd5123262305998e08d0",
    "apps/emuella-viewer/qualification/real-scene-pressure-thresholds.json": "cb1c1b16429413861024b0a5f68a0641f0eeae835e147ac1b7f2d761f208e18a",
    "apps/emuella-viewer/qualification/real-scene-recovery-thresholds.json": "a63610181385136f817b11b0afc7543d330e38e7dd04f4b440c45eb0f33c06a0"
  }
}
```
