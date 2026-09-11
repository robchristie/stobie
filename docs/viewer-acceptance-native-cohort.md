# Fixed native resource cohort

Status: prepared, unexecuted. Main commits this protocol and runner before
granting execution, after the authored probe owner has stopped. This work starts
no viewer, browser, Xvfb or representation service and takes no measurements.
The [machine protocol](viewer-acceptance-native-cohort.json) binds runtime
`fe82f85984fba5c75d7a5a8f10b2d914b773a838`, the retained `native-frozen-v1` binary,
and source/build receipt commit `e1e5ce738d7fb0452cd2e9d2ad8c30486ea6988d`.
The later wrapper commit is a separate harness identity; it never relabels the
runtime. No frozen application or composed-harness source was changed.

The exploration question is whether actual current RSS stabilises across ten
explicit display-cache release/revisit cycles, and what the available lifetime
and mapping evidence can explain about the retained native ceiling failure.
The smallest declared probe is the entire fixed five-start cohort below.
Polyorama owns detailed evidence in the approved store. The workspace
`docs/plans/active/viewer-acceptance.md` owns disposition. Exit preparation when
this protocol, external runner and authored checks are ready for main's commit;
exit the later measurement phase only after all five outcomes, actual native
visual inspection and missing predicates have been reconciled. No cause-supported
resource or scheduling repair is implemented here.

## Fixed execution boundary

- Exactly five declared fresh native starts, numbered `native-01` to `native-05`,
  zero warmups, retries, replacements or reruns. Every slot is retained even
  when application startup, harness export, audit or admission fails. An
  infrastructure failure consumes the cohort and records five unavailable
  outcomes; it cannot be described as five observed native starts.
- Every start uses the exact `native-memory-workload.json` and
  `native-memory-thresholds.json`: all ordinary actions, then ten actual
  `ClearDisplayCache` release/revisit cycles. All inherited threshold numbers,
  hardware predicates and required events remain unchanged.
- The application retains its 60-second phase deadline and the composed harness
  its 660-second native deadline. The wrapper additionally terminates its owned
  native group at 660 seconds from the recorded launch boundary. Setup and final
  export each have separate 120-second allowances; they cannot extend native
  execution. No timeout becomes a completed phase or a passing observation.
- One exclusive Xvfb display, `1440x900x24`, and one representation-service
  lifecycle span the cohort. Both use existing frozen executables and libraries.
  No download, display reuse, browser, window manager or extra viewer is needed.
  Cleanup signals only owned PID/start-time/process-group identities. It retains
  logs and partial outputs and never removes input, screenshot or evidence files.
- The first service observation is labelled `uncontrolled-first-observation`;
  subsequent slots use the inherited `warm-server` enum because the same service
  persists. This does not promise a fully populated server cache after a failed
  prior run. Each harness retains its actual before/after service metrics;
  client caches start empty. No service-conditioning journey, OS cache drop,
  storage conditioning or selective low-repeat result is introduced.

The representations are the existing approved Mansfield PAN16 4 and RGB16 12
`viewer-acceptance-full-scenes-mansfield-{pan16-4,rgb16-12}-01/representation`
directories. Their notice, lineage and manifest hashes are frozen in the JSON.
The existing development catalogue matches source identity, band order and full
geometry. Before and after the cohort, the runner streams hashes of every
representation file; it never prepares, modifies or deletes a protected input.
Source arrays, original MSI and defaults remain unchanged. These representations
remain quality-rejected diagnostic assets.

## Actual native screenshots

Read-only `import -version` and `import -help` confirmed installed ImageMagick
7.1.2-23 Q16-HDRI with its X delegate. The tool path and SHA-256 are frozen in
the JSON. No display was opened during that capability check.

For **each declared run**, the wrapper schedules at most two screenshot
processes at **2 and 5 seconds** from Linux `CLOCK_MONOTONIC` immediately before
the existing harness's single native `Popen`. The observed Popen-return time and
PID/kernel start-time identity bound that launch interval; it is not an estimated
application clock offset. The actual capture attempt/finish monotonic and Unix
timestamps, lateness, duration, command, exit status, file identity and dimensions
are retained. Captures are not labelled as exact workload phases.

The command is installed `import -display :N -window root -silent -snaps 1`,
with 8-bit PNG24 output, one capture thread, 64 MiB ImageMagick memory, no map/disk
cache, a two-second capture deadline and an 8 MiB per-file hard limit. Each image
must have a 1440 by 900 PNG header. Logs share that per-file bound; partial PNGs
and errors remain in place. Temporary paths point only into the fresh approved
capture directory and core dumps are disabled for the capture tool. No more than
ten image paths are allocated by the cohort. If the native process has exited
before a scheduled capture, the runner records an unavailable capture rather
than photographing a stale display or retrying.

The capture process is a sibling of the native process, outside the native
descendant RSS sample. Its X11/CPU/driver contention, the wrapper's bounded map
reads and existing memory diagnostics still affect observed latency. No overhead
is subtracted and no uninstrumented performance claim is made. Observed native
and capture mapped-file metadata are retained; libraries are hashed after native
exit against their observed inode/size/modification identity. Mapping failures,
process exit and partial dynamic-library coverage remain explicit. The app's
actual adapter/backend string, actual X11 geometry, configured environment,
frozen graphics files and service identity remain separate evidence.

The coordinator must open the retained **actual native** PNGs inside their
approved group and record visible UI/content/mask behaviour, blank or incomplete
views, screenshot identities and limitations. Image existence, a PNG header and
the earlier authored GPU readback do not establish visual acceptance. If neither
scheduled capture supplies usable native evidence, visual proof remains missing;
this protocol does not authorise another start or capture to fill the gap.

