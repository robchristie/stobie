# Design-agent loop evidence

This directory retains selected, reviewable campaign evidence. Generated test
runs remain outside the tracked tree unless a result is deliberately selected
and described here.

## Increment 2: token-driven application bar

`increment-2-token-application-bar.png`

- application source revision:
  `436ec422dc6220c24ed3515fcb012364125ea465`;
- captured: 29 August 2026;
- viewport: 1279×756 CSS pixels;
- application: release Wasm `analytical-workspace-lab`;
- browser: Chrome 151.0.7922.47 in a disposable Lantern session;
- backend: hardware WebGPU, `nvidia.com/gpu=0`, unsafe WebGPU explicitly enabled;
- result: nonblank scalar canvas, no Lantern layout finding, and the isolated
  application-bar recipe rendered without overlap or clipping;
- fresh follow-up navigation after the favicon correction: zero console
  messages, zero exceptions, zero failed requests and zero HTTP errors; and
- SHA-256:
  `5a83444f609d5cff0faebb019f8ed0d5885c36a644837870c8b414286acc78a8`.

The committed release Wasm output was produced by `cargo xtask verify` at the
source revision above. The exact capture sequence was:

```text
python3 -m http.server 4173 --bind 0.0.0.0 --directory apps/analytical-workspace-lab/web
/nvme/development/lantern/target/release/lantern browser start --graphics webgpu --gpu-device nvidia.com/gpu=0 --json
/nvme/development/lantern/target/release/lantern flow --endpoint http://127.0.0.1:34247 --open http://host.docker.internal:4173/ --timeout-ms 30000 --quiet-ms 1000 --json
/nvme/development/lantern/target/release/lantern layout --endpoint http://127.0.0.1:34247 --json
/nvme/development/lantern/target/release/lantern screenshot --endpoint http://127.0.0.1:34247 --output docs/design-agent-loop-evidence/increment-2-token-application-bar.png --overwrite --json
```

The screenshot is supporting visual evidence. Token correctness and variant
coverage are established by generated-code, contrast, preference and compiler
tests rather than by this single dark/comfortable capture.

## Increment 3: measured dock-tab text

`increment-3-measured-tabs.png`

- application source revision:
  `7e21dc4cb42bed9c4249eab19a9cd580027df13e`;
- captured: 29 August 2026;
- viewport: 1279×756 CSS pixels;
- application: release Wasm `analytical-workspace-lab`;
- browser: Chrome 151.0.7922.47 in a disposable Lantern session;
- backend: hardware WebGPU, `nvidia.com/gpu=0`, unsafe WebGPU explicitly enabled;
- result: all eight dock-tab labels use egui galley measurement, remain centred
  and legible, and exhibit no overlap or accidental clipping;
- companion native and browser semantic smokes each exported eight bounded
  `TextLayoutObservation` values and zero text-audit findings;
- Lantern result: zero console messages, exceptions, failed requests, HTTP
  errors or layout findings; and
- SHA-256:
  `dfe76a31cd4e4250e470860fe172182e12af3ac2dbbed2deaf49554a7929f025`.

The exact source revision passed `cargo xtask verify`: 106 Rust tests, token
drift and architecture checks, native and Wasm lint/release builds, browser
WebGPU smoke and native GL/llvmpipe physical smoke. The screenshot is visual
support; truncation, alignment and bounds are established by the measured-text
fixtures, responsive dock tests and exported semantic observations.
