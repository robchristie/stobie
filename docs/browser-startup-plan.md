# Browser startup efficiency

Status: complete

Starting revision: `dc7d3d6a89de0f070eade59bf7da7522de8b57f8` (actual clean checkout).
Owner: current implementation task, branch `codex/browser-startup-efficiency`.
Scope: this repository's Lab, Gallery, viewer, build/hosting tools and consumer recipe.
No consumer repositories, codecs, datasets or release/deployment operations change.

## Acceptance and dependency order

1. Define finite host/application/worker milestones and capture an instrumented,
   otherwise unoptimised baseline. Validate usable workspace through real input.
2. Add deterministic pinned post-bindgen packaging, compression and versioned
   static delivery; retain deterministic no-store smoke servers.
3. Compare unchanged, size and speed candidates using identical workload and
   delivery. Audit test-only exports; retain gating only for measured benefit.
4. Select at most one structural optimisation if traces justify it; otherwise
   document deferral. Preserve native, rendering, worker and codec contracts.
5. Verify final packages, failures, root/subpath, input, native/WASM and canonical
   checks. Independently review exact head, land, check post-merge CI and clean up.

## Calibration

Question: which build/delivery changes reduce usable-workspace and first-content
latency without buying an unaccepted regression in runtime or first image?
Smallest probe: Lab default 1440×900 workspace, Gallery default shell, existing
viewer deterministic fixture. Lab is primary; five fresh-process cold/repeat
pairs per delivery profile at baseline/final; smaller candidate screens.
Profiles: unthrottled local and shared 10 Mbit/s response-body budget, 40 ms response latency.
Evidence owner: `docs/browser-startup-evidence/`, raw machine-readable samples;
builds/logs/screen images in ignored `.tools/runtime/browser-startup/`.
Exit: establish baseline variability and declare numeric gates before judging
candidates. Select one coherent final configuration or reject unsupported changes.

## Closeout

Delivery: [Polyorama PR #40](https://github.com/robchristie/polyorama/pull/40).

The selected final configuration retains unprocessed bindgen WASM with
compressed/versioned delivery. All declared baseline regression gates and the
primary network improvement gate passed; Lab cold workspace/content medians
improved by 72.6%/71.6% in the fixed response profile. Local/repeat outcomes are
mixed within the declared tolerances; no universal local speedup is claimed.

Oz/O3, Lab test-export gating and the single worker-preparation overlap candidate
were rejected against predeclared gates. No structural change remains. Final
root/subpath, failure cleanup, normal revisit, real-input and viewer output/bound
checks are recorded in [the compact report](browser-startup-results.md) and
`browser-startup-evidence/`. Exact-head review, canonical/CI qualification and
merge/cleanup evidence remain on the delivery PR under the existing workflow.

Independent review identified and repaired the restored non-image-layout watchdog
regression. Usable readiness and optional first content are now separate; hidden
time does not consume the startup watchdog. New artifacts passed refreshed
five-pair timing gates and the saved-layout/late-content regression.
