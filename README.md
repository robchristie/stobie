# Polyorama

Polyorama is an experimental Rust framework layer for building GPU-driven
analytical workspaces on native desktop and WebGPU-capable browsers. It brings
together dockable panes, large tiled images, linked views, annotations and
virtualised collections above **egui**, **eframe** and **wgpu**.

The repository contains reusable framework crates and two runnable applications:
**Analytical Workspace Lab**, which exercises an analytical workflow, and
**Polyorama Gallery**, which demonstrates and verifies the UI components. Both
run natively and in the browser from Rust application code.

The project is under active development. Its APIs remain concrete and driven
by the example applications; it is not yet a stable general-purpose framework.

![Analytical Workspace Lab with four GPU image panes, results and an inspector](docs/design-agent-loop-evidence/increment-8-browser-dark.png)

*Browser capture from the [design-system verification evidence](docs/design-agent-loop-evidence/README.md).*

## What you can explore

### Analytical Workspace Lab

The Lab is a self-contained demonstration using deterministic synthetic data.
It requires no external datasets, private imagery or service credentials.

- **Multiple image views:** primary, linked detail, overview and derived views,
  with pan/zoom, camera linking and per-view display settings.
- **Progressive image loading:** scalar tiles generated and LZ4-decoded by
  background workers, with bounded scheduling, uploads and GPU residency.
- **Polygon annotation:** create and edit polygons in world coordinates, with
  gesture previews and undo/redo.
- **Large collections:** one million logical result rows and 100,000 logical
  thumbnails, materialising only the visible ranges and overscan.
- **A persistent workspace:** rearrange dock tabs, resize splits, save the layout
  and restore workspace state. Appearance preferences include light/dark themes,
  high contrast, density and font scaling.
- **Diagnostics:** inspect worker state, cache and upload budgets, render
  counters and CPU timings. Unavailable GPU timings are reported as unavailable.

Start by panning the Primary View and observing Linked Detail, then try the
Polygon and Edit tools. Open the Results, Thumbnails and Diagnostics tabs to
explore virtualisation and progressive loading.

### Polyorama Gallery

The gallery is the reference application for the shared design system. Its
fixed stories exercise buttons, tabs, splitters, status messages, virtual grids
and application chrome across normal, narrow, long-text, loading and error
states. It also exposes semantic snapshots and text-layout observations for
repeatable UI inspection and verification.

## Run locally

Run these commands from the repository root.

### Prerequisites

- Rust and Cargo. The workspace declares Rust **1.97.1** as its minimum,
  matching [CI](.github/workflows/verify.yml).
- For native applications, a graphical session and a working graphics backend
  supported by wgpu. Linux builds enable X11 and Wayland support.
- For browser builds, the `wasm32-unknown-unknown` target,
  **wasm-bindgen-cli 0.2.127**, and a browser with WebGPU available.
- Python 3 for the optional local HTTP server shown below.
- Node.js and npm for automated browser/UI verification. CI currently uses
  Node.js **25.8.2**; Playwright is pinned in `package-lock.json`.

### Native desktop

Launch the analytical workspace:

```sh
cargo run --release -p analytical-workspace-lab
```

Or launch the component gallery:

```sh
cargo run --release -p polyorama-gallery
```

### Browser

