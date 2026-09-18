# Production browser builds and startup measurement

Polyorama's browser packaging is a small post-`wasm-bindgen` step, usable without
a frontend bundler. The Lab, Gallery and viewer use the same host startup helper.
The production default retains unprocessed bindgen WASM and adds compressed,
versioned delivery. Pinned `wasm-opt` variants remain explicit measurement options;
neither passed every finalist gate. Native release settings and the viewer's
image/JPIP contracts are unchanged.

## Build and serve

Use Rust **1.97.1**, the `wasm32-unknown-unknown` target, `wasm-bindgen-cli`
**0.2.127**, Node **25.8.2**, and Binaryen **131**. On Linux x86-64:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.127
bash tools/install-binaryen.sh
cargo xtask build-browser-production
node tools/browser-serve.mjs --directory target/browser-production --port 4173
```

Open `/lab/`, `/gallery/` or `/viewer/`. The viewer also requires its existing
same-origin catalogue/descriptor/mask/JPIP service; the static server does not
publish imagery. The benchmark's optional `--viewer-upstream` forwards only those
existing data routes to a loopback fixture service with its original headers.

The repository-local installer verifies a pinned archive SHA-256. On other
platforms install the corresponding [Binaryen 131 release](https://github.com/WebAssembly/binaryen/releases/tag/version_131)
and set `WASM_OPT=/absolute/path/to/wasm-opt`. Explicit optimisation fails on a
missing or incompatible tool. `--variant none`, `--variant Oz` and `--variant O3`
select the unprocessed, size-oriented and speed-oriented comparison variants.
The selected default and measured results are recorded in
[browser-startup-results.md](browser-startup-results.md).

`cargo xtask build-web` remains the convenient development and automation build;
its existing smoke server continues to use `Cache-Control: no-store`. An explicit
Lab test-export feature was screened but rejected: it saved only 0.884% of the
Brotli WASM, below the predeclared 1% gate for another build distinction. The Lab
exports therefore remain, as do Gallery's useful catalogue/configuration/snapshot
APIs. Runtime diagnostics, persistence, keyboard handling and accessibility are
unchanged.

Production starts with clean bindgen staging, optionally post-processes WASM, compresses
assets and validates the **final** viewer WASM response adapter. Output contains:

```text
target/browser-production/
  manifest.json
  lab/index.html                    mutable entry point (+ .br/.gz)
  lab/assets/<content-sha256>/       coherent JS/WASM/CSS/helper/worker package
  gallery/index.html
  gallery/assets/<content-sha256>/
  viewer/index.html
  viewer/assets/<content-sha256>/
```

Every asset has deterministic Brotli quality 11 and gzip level 9 representations,
verified by decompression round trips. The manifest records source/configuration,
tools, input hashes and final raw/compressed hashes and sizes. Identity includes
final contents and configuration; a source commit alone is insufficient. Glue
imports are preserved; HTML's relative base and bootstrap-resolved worker URL
keep all references within a single version under root or subpath hosting.

```sh
node tools/browser-serve.mjs --directory target/browser-production \
  --base-path /preview/ --port 4173
# Open http://127.0.0.1:4173/preview/lab/
```

## Hosting and publication

The example server exercises ordinary `Accept-Encoding` negotiation, including
q-values, `br`, `gzip` and uncompressed fallback. Serve WASM as
`Content-Type: application/wasm` even when compressed, send the corresponding
`Content-Encoding`, and `Vary: Accept-Encoding`. Generated loaders use streaming
initialisation; the startup report observes actual streaming calls/completions.
Immutable **public static assets only** receive
`Cache-Control: public, max-age=31536000, immutable`; mutable HTML receives
`Cache-Control: no-cache` and an ETag. Do not apply these public policies to
private imagery, APIs, JPIP or other data responses.

Build into a new directory, upload all of its versioned assets, then atomically
replace/repoint mutable HTML. Retain previous versioned assets for already-open
pages and lazy workers. A normal revisit revalidates HTML and sees one coherent
new package. With the example server, use distinct directories and switch the
server atomically through your existing deployment supervisor:

```sh
node tools/browser-serve.mjs --directory /srv/polyorama/new \
  --retain-directory /srv/polyorama/previous --base-path /preview/
