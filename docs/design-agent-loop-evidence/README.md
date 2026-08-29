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

## Increment 5: native/browser component gallery

The gallery implementation source revision is
`d2fabcbc701afe3f949da80e57ac06d1909cb840`. The retained evidence was produced
directly from that revision; the subsequent commit records only these evidence
files and their metadata.

- `gallery-manifest.json` contains 18 unique typed story records;
- `gallery-browser-overview.png` shows the dark/standard/comfortable canonical
  dock reference at 1440×900 CSS pixels;
- `gallery-browser-high-contrast-narrow.png` shows the narrow diagnostics
  reference at light/high contrast, compact density and 150% font scale;
- `gallery-browser-splitter-states.png` shows the four deterministic hover,
  pressed, keyboard-focus and active-drag treatments painted by the same
  production splitter recipe;
- `gallery-native-overview.png` shows the same application-shell story under
  native GL/llvmpipe at 1440×900;
- `gallery-browser-snapshot.json` and `gallery-native-snapshot.json` retain the
  Rust-owned story configuration, bounded geometry, text observations and
  empty audit result;
- `gallery-browser-evidence.json` records Chrome 151.0.7922.34, Playwright
  1.62.1, browser WebGPU via eframe/wgpu and unchanged warmed idle frame 16;
  and
- the native runtime/Xvfb logs retain the software-rendering environment and
  contain no panic or wgpu failure.

Exact capture commands were:

```text
cargo build --release -p polyorama-gallery
cargo build --release --target wasm32-unknown-unknown -p polyorama-gallery
wasm-bindgen --target web --out-dir apps/polyorama-gallery/web/pkg target/wasm32-unknown-unknown/release/polyorama_gallery.wasm
POLYORAMA_EVIDENCE_DIR=docs/design-agent-loop-evidence npm run gallery-browser-smoke
POLYORAMA_EVIDENCE_DIR=docs/design-agent-loop-evidence bash tools/gallery-native-smoke.sh
```

SHA-256 values:

```text
b0585d8ccb277b75d5afe4f30ff4c14997d0aecdcf65f0a32f352b33a72d72e0  gallery-browser-overview.png
2a6ef827dc9b5ef026bfc7bdaf4a2bb97a8d442b716f378fd398fc7746c9208e  gallery-browser-high-contrast-narrow.png
cef0fe8af19ec5651b5a97924537caf9792f12e0034439037668b22583cf8130  gallery-browser-splitter-states.png
6009a91e6a255d955b53413294c74ac786776e8d0181856d4c395a6554a958ae  gallery-native-overview.png
b974a5602eeaad62f00cf167128680d76ede60f4e06447d5c60b8d58131288b5  gallery-manifest.json
```

The screenshots support visual review. Catalogue invariants, all-story text
layout, component semantics, launch behaviour and idle repaint are established
by executable tests and the native/browser smoke assertions.

## Increment 6: action registry and semantic snapshot

The implementation source revision is
`87dcccd4c1d8e7b94625a0fd2b34f01d164fa954`. The retained JSON was produced
from its native release and Wasm outputs after the runtime harness repair;
these evidence files and metadata are recorded in the following commit.

- `increment-6-browser-semantic.json` records 66 bounded application nodes,
  stable action identities, exact linked camera/render-plan agreement,
  annotation commit/undo, canonical dock resize/undo and an empty semantic
  audit under browser WebGPU;
- `increment-6-native-semantic.json` records the same 66-node initial surface,
  registry-driven physical action targeting, linked pan and exact undo,
  progressive thumbnail scrolling, vertex editing and an empty semantic audit
  under native GL/llvmpipe; and
- `increment-6-gallery-semantic.json` records the focused `Fit view` action by
  stable ID, pane, complete name, current geometry and empty semantic audit in
  the deterministic keyboard-focus story.

The browser physical harness also asserts an empty semantic audit after every
profiled interaction, including pane drag/drop where a narrow edge placement
previously exposed clipped controls outside the root surface. The native and
browser harnesses locate Undo, Redo, Save layout, Fit view, Link views and tool
selection by `ActionId` in the current Rust snapshot, not by label or fixed
coordinates.

Candidate head `d05ac950bdc6f830d96aaee22373936e26d27a65` passed
`cargo xtask verify`: 140 Rust tests, token drift and architecture checks,
native and Wasm clippy/release builds, browser WebGPU application/gallery
smokes and native GL/llvmpipe application/gallery physical smokes. Both gallery
and warmed application idle-frame assertions passed.

SHA-256 values:

```text
621047afc8f473e8404ef37a8c0bbe1a314f13504182ff3dcdd54e517c78a7db  increment-6-browser-semantic.json
87a4ccef405877eb32572c7f125479aa9512a6aa576f90f5614360513421eca8  increment-6-native-semantic.json
1e0618a718d1defadbbe9ab4e8e09877549545443064aeafe349e942c386841b  increment-6-gallery-semantic.json
```

## Increment 7: deterministic UI verification loop

Implementation source revision
`6dbd18cb833c3949ca7e3a6ddabfde7cfb836842` passed the complete canonical
gate with 147 Rust tests, five exact browser-WebGPU snapshot fixtures and all
existing native/browser physical smokes. The selected expected metadata,
semantic snapshots, text observations and PNGs are owned by
[`../ui-snapshots/`](../ui-snapshots/); they pin viewport, data seed,
preferences, bundled fonts and renderer contract.

