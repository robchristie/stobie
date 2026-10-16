# Stobie

Stobie is an agent-friendly, immediate-mode application framework in Rust for
native-style applications on the desktop and in the browser. An application
declares its state, its actions, its UI and how it applies changes. The
framework host owns everything else: lifecycle, shortcuts, availability,
repaint, semantic observation and agent operation. The same code runs natively
and in the browser, rendering with WebGPU and falling back to WebGL2. It is
built on **egui**, **eframe** and **wgpu**.

Agents can operate every Stobie application with no inspection code in the
application. The `stobie` command launches or attaches to it, natively or in
a browser, then reads its UI compactly and invokes its declared actions. In the
browser, a hidden ARIA layer mirrors the accessibility tree, so
accessibility-tree browser agents can read and operate it too. The layer is
built for screen readers as well, but no browser and screen-reader combination
is qualified yet; see [Current limits](#current-limits).

Stobie is **experimental and pre-1.0**. Its APIs change between commits. It is
not on crates.io and has no releases: applications depend on it through git,
pinned to an exact commit. See [Current limits](#current-limits).

Previously named Polyorama; see the [rename guide](docs/project-rename.md).

## Start a new app

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked --git https://github.com/robchristie/stobie --rev COMMIT stobie-cli
cargo install --locked wasm-bindgen-cli --version 0.2.127

stobie new my-app
cd my-app
cargo run --release           # native
stobie build-web              # browser: wasm build, wasm-bindgen and the startup loader
stobie serve                  # http://127.0.0.1:8080/
```

`COMMIT` is the full Stobie commit to build on, such as the head of `main`;
`stobie new` pins the new application to it. The application lives in its own
Cargo workspace: a small notes app, with its native and web entry points and
tests. [Start a new app](docs/start-a-new-app.md) explains each file, the
production build, upgrading the pin and building against a local Stobie
checkout.

An application declares each action once. The derive gives it a stable ID
(here `my-app.add`), and one availability rule serves buttons, shortcuts and
agents:

```text
#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "my-app")]
pub enum NoteAction {
    #[action(label = "Add note", description = "Add the draft as a note", shortcut = "Cmd+Enter")]
    Add,
}
```

An action can also take a value that its control or an agent supplies. In
outline, for a choice (`ActionArgument` and `ChoiceArgument` come from
`stobie_ui_egui`):

```text
// 1. Declare it: the variant carries no data. Arguments are "text" (max_length),
//    "boolean", "number" (minimum, maximum, integer) or "choice" (choices = Type).
#[action(label = "Sort notes", description = "Choose the order of the notes",
         argument = "choice", choices = SortOrder)]
Sort,

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortOrder { #[default] Oldest, Newest, Alphabetical }
impl ChoiceArgument for SortOrder {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Oldest, "oldest", "Oldest first"),
        (Self::Newest, "newest", "Newest first"),
        (Self::Alphabetical, "alphabetical", "A to Z"),
    ];
}

// 2. Handle the value in Application::perform_with; `perform` still matches
//    the variant but never receives it without a value.
fn perform_with(&mut self, target: ActionTarget<NoteAction>, argument: ActionArgument,
                state: &mut ()) -> Result<bool, String> {
    match target.action {
        NoteAction::Sort => { self.sort = argument.as_choice()?; Ok(true) }
        _ => self.perform(target, state),
    }
}

