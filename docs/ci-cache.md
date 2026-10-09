# CI verification and cache ownership

The required `verify` check aggregates eight parallel qualification stages. The
closed inventory in `tools/verification-stages.json` is shared with
`cargo xtask verify`, which runs the same stages sequentially locally. A stage
can be reproduced with `cargo xtask verify-stage <name>`.

| Stage | Required surface |
| --- | --- |
| `checks` | Tool regressions, plans/tokens, formatting, native/WASM lint, workspace tests, rendered API documentation and selected doctests, native/WASM example compilation, architecture, no-default-feature UI tests and browser-tool regressions |
| `native-lab` | Lab library/binary and minimal example in one release invocation; Lab and minimal native smokes; Lab/native interface journey |
| `native-other` | Every remaining workspace release library/binary; Gallery, icon, navigation and status-chip native smokes |
| `browser` | Development Lab/Gallery/Worker/Viewer release WASM and bindgen packages; Lab browser smoke; Lab/browser interface journey |
| `gallery-browser` | Shared UI Gallery release WASM/bindgen builder; Gallery, icon, navigation and status-chip browser smokes |
| `production` | Separate production release WASM build, packaging and response-header validation, startup smoke and the prescribed startup benchmark |
| `ui` | Gallery release WASM/bindgen package and every deterministic fixture/baseline check |
| `record-desk` | Independent workspace boundary/fmt/native and WASM lint/tests/release builds, both smokes and both interface journeys |

The two native package selections cover the previous workspace release targets.
The Lab's example is selected with its normal library and binary to avoid a
second shared-framework build pass. Record Desk retains its independent Cargo
workspace and dependency boundary. Product release settings remain ThinLTO with
one codegen unit; parallel qualification does not substitute a faster profile.

`tools/verify.py` remains the Git-derived scope guard. Only a successful docs
classification may select documentation verification instead of the eight
stages. The final job runs even after failure and cancellation; it requires the
selected route to succeed and rejects missing, failed, cancelled or unexpected
job results. Full qualification also checks the observed four app/host interface
reports, their clean source revision and their exact stage selections. Interface
stages retain separate summaries under
`.tools/runtime/verification-evidence/interface-<stage>/application-interface/`.
The standalone interface smoke still selects all four journeys by default.
Each stage uploads its qualification evidence separately for 14 days.

Browser Tab traversal waits for the application's `tab_input_epoch` receipt
before sending another key. The diagnostic counts keydowns once per egui input
frame, including discarded layout passes without double-counting. Fixed pacing
alone can leave keys queued behind slow frames; the physical focus, activation,
capture and idle assertions and the existing action limits remain required.

## Rust dependencies

`Swatinem/rust-cache@v2` owns the Cargo registry and dependency build artefacts.
Each build workload has its own key and caches only target directories created
by its commands; only Record Desk includes the independent consumer target. The docs
route can restore the checks-stage cache. `gallery-browser` restores the UI
archive and is never a writer: both lanes invoke the exact same Gallery Cargo
and bindgen builder, and the Gallery smokes add no Cargo builds. UI owns that
shared successful-main archive. The production stage's recursive
`target` cache includes `target/browser-cargo`. Cleanup removes workspace-crate
and incremental output; every restored build still runs Cargo freshness checks.
A hit never skips compilation, testing, packaging or runtime qualification.

The disposable hosted runner first selects the repository's pinned Rust
**1.99.0**, then removes unrelated image-installed toolchains. The normalisation
script refuses developer and self-hosted environments and verifies the active
compiler before removing anything. Cache compatibility retains OS, architecture,
compiler, Cargo/build environment, manifests, lockfiles and configuration.

Ordinary PRs restore dependency archives without saving. Successful main stages
save their own dependency archives; a successful stage does not establish that
the complete workflow passed. Archives represent that stage's complete workload
rather than a partially completed serial verifier. This replaces the former
single full-main-job cache owner. Failed stages and prose-only jobs do not save.
Caches remain disposable: misses and service errors require ordinary builds.

Generated `target/browser-staging` and `target/browser-production` trees are
removed before caching so restored directory skeletons cannot be mistaken for
owned packages. `target/browser-cargo` retains the required dependency reuse.

## Tools

The dedicated `actions/cache` entry owns only `~/.cargo/bin/wasm-bindgen`, keyed
by version, OS and architecture. The installer checks the executable's exit
status and exact **0.2.127** version, installs on a miss/mismatch, and checks
again. The browser stage owns successful main saves of a missing binary entry;
Rust archives exclude Cargo binaries. `setup-node` manages npm's download cache.
Only stages requiring Chromium install its runtime; Node regressions do not
launch a browser. Pinned Binaryen remains installed where packaging/tool tests
require it.

## Calibration and limits

The owning CI performance PR retains run IDs, source/toolchain identities,
per-stage timings, restore/save sizes and the retain-or-reject decision. Measure
workflow elapsed time including classification, setup, compilation, runtime
checks, uploads, cache saving and final aggregation. A successful warm rerun
must also be followed by a representative changed-source warm run. Different
profiles, source surfaces and dependency states are not interchangeable.

Provisional calibration populated successful stage archives in this task's PR
scope to permit fresh-runner warm measurement before landing. The final workflow
restores those entries but no longer writes PR archives. GitHub's scope boundary
keeps those entries out of main; post-merge qualification must populate the
ordinary main-stage archives separately.

Five minutes is the cached-run engineering target, not permission to truncate
qualification or a guarantee of hosted queue/service latency. The CI plan owns
remaining critical-path work until the measured outcome is reconciled.
