# Production browser builds and startup measurement

Stobie's browser packaging is a small post-`wasm-bindgen` step, usable without
a frontend bundler. One implementation, the `stobie-build` crate, does it for
every application: `stobie build-web` and `stobie package-web` for
applications in their own repositories (see [Start a new app](start-a-new-app.md)),
and `cargo xtask build-web`, `cargo xtask build-browser-production` and
Record Desk's `build-web.sh` here. Every build writes the same startup loader,
`browser-startup.js`, beside `index.html`; its source is
`crates/stobie-build/web/browser-startup.js`, embedded in the CLI, and nothing
copies it by hand. The Lab, Gallery and viewer use it.
The production default retains unprocessed bindgen WASM and adds compressed,
versioned delivery. Pinned `wasm-opt` variants remain explicit measurement options;
neither passed every finalist gate. Native release settings and the viewer's
image/JPIP contracts are unchanged.

## Build and serve

Use the repository's Rust **1.99.0** toolchain, the `wasm32-unknown-unknown` target, `wasm-bindgen-cli`
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

The Lab's [managed private development preview](private-preview.md) serves that
development output through an inherited gateway socket with `no-store` and
browser reloads. It does not alter production packaging or its cache policies.

Production starts with clean bindgen staging (each web root's own files, a
fresh loader and fresh bindgen output, never a development build's), optionally
post-processes WASM, compresses assets and validates the **final** viewer WASM
response adapter. Output contains:

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
tools (the packager's version, which fixes its Brotli and gzip encoders, and
`wasm-opt` when used), input hashes and final raw/compressed hashes and sizes. Identity includes
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
node tools/browser-serve.mjs --directory /srv/stobie/new \
  --retain-directory /srv/stobie/previous --base-path /preview/
```

Retention is explicit: only previous manifests' immutable assets are served;
previous HTML never overrides current entry points. Keep the previous directory
for your chosen retention period. Rebuilding over that same directory destroys
its previous output and is therefore inappropriate for a retained deployment.
This work does not deploy or publish a site.

## Renderer backends

`WebHost::start`, which every `AppHost` application uses through
`web_handle!` or its own handle, chooses the GPU backend. Applications need no
code for it.

- **Default.** The first attempt uses eframe's unchanged `WebOptions`, so
  WebGPU starts exactly as before wherever `navigator.gpu` returns an adapter.
  Without `navigator.gpu`, or when `requestAdapter()` returns null, wgpu
  starts on its GL backend, which is WebGL2. If WebGPU had an adapter but
  failed to start, for example in `requestDevice`, the host replaces the
  canvas with a fresh copy (same ID and attributes) and retries once on
  WebGL2. A canvas keeps its first context type, so the retry needs a new
  canvas.
- **Override.** `?renderer=webgl2` or `?renderer=webgpu` in the page URL forces
  one backend with no fallback. Any other value fails startup, naming the
  accepted values.
- **No backend.** When nothing starts, the start promise rejects with one
  message covering both backends, for example "No supported renderer: WebGPU
  is unavailable (no navigator.gpu); WebGL2 failed: …". The loaders show it as
  a `framework_start` failure.

The host reports what it started on in three places:
- the canvas attributes `data-stobie-renderer` (`webgpu` or `webgl2`),
  `data-stobie-renderer-adapter`, `data-stobie-renderer-selection`
  (`preferred`, `fallback` or `forced`) and `data-stobie-renderer-reason`;
- the startup report's `renderer` object, with the same four fields;
- the optional `renderer` field of the application interface's `hello`.

Native hosts report wgpu's backend name in `hello` too, with `forced` when
`WGPU_BACKEND` is set. `stobie_ui_egui::renderer_identity` gives an application
the same identity.

Both backends render the same output on Chromium, text and colour included,
because both configure an `Rgba8Unorm` surface and egui applies its own gamma.
Rasterisers can differ by a few levels per channel; the startup smoke checks
the stated tolerance. Software WebGL2 through ANGLE and SwiftShader is slower than
hardware, and WebGL2 offers no GPU timing. The Lab's scalar image renderer
runs on both backends unchanged; see the
[image composition guide](image-application-composition.md). The Emuella viewer is not on
`AppHost`: its bootstrap passes `requireWebGpu` to `startApplication` and
still fails without `navigator.gpu`.

## Firefox

`node tools/record-desk-firefox.mjs` runs Record Desk in Playwright's pinned
Firefox and operates it through the ARIA layer: select a record, set its
notes, Apply and Undo. Install Firefox with `npx playwright install --with-deps
firefox`, then run the check headful with `STOBIE_FIREFOX_HEADFUL=1` and a
`DISPLAY`, because headless Firefox offers no WebGL2. Firefox 153 on Linux has
no `navigator.gpu`, so Record Desk starts on its WebGL2 fallback.

Playwright's Firefox clicks at whole CSS pixels, and the ARIA layer's hit
target is one CSS pixel at the control's centre. A plain `locator.click()`
can therefore land beside it and reach the canvas instead. The check activates
controls with `element.click()`, the layer's assistive technology route.
`cargo xtask verify` installs only Chromium and does not run this check.

## Finite milestones

`window.__STOBIE_STARTUP` records navigation-relative milliseconds and epoch
milliseconds. Workers supply their own `timeOrigin`, durations and epoch stamps;
never compare unnormalised page and worker `performance.now()` values.

| Milestone | Definition |
| --- | --- |
| `bootstrap` | Shared host helper begins, after bootstrap module imports. |
| `wasm_init_begin/end` | Dynamic glue import and generated init, through completed instantiation; overlaps streaming transfer/compilation. |
| `framework_start_begin/end` | WebHandle construction/start call through promise resolution. |
| `application_construct_begin/end` | Rust application constructor boundaries within framework startup. |
| `workspace_ui_complete` | First application UI frame handed back to eframe on the CPU. |
| `workspace_frame_submitted` | First observed `GPUQueue.submit` after that UI handoff. Unavailable on WebGL2, which has no `GPUQueue`, and when the temporary hook cannot be installed; `submissionObservation` says which. |
| `rendering_opportunity_proxy` | One subsequent requestAnimationFrame callback; a rendering opportunity only. |
| `worker_ready` | Main realm receipt of worker initialisation readiness; viewer still fetches its catalogue afterwards. |
| `first_useful_content` | Lab observes actual renderer draws plus resident image bytes; Gallery has constructed its component content; viewer has prepared nonzero rendered regions. |
| `failure` | Terminal phase-labelled initialisation failure, including no supported renderer (or unavailable WebGPU for the viewer), an invalid renderer override, worker/init/transport failures and the bounded timeout. |

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

On Linux, headless launches through `hostedLinuxWebGpuLaunchOptions()` in
`tools/browser-launch.mjs` pass `--use-gl=angle --use-angle=swiftshader`. This
covers every headless caller of that helper, not only the startup tools. Without
an explicit GL choice, `chrome-headless-shell` falls back to software compositing
(`--use-angle=swiftshader-webgl`). A WebGPU canvas cannot allocate its
swap-chain image there, so nothing presents and the device is lost. The smoke
and benchmark check Chromium's reported GPU compositing status at launch and
stop with that status, rather than waiting for content that cannot appear.
Headful runs (`STOBIE_BROWSER_HEADFUL=1`), as in CI, and headless runs on other
platforms keep the browser's default GL selection.

On Linux the existing launch helper uses SwiftShader WebGPU. Set
`STOBIE_BROWSER_HEADFUL=1 DISPLAY=:<your-X-display>` where needed for meaningful
readback. The benchmark and smoke launch Chromium with the repository UI
sysroot when `tools/bootstrap-linux-ui.sh` has provisioned it, as
`cargo xtask verify` does; `STOBIE_USE_SYSTEM_UI_LIBS=1` selects the host's
libraries instead. Evidence records browser version/flags, UI library source,
actual adapter/backend, hardware, viewport, artifact identity, resources,
headers, streaming calls and per-instance memory. Software rendering establishes only that environment.

`STOBIE_BROWSER_RENDERER=webgl2` makes the same launch helper omit its
WebGPU-only flags (unsafe WebGPU, the SwiftShader adapter, the Dawn features
and the WebGPU developer features). With SwiftShader, Chromium then keeps
`navigator.gpu` but returns no adapter. A hardware Vulkan adapter can still
offer WebGPU, so smokes also open pages through `rendererUrl()`, which adds
`?renderer=webgl2`. `assertBrowserRenderer(page)` checks the canvas's reported
backend, so a WebGPU run cannot pass for a WebGL2 one. The `webgl2`
verification stage runs these on WebGL2:
- the Lab, Gallery and Record Desk browser smokes;
- their ARIA layer checks;
- the Lab's browser interface journey;
- Record Desk's browser CLI parity.

Besides the WebGPU startup checks, the startup smoke starts the Lab and the
Gallery on WebGL2 in four ways:
- **fallback:** without `navigator.gpu`;
- **no-adapter fallback:** with `requestAdapter()` returning null;
- **device fallback:** with `requestDevice` rejected;
- **forced:** with `?renderer=webgl2`.

Each run checks the reported backend, selection and reason, that the
submission milestone is unavailable, and that there is still one canvas.
`?renderer=webgpu` on the Gallery must start on WebGPU and report it as
forced. The smoke also compares screenshots of the Gallery and of the Lab,
image panes included, on both backends: at
most 0.1% of pixels may differ by more than 8/255 in any channel. Local
headless SwiftShader showed a largest difference of 1/255, and hosted headful
CI one of 8/255, with no pixel beyond the threshold.
Startup failures cover:
- neither backend (no `navigator.gpu` and no `webgl2` context), for both
  apps;
- an invalid override, and a forced WebGPU without `navigator.gpu`, for the
  Gallery;
- the viewer's WebGPU requirement;
- worker failures.

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
settings are not inherited. Start with release/thin-LTO/one-codegen-unit, as
`stobie new` does, and the same pinned post-bindgen steps; benchmark your
actual decoder and workload before choosing optimisation flags. Do not apply
blanket size tuning to compute code.

`stobie build-web --production [--variant none|Oz|O3] [--output DIR]` packages
one application. `stobie package-web NAME=DIR... --output DIR` packages
complete bindgen web roots built some other way, such as an application and a
worker built from separate crates, with an optional
`--source-identity-file` recording the build configuration. Both write the
manifest and layout above. `tools/browser-serve.mjs` (`createProductionServer`)
is independent of Lab state; copy or adapt it with its tests when you need a
reference production server. Add constructor, first-workspace and
meaningful-content hooks at your application boundary: an application on
`AppHost` reports construction and its first complete UI pass to the loader
without code, and can call `window.__stobieStartupMark('first_useful_content')`
itself. Keep workers, data/transport policy and framework dependencies outside
Stobie core.