## Read-only and authored commands

From `/nvme/development/polyorama-viewer-acceptance`:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/viewer-acceptance-native-cohort.py check
PYTHONDONTWRITEBYTECODE=1 \
  TMPDIR=/nvme/development/emuella/.build-targets/viewer-acceptance/native-frozen-v1/tmp \
  python3 -m unittest discover -s tools/tests -p test_viewer_acceptance_native_cohort.py -v
```

`check` verifies static identities and contracts only. It does not create a
protected group, open a display, start a service, or run benchmark admission.
The authored tests use temporary scalar/PNG-header fixtures and mocked processes.
Preparation validation passed 15 wrapper tests plus the existing 5 native
diagnostic, 17 memory and 3 composed-harness regressions: 40 authored checks.
Static identity, Python syntax, JSON and whitespace checks also passed. These
checks establish runner behaviour only; no native process or capture was tested
against a live display.
Canonical `cargo xtask verify` is not run in this preparation task because its
browser smoke and application starts are outside the explicit task boundary.
Main retains responsibility for later canonical checks and review.

The existing benchmark CLI was built only, from configured owner checkout
`70eb3d1d9448f617333d658d9a60bb5b9439ecb2`, with `cargo build --offline --locked
--release --bin emuella-benchmark`. Its registered scratch directory, exact build
command, successful log, lockfile and binary hashes are in the JSON. No benchmark
measurement or journey admission ran during preparation. The wrapper later invokes
that existing CLI's `journey TRACE THRESHOLDS` on each actual composed trace; a
missing trace remains unavailable and is never manufactured.

## Later coordinator-granted command

After committing all four owned files, main creates a grant with these exact
fields. This is a schema example, **not a grant**; replace the commit, fresh group,
display, port and attribution with main's selected values. Keep the actual grant
in registered build scratch; it contains no protected data.

```json
{
  "schema": "viewer_native_cohort_grant/1",
  "protocol_commit": "FULL_COMMITTED_HARNESS_HEAD",
  "output_name": "viewer-acceptance-native-cohort-mansfield-01",
  "display": ":177",
  "port": 8194,
  "execute": true,
  "authored_probe_owner_stopped": true,
  "granted_by": "COORDINATOR_ATTRIBUTION",
  "granted_utc": "ACTUAL_UTC_TIMESTAMP"
}
```

`execute` checks that the grant matches the current committed harness HEAD and
all four files, that inputs/helpers match the frozen protocol, and that the
exclusive group and display are unused. An existing `execution.json` with the
same immutable cohort ID prevents a rerun under another group name. Main's
grant is an execution attestation; the tool cannot independently establish that
another owner has stopped. Unrelated files owned by concurrent workers need not
be clean and are never staged by this runner.

```sh
export LD_LIBRARY_PATH=/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib
export WGPU_BACKEND=vulkan
export VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json
unset LD_PRELOAD WAYLAND_DISPLAY
PYTHONDONTWRITEBYTECODE=1 python3 tools/viewer-acceptance-native-cohort.py execute \
  --grant /nvme/development/emuella/.build-targets/viewer-acceptance/native-cohort-grant.json \
  --protocol-commit FULL_COMMITTED_HARNESS_HEAD \
  --output-name viewer-acceptance-native-cohort-mansfield-01 \
  --display :177 --port 8194
```

The runner creates a fresh attributed group through the existing
`viewer-real-scene-source.py::fresh_group`, preserving the reviewed notice and
prepared lineage. All native logs, sampler records, traces, allocation evidence,
captures, service/display logs and summaries stay in that approved group beneath
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`.
No raw evidence is copied into scratch or repository reports.

## Evidence and limits

`execution.json` binds the runtime and harness separately, committed protocol,
grant, input inventory and actual environment. `infrastructure.json` binds actual
owned service/display commands, catalogue, process identities, geometry and
cleanup. Each slot retains a launch receipt, wrapper outcome, captures, unmodified
composed evidence, diagnostic coverage, benchmark assessment and bounded summary.
`outcomes.json` always retains all five slot dispositions. `inputs-after.json`
records the unchanged-input comparison; `summary.json` links all five summaries.
All ordinary failure paths continue to later slots. Coordinator interruption,
host loss or unrecoverable output-store failure cannot promise complete exports;
already written evidence remains and the cohort is not rerun.

Each summary reports absolute application HWM, actual sampled descendant RSS and
the inherited per-PID HWM sum against **243,269,632 bytes**, retaining original
**278,794,240 bytes**. Proc/smaps, allocation/lifetime observations, every available
cycle bracket, current-RSS first/last difference, phase high waters, absolute
latencies, actual delivered bytes and limitations are supplementary. A current
growth result requires all ten comparable same-lifetime before/after brackets;
gaps remain unavailable. HWM differences cannot substitute for current growth.
Smaps fields overlap; allocator statistics and logical resources are never added
to or subtracted from RSS. Deep-map first/last summaries link the complete
bounded diagnostic table rather than discarding intermediate observations.
If composed-trace export fails after sampling, the summary still reports the
retained original `process-memory.json` sampled peak and per-PID HWM sum, plus
available application final HWM, with explicit provenance. It does not generate
a replacement trace or claim unobserved earlier application-stage peaks.

Exit 4 retains failed/incomplete startup, screenshots, memory/coverage or
benchmark evidence. Exit 0 means these mechanical checks passed; visual
inspection, original quality rejection and integration disposition still require
reconciliation. Ten explicit display release cycles do not prove natural LRU
eviction, compressed eviction or the inherited **seven-event pressure/recovery**
journey. That separate requirement is neither run nor waived here. This protocol
does not deliver a repair, completion result, speed claim or resource acceptance.