```

Retention is explicit: only previous manifests' immutable assets are served;
previous HTML never overrides current entry points. Keep the previous directory
for your chosen retention period. Rebuilding over that same directory destroys
its previous output and is therefore inappropriate for a retained deployment.
This work does not deploy or publish a site.

## Finite milestones

`window.__POLYORAMA_STARTUP` records navigation-relative milliseconds and epoch
milliseconds. Workers supply their own `timeOrigin`, durations and epoch stamps;
never compare unnormalised page and worker `performance.now()` values.

| Milestone | Definition |
| --- | --- |
| `bootstrap` | Shared host helper begins, after bootstrap module imports. |
| `wasm_init_begin/end` | Dynamic glue import and generated init, through completed instantiation; overlaps streaming transfer/compilation. |
| `framework_start_begin/end` | WebHandle construction/start call through promise resolution. |
| `application_construct_begin/end` | Rust application constructor boundaries within framework startup. |
| `workspace_ui_complete` | First application UI frame handed back to eframe on the CPU. |
| `workspace_frame_submitted` | First observed `GPUQueue.submit` after that UI handoff. Unavailable if the temporary hook cannot be installed. |
| `rendering_opportunity_proxy` | One subsequent requestAnimationFrame callback; a rendering opportunity only. |
| `worker_ready` | Main realm receipt of worker initialisation readiness; viewer still fetches its catalogue afterwards. |
| `first_useful_content` | Lab observes actual renderer draws plus resident image bytes; Gallery has constructed its component content; viewer has prepared nonzero rendered regions. |
| `failure` | Terminal phase-labelled initialisation failure, including unavailable WebGPU, worker/init/transport failures and the bounded timeout. |

No milestone proves GPU completion or physical presentation. The report does not
subtract transfer duration from streaming init and call the remainder compile
time. It samples each WASM instance's linear-memory capacity at finite boundaries;
this is **not total browser memory**. Missing observation is unavailable, not a
zero-memory assertion. Loading messages name real phases without percentages;
workspace access does not wait for imagery. Usable `ready` status requires the
workspace opportunity and required worker readiness; optional first image/content
may arrive later or remain unavailable in a saved Results-only layout. The
startup watchdog counts foreground time and pauses while a tab is hidden.
Genuine initialisation failures still clean up the failed handle; later worker
failures use the existing runtime error path.

## Benchmark and verification

```sh
npm ci
npx playwright install chromium
node tools/browser-startup-benchmark.mjs \
  --directory target/browser-production --output .tools/runtime/startup-final \
  --apps lab,gallery --pairs 5 --profiles local,network
node tools/browser-startup-smoke.mjs target/browser-production
node tools/browser-startup-compare.mjs BASELINE/results.json FINAL/results.json
cargo xtask verify
```

On Linux the existing launch helper uses SwiftShader WebGPU. Set
`POLYORAMA_BROWSER_HEADFUL=1 DISPLAY=:<your-X-display>` where needed for meaningful
readback, and supply the normal library environment on hosts using the bundled
UI sysroot. Evidence records browser version/flags, actual adapter/backend,
hardware, viewport, artifact identity, resources, headers, streaming calls and
per-instance memory. Software rendering establishes only that environment.

Each cold visit starts a new Chromium process/profile. Its repeat revisits the
same origin, URLs and profile with ordinary caching. The harness restores empty
application storage before both visits without clearing HTTP caches, disabling
caches, interception or query cache-busting. This does not reset OS/GPU caches
or guarantee compiled-code cache state. The two profiles are unthrottled local
delivery and a shared 10 Mbit/s response-body budget with 40 ms response latency.
The budget covers page, worker and proxied fixture responses. This is a documented
local network model, not a claim about a particular WAN connection.

Raw `results.json` contains all samples; medians and min/max describe spread,
without small-sample tail claims. Real input and its expected rendered response,
screenshots/readback and runtime checks happen outside the startup timing path.
Use `--encoding identity` against the same package to isolate compression/delivery
from binary changes. Viewer evidence uses `--apps viewer --viewer-upstream
http://127.0.0.1:8123` with an **existing** deterministic fixture service; no dataset
regeneration is part of this command. Timings are opt-in evidence, never CI
wall-clock gates. CI checks completion, failures, package contracts and input.

## Consuming applications

Configure profiles in the **consuming workspace**: dependency Cargo profile
settings are not inherited. Start with release/thin-LTO/one-codegen-unit and the
same pinned post-bindgen steps; benchmark your actual decoder and workload before
choosing optimisation flags. Do not apply blanket size tuning to compute code.

`packageBrowser({apps:[{name,directory}],output,variant,sourceIdentity})` in
`tools/browser-package.mjs` accepts complete bindgen web roots. The companion
`createProductionServer` and `tools/browser-startup.js` are independent of Lab
state. Copy or adapt these host/build tools and their tests into the consuming
application, record its own tool/configuration identity, and add constructor,
first-workspace and meaningful-content hooks at its application boundary. Keep
workers, data/transport policy and framework dependencies outside Polyorama core.