[`increment-7-ui-verification.json`](increment-7-ui-verification.json) records
the exact environment, command, fixture hashes and four negative probes. A
one-field mismatch emitted complete expected/actual/diff evidence; browser
launch failure retained per-fixture unavailable evidence and logs plus the root
summary; an incomplete baseline retained the actual capture and explicit
missing evidence; and a baseline-tree output target was rejected before
cleanup with its hash unchanged. Every probe exited 1 and restored source
state without a diff.
The GitHub workflow invokes only `cargo xtask verify` and uploads its ignored
evidence directory on failure; it contains no baseline update operation.

## Increment 8: complete application migration

Implementation source revision
`ec6483ca419ac61753464bac271e6e67e526e811` passed `cargo xtask verify` with
157 Rust tests, token drift and architecture checks, strict native/Wasm lint
and release builds, both browser smokes, five zero-tolerance UI fixtures, and
both native GL/llvmpipe physical smokes. The verifier retained the selected
application matrix from the same release Wasm and native binaries.

- `increment-8-browser-dark.png` and `increment-8-browser-light.png` establish
  distinct standard-theme application states;
- `increment-8-browser-high-contrast-150.png` combines authored light high
  contrast, compact density and 150% text while retaining bright scientific
  overlays and non-overlapping application chrome;
- `increment-8-browser-narrow.png` exercises collapsed application status,
  wrapped pane toolbars, truncated tabs and measured viewport status text;
- `increment-8-display-controls.png` shows the narrow-pane `Display` action
  and its complete semantic map/window popup within the application root;
- `increment-8-gallery-long-text.png`, `increment-8-gallery-error.png` and
  `increment-8-gallery-loading.png` retain the required long-text, persistent
  error and progressive-loading treatments from production components; and
- `increment-8-browser-semantic.json` and `increment-8-native-semantic.json`
  retain bounded current geometry, application actions, exact interaction
  postconditions, virtualisation, empty semantic/text audits and idle evidence.

The browser physical smoke targets the real `Appearance` and `Display` actions,
asserts every preference and image control by stable role, ID and registry
action, physically changes all five appearance fields, then saves, reloads and
checks their exact persisted values. It also retains the complete semantic value
of ellipsised viewport status text and checks an empty semantic audit after every
profiled interaction. A separate disposable Lantern session used Chrome 151.0.7922.47
with hardware WebGPU on `nvidia.com/gpu=0`; it showed a nonblank four-view
canvas, zero layout findings, and zero navigation-time console exceptions,
failed requests or HTTP errors. Canonical browser evidence uses the pinned
Playwright 1.62.1 WebGPU path; native evidence uses GL/llvmpipe at 1440×900.

SHA-256 values:

```text
3e5dd2e310e10c717d0468798eb3de7bf02334cc455ce2734dc6d4daf9cf49e2  increment-8-browser-dark.png
d1ad3a40000cc447d239425d756035909c466a99c7cc89acc1e5a21c1b0cb907  increment-8-browser-light.png
4cb4a24e22cfe226a9aba4fce23d5840107ae6980aef5974ffcdaa350f2c8a30  increment-8-browser-high-contrast-150.png
6baa6af2404e7e867c166e61344dc59c81229dba4c811023132bc8b7add74fcc  increment-8-browser-narrow.png
ccce74ad999aef561dfe2b3b84bd6807b26f61dde991bed07b01b3f1ea7e1706  increment-8-display-controls.png
1fdd232cd63abb65a0989a1b3d43a7d5bf5d12ed4737b33ce705cd3736b2f503  increment-8-gallery-long-text.png
7bd6510d5e828309c8469d11da7bb5cca7aaa62c7a66a83edc5efc7535456e07  increment-8-gallery-error.png
da046b6d200d62148f22a79ee64754719fab97aec6dd1b064738628ae6dd91ec  increment-8-gallery-loading.png
4fbf7d473014e084af255c522978ab7bddcd0ebc81d792335acb981ca8bba629  increment-8-browser-semantic.json
e2d146c7a603541026b54f83829f85ffe9da6127a2cf0e51ae432fb97999a08c  increment-8-native-semantic.json
```

## Increment 9: final release qualification

Release-observation source revision
`7a5e63f47fe078b68816b50098c5dd338dae7d0e` passed the complete local
`cargo xtask verify` gate: 157 tests, token drift and architecture checks,
strict native/Wasm lint and release builds, both browser smokes, five exact UI
fixtures, and both native GL/llvmpipe physical smokes.

- `increment-9-browser-performance.json` retains bounded release-Wasm
  application-update CPU observations for initial loading, four-view panning,
  rapid zoom, million-row results, thumbnails, polygon editing, dock
  interactions, theme switching, font scaling and workspace restoration;
- the same file records warmed event-driven idle at frame `440 → 440`; and
- `increment-9-gallery-browser-evidence.json` retains browser/backend identity,
  18-story release readiness, seven selected story transitions and gallery
  idle at frame `26 → 26`.

GPU timestamp timing was unavailable and remains explicitly `null`; these CPU
and wall observations are not labelled as whole-frame or GPU timing. The
criterion-by-criterion reconciliation is in
[`../design-agent-loop-report.md`](../design-agent-loop-report.md).

SHA-256 values:

```text
04d7d8bb7cc4f74d8944e6d2706c45965bb1eadf1fb98c471a515b1cc10a085e  increment-9-browser-performance.json
96d3ca46ad40e4e7913dc4cbd0c48e8dcea5025c2d0b79d964bbbc919a17af67  increment-9-gallery-browser-evidence.json
```