// 3. Present a control in the pane and trigger the action with its value.
let mut sort = self.sort;
let options: Vec<(SortOrder, &'static str)> =
    SortOrder::CHOICES.iter().map(|(value, _, label)| (*value, *label)).collect();
stobie_ui_egui::choice_control(ui, SemanticUiId::new("my-app.sort"), SemanticUiId::pane(pane),
    "Sort", &mut sort, &options, NoteAction::Sort, p.tokens());
if sort != self.sort { frame.trigger_with(NoteAction::Sort, ActionArgument::choice(sort)); }
```

An action with an argument has no shortcut, and its `perform` arm returns an
error. For the other kinds, `as_text()`, `as_boolean()` and `as_number()` each
return an `Option`, and controls trigger `ActionArgument::Text(..)`,
`ActionArgument::Boolean(..)` or `ActionArgument::Number(f64)`. A number is
declared `argument = "number", minimum = 1, maximum = 20, integer = true`
and presented with `range_control(ui, id, parent, label, &mut value_f32,
1.0..=20.0, action, p.tokens())`. A text value usually comes from a text field
that agents fill with `stobie set-text`. Then `stobie invoke my-app.sort --value alphabetical` takes the
option's ID or label.
[Add an action with an argument](docs/start-a-new-app.md#add-an-action-with-an-argument)
explains each step. The [composition guide](docs/application-composition.md) explains the four
parts and the host's contract, with the compiled
[`notes` example](crates/stobie-ui-egui/examples/notes.rs), which adds undo and
redo.

## Operate it with `stobie`

```sh
stobie launch app --manifest-path Cargo.toml
stobie observe                       # role "name" state #ID [capabilities], compactly
stobie set-text my-app.draft 'Buy stamps'
stobie invoke my-app.add             # waits for the effect and reports what changed
stobie stop app
```

In the browser, open the page with `?automation=1` in a Chromium that exposes
DevTools, then `stobie attach`. On a machine without GPU acceleration, start
Chromium with `--use-angle=swiftshader --enable-unsafe-swiftshader` for
software WebGL2. `stobie --help` and `stobie capabilities --json`
describe every command and the JSON output. [The `stobie` command](docs/agent-cli.md)
and the [application interface](docs/application-interface.md) give the
details.

## Examples

- **[`notes`](crates/stobie-ui-egui/examples/notes.rs)**: the complete hosted
  application in one file, with undo and redo:
  `cargo run -p stobie-ui-egui --example notes`.
- **[Record Desk](consumers/record-desk/README.md)**: a two-pane record
  workflow in its own Cargo workspace. It finds, edits and reviews synthetic
  records, applies one validated transaction, undoes and redoes, and saves
  across native restarts and browser reloads. It uses only the core and UI
  crates.
- **Analytical Workspace Lab** and **Stobie Gallery**, below: the optional
  image layer under load, and the shared design system's components.
- **[Emuella image viewer](https://github.com/robchristie/emuella-viewer)**, in
  its own repository: regional JPEG 2000 and NITF viewing on the image layer,
  built with the route above.

## The optional image layer

Most applications need only `stobie-core` and `stobie-ui-egui`. Image
applications add `stobie-imaging` (documents, sessions, cameras and image
commands), `stobie-runtime` (demand reconciliation and bounded workers),
`stobie-render-wgpu` (persistent GPU image resources) and the UI crate's
`image-rendering` feature; `stobie new --imaging` sets this up. The scalar
image renderer runs on WebGPU and WebGL2. See
[Compose an image application](docs/image-application-composition.md).

![Analytical Workspace Lab with four GPU image panes, results and an inspector](docs/images/analytical-workspace-lab-dark.webp)

*Browser capture from the [design-system verification evidence](docs/design-agent-loop-evidence/README.md).*

## The applications in this repository

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
- **A persistent workspace:** rearrange dock tabs, resize splits, close or reopen
  Diagnostics, save the layout and restore workspace state. Appearance
  preferences include light/dark themes, high contrast, density and font scaling.
- **Diagnostics:** inspect worker state, cache and upload budgets, render
  counters and CPU timings. Unavailable GPU timings are reported as unavailable.

Start by panning the Primary View and observing Linked Detail, then try the
Polygon and Edit tools. Open the Results, Thumbnails and Diagnostics tabs to
explore virtualisation and progressive loading.

Use **View/Panels → Diagnostics** to close or reopen Diagnostics. The menu item
shows whether it is open; **Ctrl/Cmd+Shift+D** toggles it from the keyboard.
Closing Diagnostics gives its dock space to the remaining panes. Reopening adds
the same pane as a tab beside Inspector and selects it. Use **Save layout** to
retain the open or closed state across restarts. Closing and reopening do not
change annotations, selection or cameras.

### Stobie Gallery

The gallery is the reference application for the shared design system. Its
fixed stories exercise buttons, tabs, splitters, status messages, virtual grids
and application chrome across normal, narrow, long-text, loading and error
states. It also exposes semantic snapshots and text-layout observations for
repeatable UI inspection and verification.

## Run this repository

Run these commands from the repository root to build the framework, the Lab and
the Gallery. An application of your own needs only the tools in
[Start a new app](#start-a-new-app).

### Prerequisites

- Rust and Cargo. [The repository toolchain](rust-toolchain.toml) and
  [CI](.github/workflows/verify.yml) select Rust **1.99.0** for development and
  verification. The workspace minimum remains Rust **1.97.1**.
- cargo-nextest **0.9.146** or later for the canonical Rust test runner. The
  [Nextest installation guide](https://nexte.st/docs/installation/pre-built-binaries/)
  provides pre-built binaries; CI installs the pinned, checksum-verified release.
- For native applications, a graphical session and a working graphics backend
  supported by wgpu. Linux builds enable X11 and Wayland support.
- For browser builds, the `wasm32-unknown-unknown` target,
  **wasm-bindgen-cli 0.2.127**, and a browser with WebGPU or WebGL2.
- Node.js and npm for automated browser/UI verification. CI currently uses
  Node.js **25.8.2**; Playwright is pinned in `package-lock.json`.

The root workspace and independent Record Desk workspace use Cargo resolver 3,
making Rust-version-aware dependency selection the default. When selecting new
dependency versions, Cargo prefers versions compatible with declared
`rust-version` values. This fallback is a preference; it does not guarantee
that every dependency supports the workspace minimum.

### Native desktop

Launch the analytical workspace:

```sh
cargo run --release -p analytical-workspace-lab
```

Or launch the component gallery:

```sh
cargo run --release -p stobie-gallery
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
cargo run --release -p stobie-cli -- serve apps/analytical-workspace-lab/web
```

Open [Analytical Workspace Lab](http://localhost:8080). To run the gallery in
another terminal:

```sh
cargo run --release -p stobie-cli -- serve apps/stobie-gallery/web --port 8081
```

Open [Stobie Gallery](http://localhost:8081). Serve the files over HTTP rather
than opening `index.html` directly. Applications on the framework host prefer
WebGPU and fall back to WebGL2 when WebGPU is missing, has no adapter or fails
to start; add `?renderer=webgl2` or `?renderer=webgpu` to the URL to force one.
The [browser startup guide](docs/browser-startup.md#renderer-backends) describes
the policy and how the running backend is reported. Re-run `cargo xtask build-web` after changing Rust code.

For measured production packaging, compression, immutable asset URLs and consumer
workspace setup, use the [browser build and startup guide](docs/browser-startup.md).
For a managed private HTTPS preview of the Lab from a development worktree, use
the [private development preview guide](docs/private-preview.md).

## How the project fits together

Stobie separates application state, asynchronous work, GPU resources and UI
presentation so each has a clear owner.

| Package | Responsibility |
| --- | --- |
| [`stobie-core`](crates/stobie-core) | What every application needs: the serialisable dock tree, stable pane IDs, generic coordinates, virtualisation, repaint reasons and a generic undo history. No egui, wgpu, browser or image dependencies. |
| [`stobie-protocol`](crates/stobie-protocol) | The application interface without egui: the version 1 inspection wire types, typed argument contracts, and the semantic, action and text observation types they carry. `stobie-ui-egui` re-exports it; clients depend on it alone. |
| [`stobie-imaging`](crates/stobie-imaging) | The image domain that image applications opt into: documents, sessions, cameras, image intents and commands, and renderer-independent tile and regional demands. |
| [`stobie-runtime`](crates/stobie-runtime) | Demand reconciliation, bounded worker scheduling and completion state for image tiles and regions. Independent of egui and wgpu. |
| [`stobie-render-wgpu`](crates/stobie-render-wgpu) | Persistent GPU resources, tile residency and typed render requests shared across viewports. |
| [`stobie-client`](crates/stobie-client) and [`stobie-cli`](crates/stobie-cli) | The client for the application interface and the `stobie` command built on it: create, build and serve applications, launch or attach to them, read and operate them. No application depends on them. |
| [`stobie-build`](crates/stobie-build) | The one web build and packaging implementation, shared by `stobie build-web` and this repository's `xtask`: wasm-bindgen with its version check, the embedded startup loader, compression and immutable assets. |
| [`stobie-ui-egui`](crates/stobie-ui-egui) | The framework's egui integration: the application host, dock presentation, measured components, typed design tokens and semantic UI observations. Its optional `image-rendering` feature adds GPU image presentation. |
| [`record-desk`](consumers/record-desk) | Independent native/browser record workflow with its own manifest, dependencies and lockfile; no image renderer/runtime or application-package dependency. |
| [`analytical-workspace-lab`](apps/analytical-workspace-lab) | The analytical demo, its feature panes and application-owned actions. |
| [`minimal-workspace` example](apps/analytical-workspace-lab/examples/minimal-workspace.rs) | A small native consumer using public framework APIs and existing package dependencies, independent of Lab implementation. |
| [`stobie-gallery`](apps/stobie-gallery) | The component catalogue and deterministic UI stories. |
| [`stobie-tile-worker`](apps/tile-worker) | The browser Web Worker entry point for tile preparation and decoding. |
| [`xtask`](xtask) | Builds, architecture checks, token generation and verification tooling. |

The serialisable `stobie_core::Workspace` is the sole authoritative dock
tree. Pane presenters receive narrow state views and emit intents; validated
commands apply changes, and core's generic history makes them undoable. In
image applications, durable annotations live in the image document, while
selection, cameras, tools and gesture previews live in the image session.

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
Explicit Record Desk checks cover its excluded Cargo workspace, dependency
independence, tests, native/WASM lint/build and physical native/browser workflows.
On Linux it also runs native interaction smokes. Generated evidence goes to
the ignored `.tools/runtime/verification-evidence/` directory; raw evidence is
never committed. The [evidence archive](docs/evidence-archive.md) explains what
stays tracked and how to retrieve archived captures.

Full verification requires the browser build tools above, Rust's `rustfmt` and
`clippy` components, Node.js/npm and the platform's browser/graphics libraries.
It runs `npm ci` and installs Playwright Chromium, so the first run needs network
access and can take longer than ordinary Rust tests.

On Linux, the default [bootstrap script](tools/bootstrap-linux-ui.sh) unpacks
pinned x86_64 UI packages into `.tools/`; that path also relies on tools such as
`curl`, `bsdtar`, `bwrap`, ImageMagick, `jq` and `rg`. For a system-library setup, use
`STOBIE_USE_SYSTEM_UI_LIBS=1` and provision the dependencies and display as
shown in the [Ubuntu CI setup action](.github/actions/setup-verification/action.yml).

Hosted verification runs ten stages in parallel and requires all of them under
the final `verify` check. The local command runs the same stages sequentially.
See [CI verification and cache ownership](docs/ci-cache.md) for the stage
inventory, focused reproduction commands and timing limits.

For focused development checks:

```sh
python3 tools/test-rust.py workspace
python3 tools/test-rust.py ui-no-default
python3 tools/test-rust.py record-desk
cargo xtask architecture
cargo xtask tokens check
cargo xtask ui list --output-dir .tools/runtime/ui-list
```

UI rendering and snapshot checks require built browser packages. See the
[snapshot guide](tests/ui-snapshots/README.md) for exact capture, inspection and
verification commands. Baselines are reviewed source artefacts; verification
does not update them automatically.

## Documentation

The [documentation index](docs/README.md) separates current guides from plans,
reports and evidence. The main guides:

- [Start a new app](docs/start-a-new-app.md): `stobie new`, the generated
  application, web builds, upgrading the Stobie pin and local checkouts.
- [Application composition](docs/application-composition.md): the hosted
  application path, derived actions, panes, browser entry and local rustdoc.
- [Image application composition](docs/image-application-composition.md): the
  four-crate GPU image lifecycle, a minimal triangle consumer and a presenter variation.
- [Application inspection and exercise](docs/application-interface.md): shared
  native/browser client, typed discovery and invocation, bounded queries and
  waits, physical targeting and capture provenance.
- [The `stobie` command](docs/agent-cli.md): launch or attach to an
  application natively or in a browser, read it compactly and operate it from
  a shell.
- [Browser builds and startup](docs/browser-startup.md): packaging,
  hosting, the WebGPU and WebGL2 backends and startup measurement.
- [UI guides](docs/ui-guides/README.md): entry point for component, pane,
  interaction, accessibility and UI review work.
- [Design language](docs/design-language.md): visual and semantic contracts,
  backed by the [token source](design/tokens/stobie.tokens.json).
- [UI evaluation seed](docs/ui-evaluation-seed.md): frozen tasks and explicit
  scoring criteria for repeatable UI evaluation.
- [Regional adapter contract](docs/regional-adapter-contract.md): immutable
  parent-image demands, external workers, resource accounting and integer display.
- [Vertical-slice contract](docs/vertical-slice-goal.md) and
  [report](docs/vertical-slice-report.md): the Lab's original requirements,
  architecture, hardening results and retained runtime evidence.
- [Design-system report](docs/design-agent-loop-report.md): the component
  system, application migration and native/browser verification evidence.
- [Accessibility integration evidence](docs/accessibility-integration-report.md):
  adapter decisions, automated proof and the exact assistive-technology
  qualification matrix.
- [Contributing](CONTRIBUTING.md): architectural boundaries, verification and
  contribution expectations for changes to Stobie itself.

## Current limits

- **Experimental, pre-1.0.** Public APIs are concrete and still driven by the
  example applications, and they change between commits without deprecation.
- **Git-pinned, not published.** No crate is on crates.io and there are no
  releases or version tags. Applications pin an exact commit and should install
  the `stobie` CLI from the same commit.
- **Browsers.** WebGPU is preferred and WebGL2 is the fallback; at least one
  must be available. CI automates Chromium on both. A scripted Firefox check of
  one Record Desk journey on WebGL2 runs outside CI.

Stobie compiles eframe's native AccessKit adapter and is
**AccessKit-semantic and keyboard-tested**. Its tests cover roles, names,
states, bounds, actions, semantic parity and keyboard operation. The
representative workflow was qualified with human-confirmed audible Orca output
for the exact Debian 13/GNOME 48/RDP/Orca 48.1 environment on
5 September 2026
([qualification matrix row](docs/accessibility-integration-report.md#platform-qualification-matrix)).
That qualification predates the ARIA-layer work (PRs #74 to #76), which also
changed the shared native AccessKit tree: it gained status roles with live
settings, named pane regions, headings, list boxes with position and set size,
keyboard shortcuts and percentage splitter values
([changes](docs/accessibility-integration-report.md#browser-aria-layer-with-orca)).
The current native tree has not been requalified with Orca. The one versioned result is also not
evidence of screen-reader support on Windows, macOS, other Linux
configurations or browsers.
In the browser, hosted applications mirror the same AccessKit tree into a
hidden [ARIA layer](docs/ui-guides/accessibility.md#browser-aria-layer).
Automated Chromium checks cover its tree, rule conformance, activation and
focus reporting. Actual Orca generated speech through it for a short journey in
one containerised Chromium environment without audio. No browser and
screen-reader combination is qualified. The exact qualification and remaining platform boundaries are in the
[accessibility integration evidence](docs/accessibility-integration-report.md).

The synthetic source and decoder demonstrate the architecture; they are not
production image codecs or remote data integrations. Production geospatial
reprojection, arbitrary texture import and a general-purpose render graph are
outside the current implementation. Diagnostics is the only closeable pane in
Analytical Workspace Lab. Reopening places it beside Inspector rather than
recreating its exact former split or tab position; arbitrary pane creation and
closing other panes remain unimplemented.

Retained native runtime evidence uses Mesa llvmpipe under Xvfb, so it establishes
functional behaviour rather than physical-GPU performance. GPU timestamps are
unavailable in the documented captures. See the reports above for the tested
environments and the scope of each performance observation.

## Licence

Stobie and its workspace packages are licensed under
[Apache License 2.0](LICENSE).
