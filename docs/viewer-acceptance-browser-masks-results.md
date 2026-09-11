# Frozen browser mask proof result

Status: **partial**, `quality-rejected-diagnostic-only`. The authorised single
invocation returned exit code **2** after **2.669 seconds**.
All nine serial persistent-context launch attempts failed before Worker
initialisation. There were no additional invocations or retries.

Protocol: `8c5d1e84176a43ad9cfccc857a0cf3a164e3367d`. Actual frozen web runtime:
`fe82f85984fba5c75d7a5a8f10b2d914b773a838`. Chromium revision-directory 1234 and Playwright
1.62.1 were pinned; the documented Chromium/sysroot library path was used.
The observed browser version, Worker backend, GPU backend and adapter are
unavailable because no usable browser context was established.

The common retained error is
`browserType.launchPersistentContext: Target page, context or browser has been closed`.
The frozen runner bounds each error to 1024 characters; launch arguments occupy
the remaining retained text, so it does not establish the underlying browser
exit cause. Every failed context and fresh profile is preserved.

| Observation | Planned | Actual |
|---|---:|---:|
| Invocation | 1 | 1 |
| Serial context launch attempts | 9 | 9 |
| Usable browser contexts / Workers | 9 / 9 | 0 / 0 |
| Worker jobs | 2,309 | 0 |
| Catalogue mask comparisons | 1,883 | 0 |
| Required missing/corrupt/stale faults proved | 3 | 0 |
| Pressure forward / revisit jobs | 47 / 47 | 0 / 0 |
| Proof HTTP requests / body bytes | Bounded by protocol | 0 / 0 |

No sample or validity values were compared. Agreement, disagreement,
false-valid and false-invalid counts are therefore unavailable, not measured
zero-error results. Mask eviction, refetch and cache bounds remain unproved.
All five scene contexts, three fault contexts and the pressure context retained
`context-operation-failed`; the terminal reason is `incomplete-context-proof`.

The reviewed workspace source-coverage record, notice and all five native parent
identities were verified. The existing five immutable representations were served
in frozen order without preparation. Service source revision
`4a1594b0840eadae26a4d6005d50d9201734a1a7` remains distinct from the web runtime; its binary
SHA-256 is `7ea60902498fbbfcac67f7d7785e42a38b4ce06741895ad916f8996c0f137102`.
The binary was checked before launch, against `/proc/PID/exe`, and after shutdown.
One service-preflight catalogue response delivered **148,937 bytes**;
its five manifests matched native evidence. These bytes are separate from proof
HTTP accounting. Catalogue SHA-256:
`49dfe454bdcce17dc2212e5de3ee1b130b9b7e92f02f2440dff80f419c67841d`.

Capsule validation confirmed exactly 9 contexts, 2,309 jobs, 1,883 catalogue
comparisons and 47+47 pressure jobs. The exact committed protocol and grant were
validated before invocation. Limits stayed at 60 seconds/job, 660/context,
6,000/invocation and a separate 6,100-second process timeout. No deadline was
reached. Page/Worker timing is unavailable; elapsed process time is not latency
or speed evidence. The runner's final immutable-input recheck passed.

Evidence group: `/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-browser-masks-fullscene-20260911t072423z`.
Execution capsule, grant, service receipt, bounded logs and cleanup receipt:
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/viewer-acceptance-browser-masks-fullscene-20260911t072423z-execution`.

- Terminal result SHA-256: `8ae77fbb49b1cf699e62fa57845d3e4795657bff7d7c2217bd5bd241126ab50a`.
- Capsule SHA-256: `9062781ea078709bf384ed8b421006f5c1a12217dae066d9696238d7b005a44a`.
- Grant SHA-256: `dc6973d6f307c4fa08218d8e58468340548136e631f4844cd298292251786bae`.
- Canonical plan digest: `45db6fd835feafebcb88aefc410a248943a32a2caf5893288c30f592b6d5def4`.

The [JSON aggregate](viewer-acceptance-browser-masks-results.json) pins every
context result and the retained plan, inputs, comparison journal, receipts and
logs by absolute path, byte length and SHA-256. The plan digest above hashes the
canonical compact plan; the JSON aggregate separately pins the saved plan file.

The owned service was stopped and its PID is absent. No browser process using
this invocation's profile paths remained. The unrelated authored service on
port 8123 was left untouched. No source, runtime or protocol changes, preparation,
commits or pushes were performed. Main owns inspection, review, canonical
verification and any commit. Actual hardware UI, normal latency and inherited
recovery remain unmeasured; this partial result selects no configuration and
establishes no viewer acceptance.
