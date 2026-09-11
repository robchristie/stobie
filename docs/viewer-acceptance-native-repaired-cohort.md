# Repaired runtime: fixed native cohort and browser capsule preparation

Status: **ready for main's commit; unexecuted**. This is the sole repaired-runtime
cohort, separately identified from the original results. Main owns review,
commit, execution grants, canonical verification and later merged qualification.
Preparation starts no application, browser, GPU, display, representation service
or timed acceptance run. No protected output group was created.

The [build receipt](viewer-acceptance-repaired-build.json) freezes committed
`8633754fa28f2ca34f159a7368e0b8e7e953d205`, tree
`df6faac74c811315d07d5ec463f4e5450db56931`, from a clean Git archive in registered
`/nvme/development/emuella/.build-targets/viewer-acceptance/repaired-frozen-v1`.
All 852 archived regular files match after building; Cargo.lock is unchanged.
The original uncommitted worker artefacts are not this freeze. The source manifest,
archive, lockfile, toolchain executables/versions, flags, build logs, native binary,
complete static manifest and loader/library identities are bound in the receipt.

The repair protects only active required masks/bins and explicitly fails exhausted
continuations, as documented in [mask admission](viewer-acceptance-mask-admission.md).
New source caches do not change representation identity, source-mask semantics,
all/any validity, decoded native references or source arrays. No preparation,
re-encoding, source TIFF access, external codec source consultation, acquisition,
new terms, protected copying or deletion is performed.

## Frozen builds

All commands ran from the archived `source` directory with
`CARGO_TARGET_DIR=$FREEZE/cargo`, `CARGO_NET_OFFLINE=true` and
`TMPDIR=$FREEZE/tmp`, where `FREEZE` is the registered path above:

```sh
cargo build --offline --locked --release -p emuella-viewer
cargo build --offline --locked --release --target wasm32-unknown-unknown -p emuella-viewer
POLYORAMA_VIEWER_WEB_DIR="$FREEZE/web-built" \
  cargo run --offline --locked --package xtask -- build-viewer-web
```

The native release was copied to read-only `measured-bin/emuella-viewer` before
WASM/xtask work. The complete eight-file static root and `web-manifest.json` were
retained read-only before any canonical feature unification. Package defaults,
thin LTO, one codegen unit and release optimisation remain unchanged. The nested
xtask WASM build is offline against the already locked graph; archive comparison
checks that it did not change Cargo.lock. Its Node/WASM response-header check is
an authored build check, with no browser or listener.

| Artefact | SHA-256 |
| --- | --- |
| Native, 27,275,016 bytes | `3624000ce2d78ff1b646db572102df1a028800278d6fdb661fde43d36a10f0a6` |
| Static manifest | `63cf2e1529b16e715edaf63747a901466e67f67318062968c76672d909e1da5d` |
| Packaged WASM, 11,377,122 bytes | `0e5731c51c930784490f7e2d73a3d996546d2918ef69eeb2773152d88161b774` |
| Source Cargo.lock | `8659266333ffacc69d40ab7f31336801acb800ff05cf1375c09a7bef6a3aeb75` |

The separately identified existing service remains
`4a1594b0840eadae26a4d6005d50d9201734a1a7`, binary SHA-256
`7ea60902498fbbfcac67f7d7785e42a38b4ce06741895ad916f8996c0f137102`.
The service/tool/reference files and validity implementation match that revision;
the lockfile delta only adds the viewer's libc dependency. Loader tracing binds
current native, service, Xvfb, capture and graphics-library files without invoking
application entry points. Actual service PID/start time, loaded libraries, hardware
adapter/backend and display remain execution observations. Main rebuilds the final
merged service later; this old service is never relabelled as the repaired runtime.

## Protocol equality and selected-file binding

The [repaired machine protocol](viewer-acceptance-native-repaired-cohort.json)
inherits the [original protocol](viewer-acceptance-native-cohort.md) and JSON,
whose immutable SHA-256 is
`68dbbdee8032f9261c527e41c5814fea371024ae40e880662fbe5252ce8c9816`.
The runner's `--protocol` defaults to the original file. Only the two literal
repository paths are accepted; aliases, arbitrary replacement files and new
protocol fields reject. No module-global protocol replacement or new monkeypatch
selects the repaired candidate. The original committed launch observer remains
unchanged and continues to observe the single native Popen boundary.

The comparison rejects every change outside runtime/build/harness identities.
Every budget, threshold record, workload/action record, catalogue, representation,
owner, environment, capture policy and recovery requirement matches the original.
The only helper difference is the existing five-line additive optional
`--completion-pump` argument, pinned to SHA-256
`706232cf5cac2c39deb0124aa7e0c40fb078298f42946e4b10044cd0e34d7a1f`.
This cohort never passes it; authored checks compare all five argument lists after
substituting only native/web paths. All other helper records remain identical.
The original protocol still expects its original helper identity; its historical
records are preserved, not rewritten to accept this later helper.

| Invariant | Unchanged requirement |
| --- | --- |
| Starts | Exactly five fresh slots; zero warmups, retries, replacements or reruns |
| Workload | Startup plus 39 actions: 40 phases, including ten actual release/revisit cycles |
| Native deadlines | 60 seconds/phase, 660 seconds/native process; separate 120-second setup/export allowances |
| Captures | At most two/start at 2 and 5 seconds from the actual monotonic launch boundary; none after process exit |
| Capture bounds | Two seconds, 8 MiB/file, 1440×900 PNG24; one thread, 64 MiB memory, no map/disk cache |
| RSS | All original gates and hardware predicates; ceiling 243,269,632 bytes |
| Service/cache | One owned service and exclusive 1440×900×24 Xvfb; first uncontrolled observation, then inherited warm-server label |
| Inputs | Same full Mansfield PAN16 4 and RGB16 12 representations and development catalogue |
| Evidence | All failed slots, partial captures and observed timing overhead retained; no subtraction or invented completion |

