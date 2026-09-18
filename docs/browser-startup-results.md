# Browser startup results

Delivery: [Polyorama PR #40](https://github.com/robchristie/polyorama/pull/40).

The retained change is compressed, cache-aware, versioned delivery with finite
startup observation and production artifact checks. The default is unprocessed
bindgen WASM (`--variant none`). No Cargo profile, native rendering, codec,
worker architecture or application test-export feature change remains.

The actual starting checkout was `dc7d3d6a89de0f070eade59bf7da7522de8b57f8`.
The baseline added instrumentation before optimisation; its source revision,
working-diff hash, input/final artifact hashes and flags are retained in the raw
reports. Final artifact identity is in [the manifest](browser-startup-evidence/final-manifest.json);
the eventual merge revision and exact-head verification are recorded on the PR.

## Measured outcome

Milliseconds, median [minimum–maximum]. W is the first workspace submission’s
subsequent rendering-opportunity proxy; C is application-specific first useful
content. Neither establishes physical GPU presentation. Real keyboard/mouse
input and expected visible response were checked after the timed path.

| Scenario | Baseline W | Final W | Baseline C | Final C |
| --- | ---: | ---: | ---: | ---: |
| lab/local/cold | 175.6 [171.2–188.7] | 171.6 [158.1–184.5] | 245.4 [223.5–255.3] | 237.1 [221.3–250.6] |
| lab/local/repeat | 68.1 [58.9–91.3] | 71.8 [67.8–101.8] | 122.6 [111.8–123.9] | 133.6 [130.0–139.6] |
| lab/network/cold | 9552.5 [9548.4–9566.3] | 2621.4 [2614.6–2655.6] | 9806.5 [9804.3–9822.2] | 2787.5 [2776.1–2819.7] |
| lab/network/repeat | 110.7 [101.4–118.3] | 115.8 [112.4–120.0] | 165.9 [153.1–173.4] | 168.5 [162.8–171.3] |
| gallery/local/cold | 165.2 [157.8–188.7] | 156.7 [150.7–161.2] | 158.2 [151.3–181.0] | 150.2 [144.2–154.8] |
| gallery/local/repeat | 66.6 [53.3–69.7] | 72.2 [56.9–72.7] | 57.2 [44.9–60.7] | 62.7 [48.9–63.2] |
| gallery/network/cold | 8829.1 [8824.0–8845.0] | 2486.7 [2479.3–2508.8] | 8822.5 [8817.2–8837.1] | 2478.5 [2472.8–2498.6] |
| gallery/network/repeat | 110.4 [100.0–112.5] | 105.2 [103.3–107.0] | 102.8 [90.5–103.9] | 95.8 [94.9–99.4] |
| viewer/local/cold | 156.6 [151.7–165.1] | 160.0 [156.6–168.6] | 219.1 [209.7–254.3] | 234.8 [230.6–283.9] |
| viewer/local/repeat | 63.2 [49.3–66.7] | 72.3 [61.8–75.9] | 131.1 [117.2–150.3] | 142.6 [138.0–145.3] |
| viewer/network/cold | 9567.3 [9562.6–9571.9] | 2641.8 [2633.2–2652.6] | 10466.3 [10456.3–10476.3] | 3575.5 [3565.9–3585.4] |
| viewer/network/repeat | 97.9 [97.9–97.9] | 114.8 [111.7–121.1] | 923.4 [915.2–931.6] | 965.0 [951.1–983.9] |

Lab’s constrained cold workspace and useful-content medians improve by **72.6%**
and **71.6%** respectively. Gallery corroborates the gain; viewer first content
improves by **65.8%**. All original regression tolerances pass. Local/repeat results
are mixed: viewer local cold content increases by 15.7 ms, within its predeclared
44.6 ms allowance. This is a delivery win, not evidence of universally faster
local startup, isolated compilation or GPU execution.

Lab/Gallery baseline and final each contain five cold/repeat pairs per profile.
Viewer baseline has five local pairs (expanded from two to resolve a decision)
and two network pairs; final has five pairs per profile. All original and added
viewer local samples remain. The fixed network model shares 10 Mbit/s response
body bandwidth and adds 40 ms response latency, including fixture responses.
The cache procedure uses a fresh process/profile for each cold run and ordinary
same-origin, same-URL repeat navigation; only application storage is reset.
OS/GPU caches and compiled-code cache state are uncontrolled.

Environment: Ryzen 9 9950X3D, Linux, Chrome/151.0.7922.34 with Playwright 1.62.1
(exact launch flags in each raw report), 1440×900, **SwiftShader WebGPU**.
Evidence establishes this software-GPU environment only. Hardware WebGPU and
physical presentation timing remain unmeasured. Viewer uses the existing authored
`image-a` fixture; no imagery or codec qualification dataset was regenerated.

## Attribution and choices

- **Retained:** deterministic Brotli/gzip, correct MIME/encoding negotiation,
  immutable content-derived asset directories, revalidating HTML, root/subpath
  loading, finite truthful milestones and phase-labelled failure cleanup.
- **Rejected `-Oz`:** approximately 3.2% smaller Brotli Lab WASM than unchanged
  WASM, but the packaging-only finalist missed the viewer local first-content
  tolerance. The raw failed samples and comparison are preserved.
- **Rejected `-O3`:** less than another 0.1% compressed saving versus Oz and a
  slower viewer cold screen. Pinned Binaryen 131 remains available explicitly
  for consumer-specific measurement; neither variant is a proven default here.
- **Rejected test-export gating:** Lab test actions/snapshots and otherwise
  unused paths saved 0.884% compressed WASM, below the predeclared 1% gate.
  The feature distinction was fully reverted. Useful runtime APIs remain.
- **Rejected structural option B:** preparing the same viewer worker during
  framework startup saved about 20 ms in the screen, below the declared 71 ms
  gate. Production code was reverted. **Option A is deferred:** the shared
  package reuses cached assets, and these measurements do not justify splitting
  dependencies or accepting duplicated common code.

Compression-only screens reuse the exact baseline WASM: Lab constrained cold
workspace fell to about 2.60 s, accounting for almost all the finalist gain.
Binary size alone was never used as startup acceptance. The final raw binary
remains approximately the original size; finite hooks and failure repair add
small differences. No further Cargo/profile tuning was investigated.

| Final WASM | Raw bytes | Brotli bytes | gzip bytes |
| --- | ---: | ---: | ---: |
| gallery/polyorama_gallery_bg.wasm | 10453069 | 2691352 | 3745082 |
| lab/analytical_workspace_lab_bg.wasm | 11328038 | 2842015 | 3991951 |
| lab/polyorama_tile_worker_bg.wasm | 145682 | 43189 | 51034 |
| viewer/emuella_viewer_bg.wasm | 11377181 | 2895289 | 4050753 |

The reports retain browser resource transfer/encoded/decoded sizes, actual HTTP
headers/status/body-byte evidence for page and worker assets, and per-instance
WASM linear-memory capacity. These capacities are not total browser memory.
Main and worker generated loaders completed streaming initialisation; no pure
compile time is inferred by subtracting overlapping intervals. Cold Brotli
responses and repeat cached assets corroborate the transfer effect.

Median available WASM capacities, final local cold samples (bytes):

| App | Main at first content | Worker at first decode |
| --- | ---: | ---: |
| lab | 7733248 | 1572864 |
| gallery | 5636096 | No worker |
| viewer | 6619136 | 4784128 |

Independent review found that a restored Results-only layout could be destroyed
while waiting for an image milestone. The repair completes usable readiness
without requiring imagery, records later content once, and pauses the watchdog
in hidden tabs. A real saved-layout browser regression reproduces the old failure
and passes after repair, including advancing past the watchdog and physically
selecting an image tab. Final timing cohorts were refreshed for the changed host
assets; `pre-review-*.json.gz` preserves the earlier successful cohorts.

## Verification and reproduction

`cargo xtask verify` passed locally; [verification identity](browser-startup-evidence/verification.json)
records the environment and artifact match. The canonical surface includes
native/WASM builds, lint/tests, architecture, unchanged UI snapshots and existing
browser/native smoke. It additionally checks final production packaging, startup
completion/failure and real input without CI timing thresholds. The PR records
its exact candidate, CI and post-merge outcomes.

Focused startup/package/worker suite: 39 tests passed. Final production root and
nested subpath, WebGPU-unavailable and worker-failure checks passed, including
resource cleanup and a normal cached revisit across two builds. The final viewer
response adapter rejected 44 invalid cases and admitted its valid case. Final
viewer real-wheel checks matched **440 decoded-region observations** against
baseline, with bounded resources. Worker operation elapsed includes transport
and caching; isolated decoder throughput is unavailable.

The broader existing viewer smoke exercised normal image switching/bookmarks,
cancellation and bounds. Its pressure eviction assertions fail for baseline and
candidate alike because these small fixtures do not fill the 1 MiB compressed
or 16 MiB GPU caches. The assertions and fixtures were not changed. This is a
qualification limit, not passing eviction evidence; see
[fixture smoke details](browser-startup-evidence/fixture-smoke.json).

```sh
bash tools/install-binaryen.sh
cargo xtask build-browser-production --variant none
node tools/browser-serve.mjs --directory target/browser-production --port 4173
node tools/browser-startup-benchmark.mjs --directory target/browser-production \
  --output .tools/runtime/startup-final --pairs 5 --apps lab,gallery \
  --profiles local,network
node tools/browser-startup-benchmark.mjs --directory target/browser-production \
  --output .tools/runtime/startup-viewer --pairs 5 --apps viewer \
  --profiles local,network --viewer-upstream http://127.0.0.1:8123
node tools/browser-startup-smoke.mjs target/browser-production PREVIOUS_PACKAGE
node tools/browser-startup-compare.mjs BASELINE/results.json FINAL/results.json
cargo xtask verify
```

Serve the existing fixture with `emuella-viewer-tools serve --representation
/absolute/path/to/existing/image-a --listen 127.0.0.1:8123 --verify-payload true`.
Use `--encoding identity` on the same benchmark package to isolate compression.
For optimizer comparisons, build with explicit `--variant Oz` or `--variant O3`
into separate `--output` directories. Full environment/tool installation and
consumer hosting/profile guidance is in [the recipe](browser-startup.md).

Compact raw `.json.gz` samples, predeclared gates, rejected screens, comparisons,
fixture identities and the final manifest live in `browser-startup-evidence/`.
Screenshots and complete local logs stay in ignored startup scratch; the raw
reports retain screenshot hashes and visible-response checks. No old codec or
image qualification result is promoted to evidence for these artifacts.
