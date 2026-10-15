# WebGL2 fallback for Stobie web apps

Status: active
Next action: Land increment 1, then start increment 2 (the Lab's image renderer on WebGL2).

## Outcome and boundaries

Every application on `AppHost` starts and works on WebGL2 when WebGPU is
unavailable, with no application code. WebGPU stays preferred and the host
says which backend is running. The scalar image renderer's WebGL2 outcome is
proven and recorded.

- Where WebGPU is available, nothing changes: the same eframe options and
  backend, the same output, UI snapshot baselines and startup budgets.
- Use wgpu's GL backend through eframe's existing `wgpu` renderer. Do not add
  the glow renderer or a second UI renderer, and do not patch or fork eframe,
  egui or wgpu.
- The ARIA layer, capability IDs, semantic IDs and the `stobie` CLI behave the
  same on both backends. The inspection protocol changes only additively.
- No unconditional repaint; idle behaviour holds on WebGL2.
- The Emuella viewer is not on `AppHost` and keeps its WebGPU requirement.

## Probe

Run on 2026-10-15 against main `a346980` (tree-identical to `e2ad856`) with
Playwright Chromium 151.0.7922.34, headless with ANGLE on SwiftShader. A local
server removed the two JavaScript `navigator.gpu` gates; Rust was unchanged.
Raw results stay under ignored `.tools/runtime/probe/`.

- **Backend selection.** eframe 0.36.1's default `WgpuSetup` requests
  `PRIMARY | GL` and calls `wgpu::util::new_instance_with_webgpu_detection`.
  This drops WebGPU when `navigator.gpu` is missing or `requestAdapter()`
  returns null, and then wgpu picks GL. With `navigator.gpu` removed, and with
  the WebGPU-only Chromium flags omitted (`navigator.gpu` present, no adapter),
  Record Desk, the Gallery and the Lab all started on
  `Gl` / "ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device …))", reported as
  "WebGL 2.0 (OpenGL ES 3.0 Chromium)". With WebGPU available, the instance is
  WebGPU-only, as before.
- **No fallback after a device failure.** When the adapter exists but
  `requestDevice` rejects, startup fails, and the canvas already holds a
  `webgpu` context. A canvas keeps its first context type, so WebGL2 needs a
  fresh canvas.
- **Neither backend.** With no `navigator.gpu` and a null `webgl2` context,
  startup fails with wgpu's "failed to create wgpu surface … canvas.getContext()
  returned null".
- **Context acquisition.** One `getContext` call per start: `webgpu`, or
  `webgl2` with `{antialias: false}`.
- **Surface format and colour.** Both backends configure `Rgba8Unorm`, and
  egui applies its own gamma. Screenshots of Record Desk and the Gallery were
  pixel-identical across backends (ImageMagick `compare -metric AE` = 0). The
  Lab's differed by one pixel of Inspector text, and was identical at 1% fuzz.
- **Text rendering.** Identical, as above.
- **Startup report.** On WebGL2 the shared loader hangs in `starting`: it
  installed the `GPUQueue.submit` hook and waits for a submission that never
  comes. Record Desk's bootstrap has no such wait and started normally.
- **Image renderer.** The Lab's `R16Uint` scalar tiles uploaded and drew on GL:
  four GPU viewports, 20 draw calls, 655,360 resident texture bytes, the same
  as on WebGPU, with no wgpu validation errors in the console. Its capability
  text still says "WebGPU/WGPU", and Lab Diagnostics says "wasm32/WebGPU" as
  the platform.
- **Configuration.** WebGL2 needs nothing in `WebOptions` or `wgpu_options`.
  egui-wgpu already requests `downlevel_webgl2_defaults()` limits (with an
  8192 maximum 2D texture) on GL. Only forcing a backend and retrying after a
  WebGPU failure need options.

## Decisions

### Selection policy and context order

`WebHost::start` owns the policy, so every `AppHost` application gets it.
Lab, Gallery and Record Desk handles need no change.

- **Default (`auto`).** The first attempt uses `WebOptions::default()`, so the
  WebGPU path is byte-for-byte today's: eframe's detection uses WebGPU when an
  adapter exists, and GL when it does not. Suppose that attempt fails before
  the application is created, while `navigator.gpu` exists and
  `requestAdapter()` still returns an adapter (checked only on this failure
  path). Then WebGPU failed after adapter detection, for example in
  `requestDevice`. The host replaces the canvas with a shallow clone (same ID
  and attributes, no context) and retries once with `Backends::GL`. In every
  other case the first attempt already was WebGL2, or the failure was not
  the renderer's, so the host does not retry.
- **Override.** `?renderer=webgpu` or `?renderer=webgl2` in the page URL
  forces one backend, with `Backends::BROWSER_WEBGPU` or `Backends::GL` and no
  retry. A forced WebGPU without `navigator.gpu` fails without trying. Any
  other value fails startup and names the accepted values.
- **Failure.** When no attempt starts, the start promise rejects with one
  message naming each backend and why it failed, such as "No supported
  renderer: WebGPU is unavailable (navigator.gpu is missing); WebGL2 failed:
  …". The loaders show it under the existing `framework_start` phase.
- **Creator reuse.** eframe consumes the app creator only after its painter
  succeeds, so the host keeps the creator in a shared slot and passes each
  attempt a closure that takes it from the slot.
- **The JavaScript gates go.** The shared loader and Record Desk's bootstrap
  stop checking `navigator.gpu`. The loader's `startApplication` gains
  `requireWebGpu`, which only the viewer sets, so the viewer keeps today's
  WebGPU gate and failure case.

### Reporting

- **One identity.** The host records a `RendererIdentity`: `backend` (`webgpu`,
  `webgl2`, or a native wgpu backend name such as `vulkan` or `gl`),
  `adapter` (the wgpu adapter name), `selection` (`preferred`, `fallback` or
  `forced`) and an optional `reason`, such as "WebGPU returned no adapter".
  It lives in `stobie-protocol` and comes from the adapter that eframe
  created. Native hosts record it too.
- **Application interface.** `hello` gains an optional `renderer` field,
  omitted when unknown, so existing clients and wire tests are unaffected.
- **Page.** The web host sets `data-stobie-renderer`, `-adapter`,
  `-selection` and `-reason` attributes on the canvas before constructing the
  application. Any page, smoke or browser agent can read them without
  application hooks.
- **Startup milestones.** The shared loader copies those attributes into
  `__STOBIE_STARTUP.renderer`. On a backend other than WebGPU, it removes its
  `GPUQueue.submit` hook and reports `workspace_frame_submitted` as
  unavailable on WebGL2, taking the rendering opportunity straight after the
  CPU UI handoff, as it already does when the hook cannot be installed.
- **Lab Diagnostics.** Backend, adapter and selection come from the host's
  identity. Platform and the renderer capability name the real backend, and
  GPU timestamps stay unavailable.
- **Applications.** `renderer_identity(&egui::Context)` lets an application
  read the identity, as the Lab does.

### Test override and coverage

- **Forcing WebGL2 in smokes.** `STOBIE_BROWSER_RENDERER=webgl2` makes
  `tools/browser-launch.mjs` omit the five WebGPU-only Chromium flags (unsafe
  WebGPU, the SwiftShader adapter, the Dawn features and the WebGPU developer
  features), while keeping ANGLE and compositing. The Gallery browser smoke's
  own flag list follows the same switch.
- **The flags are not enough.** With SwiftShader, `navigator.gpu` remains but
  returns no adapter, as in Chromium's own Linux default. The Gallery smoke's
  headless shell on this host's hardware Vulkan still got a WebGPU adapter,
  and neither `--disable-features=WebGPU`, `--disable-webgpu` nor
  `--disable-blink-features=WebGPU` removed it. Smokes therefore also open
  pages through `rendererUrl()`, which adds `?renderer=webgl2`. The startup
  smoke still covers genuine fallback.
- **Proof of backend.** Each smoke checks the canvas's `data-stobie-renderer`
  against the expected backend, so a WebGPU run cannot pass as WebGL2.
- **New stage.** A `webgl2` verification stage, run in CI, repeats these on
  WebGL2:
  - Record Desk's browser smoke (pointer, keyboard, idle);
  - its ARIA layer check;
  - its browser CLI parity;
  - the Gallery browser, icon, navigation and status-chip smokes;
  - the Gallery's ARIA layer check.

  Increment 2 adds the Lab's smoke, its ARIA layer check and its browser
  application-interface check.
- **Startup smoke.** The Lab and Gallery "webgpu" failure cases become:
  - **fallback:** `navigator.gpu` removed, so WebGL2 is preferred;
  - **device fallback:** `requestDevice` rejected, so the canvas is replaced
    and WebGL2 retried;
  - **forced:** `?renderer=webgl2` on a WebGPU-capable browser;
  - **neither:** no `navigator.gpu` and no `webgl2` context, giving a clear
    failure.

  Each checks the reported backend and selection. The viewer keeps its
  "webgpu" case.
- **Unit tests.** Rust covers override parsing, backend naming, selection
  reasons and the optional `hello` field. `browser-startup.test.mjs` covers
  the WebGL2 submission path.

### Image renderer

The probe found the scalar renderer working on GL without changes. Increment
2 adds the proof:
- the Lab's browser smoke on WebGL2, checking draws, residency and a pixel
  readback of an image pane against the WebGPU run;
- its ARIA and application-interface checks;
- honest capability text.

If a check exposes a defect that is not a small, WebGPU-neutral fix, image
panes get a declared WebGPU requirement and a tested "this view needs WebGPU"
state instead. The renderer crate keeps its name: `stobie-render-wgpu`
remains one wgpu renderer on both backends, so the deferred rename stays
deferred.

## Increment 1 evidence

Measured locally on 2026-10-15 with Chromium 151.0.7922.34, headless with
SwiftShader. Raw results stay under ignored `.tools/runtime/`.

- **WASM size** (production package, branch against main `a346980`):

  | WASM | Raw | Brotli |
  | --- | --- | --- |
  | Gallery | +20,484 B (+0.18%) | +4,629 B (+0.16%) |
  | Lab | +21,282 B (+0.18%) | +3,237 B (+0.11%) |
  | Record Desk | +20,576 B (+0.20%) | not measured |

  The tile worker is unchanged. The viewer's raw size is unchanged, and its
  only difference is the build path embedded in it.
- **Startup.** `browser-startup-benchmark.mjs`: lab and gallery, five pairs,
  local profile, median milliseconds. WebGPU is unchanged within noise, and
  `browser-startup-compare.mjs` passes every regression gate against main.
  WebGL2 runs through ANGLE on SwiftShader.

  | Milestone (cold) | main WebGPU | branch WebGPU | branch WebGL2 |
  | --- | --- | --- | --- |
  | Lab `first_useful_content` | 175.0 | 173.5 | 206.2 |
  | Lab `rendering_opportunity_proxy` | 116.0 | 116.2 | 152.4 |
  | Gallery `first_useful_content` | 101.3 | 98.0 | 117.9 |
  | Gallery `rendering_opportunity_proxy` | 110.4 | 108.1 | 135.7 |
- **Visual agreement.** The startup smoke's Gallery comparison found 0 of
  1,296,000 pixels beyond tolerance, with a maximum channel difference of
  1/255.
- **Verification.** `cargo xtask verify` passes all nine stages, including
  the `webgl2` stage. Browser CLI parity on WebGL2 passed 14 of 15 local runs.
  The first run failed once with a parity mismatch whose detail was not
  captured; it did not recur in 14 further WebGL2 runs and 3 WebGPU runs.

## Delivery

| Increment | Scope | Status |
| --- | --- | --- |
| 1 | Host policy and override, reporting (`hello`, canvas, startup, Lab Diagnostics), JavaScript gates removed, Record Desk and Gallery WebGL2 stage, startup smoke cases, README and browser startup guide | In review |
| 2 | Lab image renderer on WebGL2 with tests, honest capability text, composition guides, Firefox check, General-agent probe, WASM size and startup evidence, plan closeout | Planned |