All inherited numeric bounds, required events, resource diagnostics, seven-event
recovery separation and actual native image inspection requirements still apply.
The fixed new output is
`viewer-acceptance-native-cohort-mansfield-repaired-01` under the approved
`emuella-testdata/artifacts/rareplanes-expanded-v1` store, created only during
later execution with existing notice/lineage handling. The new immutable cohort
ID prevents replay even after failure. Infrastructure failure consumes the cohort
and records unavailable slots; it cannot be called five observed starts.

Before execution, main's committed HEAD must contain the runner/test, both original
protocol files, the new protocol pair and the new build receipt. Only the five
new or edited files need staging. `source_build_receipt_commit` is null because
the new receipt travels with the protocol commit; the execution's full
`protocol_commit` and seven committed file identities bind it without a circular
self-reference.
No future commit hash is invented. The additional repaired-grant fields are:

```json
{
  "protocol_path": "docs/viewer-acceptance-native-repaired-cohort.json",
  "protocol_sha256": "SHA256_OF_MAIN_COMMITTED_REPAIRED_JSON"
}
```

Keep every original grant field, with the exact new output name, actual full
committed HEAD, allocated display/port, stopped authored owner and attribution.
The runner checks the selected digest, committed bytes and grant before any
protected group or process is created. This document and prepared fields are
not an execution grant.

From the repository root, this is the later native command, **not executed here**:

```sh
LD_LIBRARY_PATH=/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib \
WGPU_BACKEND=vulkan VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/nvidia_icd.json \
PYTHONDONTWRITEBYTECODE=1 python3 tools/viewer-acceptance-native-cohort.py execute \
  --protocol docs/viewer-acceptance-native-repaired-cohort.json \
  --protocol-commit "$PROTOCOL_COMMIT" --grant "$GRANT" \
  --output-name viewer-acceptance-native-cohort-mansfield-repaired-01 \
  --display "$DISPLAY_NUMBER" --port "$SERVICE_PORT"
```

## Corrected browser capsule fields, without launch

`$FREEZE/browser-capsule-fields.json` contains the new committed runtime,
build-record identity, complete static asset map, unchanged Chromium 1234,
Playwright 1.62.1, five ordered scenes and original native reference receipt.
Its SHA-256 is
`125ef34b0f99046683a1355900453926149dc91677062b9ea4238d910d05c572`.
`$FREEZE/browser-preparation.json`, SHA-256
`04c8715dcb0b482fd2cbd36302c2f64aecf657493db3f718d9d049c56c76bb2d`,
binds the unchanged browser protocol file map, old capsule and exact budget.
These scratch files contain identifiers and prepared fields, not protected payload.

The fields deliberately leave the fresh service URL/receipt and alias device,
inode and parent mount namespace null. Main observes these in a fresh approved
`viewer-acceptance-browser-masks-repaired-01-execution` group, supplies notice,
rights/native-owner attribution and prior-failure lineage, then writes the final
capsule/grant there. No old PID or alias inode is a new observation.
Follow the unchanged [private alias protocol](viewer-acceptance-browser-environment.md):
mode-0700 approved backing, private bind at `/tmp`, namespace/device/inode/effective
mount checks before Playwright import and every context, no cleanup of backing.
The service catalogue remains
`49dfe454bdcce17dc2212e5de3ee1b130b9b7e92f02f2440dff80f419c67841d`.

One invocation remains nine serial contexts/Workers, 2,309 jobs, 1,883 masks,
47 forward and 47 revisit pressure jobs, zero warmups/retries. All original HTTP,
cache, output, comparison and failure criteria remain unchanged. Deadlines remain
60/job, 660/context, 6,000/proof and 6,100/outer seconds. Canonical plan SHA-256 stays
`45db6fd835feafebcb88aefc410a248943a32a2caf5893288c30f592b6d5def4`.
Main runs read-only `validate` on the final capsule and binds its actual digest
before the one later private-namespace command; no blank-page trial or intermediate
browser call is added. Final viewing/merged qualification remains separate.

## Preparation checks and retained limitations

The focused command is:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 tools/viewer-acceptance-native-cohort.py check \
  --protocol docs/viewer-acceptance-native-repaired-cohort.json
PYTHONDONTWRITEBYTECODE=1 TMPDIR="$FREEZE/tmp" \
  python3 -m unittest discover -s tools/tests -p test_viewer_acceptance_native_cohort.py -v
```

Twenty cohort tests pass, including alternate selection, each inherited policy
leaf's rejection, original-file replacement rejection, grant path/digest/output
binding, seven-file committed identity and unchanged five-start argument lists.
The existing five native diagnostic, seventeen memory and three composed-harness
checks also pass: **45 authored tests**. Build/header checks pass, including
44 rejected response-header cases. Evidence is retained under `$FREEZE/authored`.
The first static check correctly rejected the historical helper hash; its log is
retained, and the explicitly pinned repaired helper passes the final check.

Canonical `cargo xtask verify`, browser smoke, live captures and timing are excluded
by this preparation instruction and remain main-owned. All original results and
protocols are unchanged. Preserve the old proof's **40 pressure failures**
(20 forward, 20 revisit; eleven continuation exhaustions each), its 2,212 successful
normal jobs and the historical **278,794,240 > 243,269,632 bytes** observation.
This freeze proves neither pressure completion nor an RSS repair. The inherited
summary's `repair_implemented: false` remains a resource-repair limitation; the
new runtime/build identities separately identify the authored admission repair.