Install the browser build tools once. The CLI version must match the version
expected by `xtask`:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.127
```

Build both applications and the tile Web Worker, then serve the Lab:

```sh
cargo xtask build-web
python3 -m http.server 8080 --bind 127.0.0.1 --directory apps/analytical-workspace-lab/web
```

Open [Analytical Workspace Lab](http://localhost:8080). To run the gallery in
another terminal:

```sh
python3 -m http.server 8081 --bind 127.0.0.1 --directory apps/polyorama-gallery/web
```

Open [Polyorama Gallery](http://localhost:8081). Serve the files over HTTP rather
than opening `index.html` directly. Browser rendering requires WebGPU; there is
no WebGL fallback. Re-run `cargo xtask build-web` after changing Rust code.

## How the project fits together

Polyorama separates application state, asynchronous work, GPU resources and UI
presentation so each has a clear owner.

| Package | Responsibility |
| --- | --- |
| [`polyorama-core`](crates/polyorama-core) | Documents, session state, the serialisable dock tree, validated commands, typed coordinates and renderer-independent demands. No egui, wgpu or browser dependencies. |
| [`polyorama-runtime`](crates/polyorama-runtime) | Demand reconciliation, bounded worker scheduling and completion state. Independent of egui and wgpu. |
| [`polyorama-render-wgpu`](crates/polyorama-render-wgpu) | Persistent GPU resources, tile residency and typed render requests shared across viewports. |
| [`polyorama-ui-egui`](crates/polyorama-ui-egui) | The framework's egui integration: dock presentation, measured components, typed design tokens and semantic UI observations. |
| [`analytical-workspace-lab`](apps/analytical-workspace-lab) | The analytical demo, its feature panes and application-owned actions. |
| [`polyorama-gallery`](apps/polyorama-gallery) | The component catalogue and deterministic UI stories. |
| [`polyorama-tile-worker`](apps/tile-worker) | The browser Web Worker entry point for tile preparation and decoding. |
| [`xtask`](xtask) | Builds, architecture checks, token generation and verification tooling. |

The serialisable `polyorama_core::Workspace` is the sole authoritative dock
tree. Pane presenters receive narrow state views and emit intents; validated
commands apply changes. Durable annotations live in the document, while
selection, cameras, tools and gesture previews live in the session.

Data requests describe desired state. The runtime reconciles and deduplicates
them before scheduling work and rejects stale completions. The renderer owns
GPU resources across all viewports, and repainting is driven by recorded work
or interaction rather than an unconditional frame loop.

## Development and verification

The canonical verification command is:

```sh
cargo xtask verify
```

It checks generated-token drift, formatting, native and WASM Clippy, workspace
tests and architecture boundaries; builds release native and browser artefacts;
and runs application/gallery browser smokes and deterministic UI snapshots.
On Linux it also runs native interaction smokes. Generated evidence goes to
the ignored `.tools/runtime/verification-evidence/` directory.

Full verification requires the browser build tools above, Rust's `rustfmt` and
`clippy` components, Node.js/npm and the platform's browser/graphics libraries.
It runs `npm ci` and installs Playwright Chromium, so the first run needs network
access and can take longer than ordinary Rust tests.

On Linux, the default [bootstrap script](tools/bootstrap-linux-ui.sh) unpacks
pinned x86_64 UI packages into `.tools/`; that path also relies on tools such as
`curl`, `bsdtar`, `bwrap`, ImageMagick and `jq`. For a system-library setup, use
`POLYORAMA_USE_SYSTEM_UI_LIBS=1` and provision the dependencies and display as
shown in the [Ubuntu CI workflow](.github/workflows/verify.yml).

For focused development checks:

```sh
cargo test --workspace
cargo xtask architecture
cargo xtask tokens check
cargo xtask ui list --output-dir .tools/runtime/ui-list
```

UI rendering and snapshot checks require built browser packages. See the
[snapshot guide](docs/ui-snapshots/README.md) for exact capture, inspection and
verification commands. Baselines are reviewed source artefacts; verification
does not update them automatically.

## Documentation

- [Working rules](AGENTS.md): architectural boundaries and contribution expectations.
- [UI guides](docs/ui-guides/README.md): entry point for component, pane,
  interaction, accessibility and UI review work.
- [Design language](docs/design-language.md): visual and semantic contracts,
  backed by the [token source](design/tokens/polyorama.tokens.json).
- [UI evaluation seed](docs/ui-evaluation-seed.md): frozen tasks and explicit
  scoring criteria for repeatable UI evaluation.
- [Vertical-slice contract](docs/vertical-slice-goal.md) and
  [report](docs/vertical-slice-report.md): the Lab's original requirements,
  architecture, hardening results and retained runtime evidence.
- [Design-system report](docs/design-agent-loop-report.md): the component
  system, application migration and native/browser verification evidence.
- [Accessibility integration evidence](docs/accessibility-integration-report.md):
  adapter decisions, automated proof and the exact assistive-technology
  qualification matrix.

## Current limits

Polyorama compiles eframe's native AccessKit adapter and is
**AccessKit-semantic and keyboard-tested**. The representative workflow is
directly qualified with human-confirmed audible Orca output for the exact
Debian 13/GNOME 48/RDP/Orca 48.1 environment. Its tests cover roles, names,
states, bounds, actions, semantic parity and keyboard operation, but that one
versioned result is not evidence of screen-reader support on Windows, macOS,
other Linux configurations or browsers.
Stock eframe 0.36.1 discards browser AccessKit updates and provides no web
accessibility-tree adapter; the retained upstream reproduction and exact
qualification state are in the
[accessibility integration evidence](docs/accessibility-integration-report.md).
The exact qualification and remaining platform boundaries are recorded there.

The synthetic source and decoder demonstrate the architecture; they are not
production image codecs or remote data integrations. Production geospatial
reprojection, arbitrary texture import and a general-purpose render graph are
outside the current implementation. Optional pane creation and closing are
also not implemented.

Retained native runtime evidence uses Mesa llvmpipe under Xvfb, so it establishes
functional behaviour rather than physical-GPU performance. GPU timestamps are
unavailable in the documented captures. See the reports above for the tested
environments and the scope of each performance observation.

## Licence

Polyorama and its workspace packages are licensed under
[Apache License 2.0](LICENSE).
