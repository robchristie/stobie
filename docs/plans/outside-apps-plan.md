# Apps outside the repository, the viewer's own repository and the front door

Status: complete
Delivery: [PR #88](https://github.com/robchristie/stobie/pull/88) completes the plan; increments landed in [PR #86](https://github.com/robchristie/stobie/pull/86), [emuella-viewer#1](https://github.com/robchristie/emuella-viewer/pull/1), [stobie-workspace#5](https://github.com/robchristie/stobie-workspace/pull/5) and [PR #87](https://github.com/robchristie/stobie/pull/87).

## Outcome and boundaries

Anyone can build a Stobie application in its own repository, natively and for
the web, through the `stobie` CLI, and CI proves that from outside this
checkout on every pull request. The Emuella viewer lives in
`robchristie/emuella-viewer` as a downstream consumer of that route. The
README and front-door guides present Stobie as the general framework it is.

- No crates.io publication, releases or version tags. Applications depend on
  Stobie through git dependencies pinned to an exact revision.
- The Lab, the Gallery, Record Desk and `notes`, the inspection protocol, IDs,
  UI snapshot baselines and startup budgets are unchanged. The viewer behaves
  in its new repository as it did here and its existing checks pass there.
- History is not rewritten anywhere, and stobie-evidence is not modified.
  No imagery or prepared scene is copied; the viewer files are text, and their
  catalogues hold only identities (hashes, sizes, band counts) of external
  scenes, not pixels.
- The emuella-j2k git dependencies are Apache-2.0 (`emuella-j2k-codestream`
  is Apache-2.0 AND BSD-2-Clause), which permits an Apache-2.0 repository that
  depends on them.

## Design

### Dependencies and pinning

An outside application depends on each Stobie crate it uses with
`{ git = "https://github.com/robchristie/stobie", rev = "<40-hex commit>" }`:
`stobie-core` and `stobie-ui-egui` (feature `eframe`) for every app, plus
`stobie-imaging`, `stobie-runtime`, `stobie-render-wgpu` and the UI crate's
`image-rendering` feature for image apps. Every Stobie dependency names the
same revision. Upgrading means changing that revision everywhere and running
`cargo update`.

A local checkout overrides the pin with Cargo's
`[patch."https://github.com/robchristie/stobie"]` table, written into the
app's `.cargo/config.toml`. Cargo still fetches the pinned revision to resolve
the patch (checked 2026-10-16), so the pin must be a pushed commit; the patch
then compiles the local source. `stobie new --stobie-path PATH` writes that
file.

### `stobie new NAME`

Generates a standalone Cargo workspace outside any Stobie checkout:
`Cargo.toml` (package, empty `[workspace]`, `cdylib`+`rlib` library, release
profile, `wasm-bindgen = "=0.2.127"` for wasm), `rust-toolchain.toml` (the
Stobie toolchain plus the wasm target), `src/lib.rs` with a small notes-like
application on `AppHost` and its tests, `src/main.rs` (`run_native`), `web/`
(`index.html`, `bootstrap.js` importing the generated `browser-startup.js`),
`.gitignore` and a `README.md` with the build, serve and `stobie` commands.
It pins the revision the CLI was built from (a build script reads Git;
`--stobie-rev` overrides). `--imaging` adds the image crates and feature.

### One web packaging path

A new library crate, `stobie-build`, owns web packaging; the `stobie` CLI and
`xtask` both call it.

- The startup loader moves from `tools/browser-startup.js` to
  `crates/stobie-build/web/browser-startup.js` and is embedded with
  `include_str!`; builds write it beside `index.html`. Nothing copies it by hand.
- `build` runs `cargo build --lib --target wasm32-unknown-unknown`, reads the
  `wasm-bindgen` version the project locks from `cargo metadata`, refuses a
  different CLI version with the exact install command, and runs bindgen into
  `web/pkg`.
- `package` is a Rust port of `tools/browser-package.mjs`: the same manifest
  schema, content addressing, `<base>` injection, Brotli 11 and gzip 9 with
  round-trip checks, optional Binaryen 131, and atomic replacement. The Node
  script and its packaging tests are replaced by Rust tests; the Node
  production server and its tests stay.
- `serve` is a loopback development server with `no-store`.
- `stobie build-web [--production]` and `stobie serve` expose these.
  `cargo xtask build-web` and `build-browser-production` call the library;
  Record Desk's `build-web.sh` runs `stobie build-web --locked`.

Production output keeps its layout, raw assets, manifest schema and headers.
Content IDs already change with any root `Cargo.toml` edit or Node version;
they now hash the packager's version instead of Node's, and compressed bytes
come from the Rust encoders.

### CI from outside the checkout

A new `outside-app` verification stage builds the CLI, runs `stobie new` into
a temporary directory outside the checkout with `--stobie-path` pointing at
the checkout under test, runs its tests, builds it natively and with
`stobie build-web`, then:

- launches it natively with `stobie launch`, observes it and invokes one action;
- serves `web/` with `stobie serve`, opens it in Chromium with DevTools on
  WebGPU and with `?renderer=webgl2`, attaches with `stobie attach`, checks
  the reported renderer, observes and invokes one action.

### Viewer import list

Fresh import into `robchristie/emuella-viewer`, first commit naming the Stobie
source commit:

- `apps/emuella-viewer`, `apps/emuella-viewer-tools`, `crates/emuella-viewer-source`;
- the 29 viewer tools and 17 viewer tests under `tools/` and `tools/tests/`;
- the 65 viewer documents (`emuella-viewer-*`, `viewer-acceptance-*`,
  `viewer-native-*`, `real-scene-*`, `representation-efficiency-*`);
- adapted copies of `tools/browser-launch.mjs` and the UI bootstrap it needs,
  a `package.json` pinning Playwright, `rust-toolchain.toml`, the Apache-2.0
  licence, `CONTRIBUTING.md` and GitHub-hosted CI.

The viewer is not moved onto `AppHost`; that is the new repository's first
later-work item. The framework's `regional` modules in `stobie-imaging`,
`stobie-runtime` and `stobie-render-wgpu`, the regional adapter contract and
the loader's `requireWebGpu` option stay in Stobie as the public extension
points the viewer uses.

### Removal list

Workspace members and lock entries; the three viewer packages; the viewer
tools and tests; the viewer documents (with a pointer where they lived);
`xtask build-viewer-web`, `viewer-smoke`, the viewer in `build-web`, the
production build, wasm Clippy and the architecture lists; the viewer failure
cases in the startup smoke and benchmark; the forbidden-package entries in
`check-record-desk.py`; `.gitignore` entries; README mentions.
`docs/evidence-archive.md` keeps its archive table and permalinks and points
viewer readers to the new repository.

## Fresh-agent probe

On 2026-10-16 a fresh agent had only the README from this increment and a
`stobie` CLI installed with `cargo install --git … --rev 8bfcdd6`. It had no
Stobie checkout or documentation. Working outside any repository, it:

- created an application;
- added a `sort` action with a choice argument and a combo box;
- built it natively and for the web;
- drove it with `stobie`, natively and in Chromium.

It needed about 11 minutes and finished every step. Its gaps and their fixes:

- **Argument actions.** The README, the CLI and the template never showed
  how to declare an action with an argument; the agent found the derive's
  `argument` keys, `perform_with`, `trigger_with` and `choice_control` in
  rustdoc. Fixed: [Start a new app](../start-a-new-app.md#add-an-action-with-an-argument)
  shows the complete pattern, the README links it and the template points to
  it.
- **Generated code formatting.** It was not `rustfmt`-clean. Fixed: a test
  checks it for any name length, and the outside-app stage runs `cargo fmt
  --check`.
- **Attaching to a loading page.** Attaching before the application started
  failed. When startup failed, attach said only "no dispatcher". Fixed: attach
  waits, and both cases report the page's own startup state or failure. The
  outside-app stage checks both.
- **Help text.**
  - `attach --help` named an undefined `lantern` tool; it now describes the
    DevTools endpoint.
  - `capture --help` now names its native X11 requirements: `xdotool` and
    ImageMagick.
- **Software-only hosts.** Chromium needs SwiftShader flags for WebGL2, and
  natives may need `WGPU_BACKEND=gl`. Both are now documented.
- **What agents can read.** Content text and geometry are not exported, and
  generated presentation IDs can differ between hosts. Now documented: publish
  facts, and address controls by capability or semantic IDs.

A second fresh agent then tried the fixes. It used the README and a CLI built
from this branch (`bd60565`), and its own choice of argument: a number limit
with a slider. It finished every step in about 5 minutes, with no rustdoc and
no crate sources. It hit three compile errors, from details the README left
out for number arguments:
- `as_number()` returns an `Option`;
- a number is triggered with `ActionArgument::Number`;
- `range_control` takes an `f32` value and range.

Those details are now in the README, the generated README and the guide, along
with the software-rendering flags. The second probe also found that a slider's
value is not observed; that is recorded below.

## Later work

Found by the probe and not changed here, because each changes observations or
components the remaining applications and baselines rely on:

- **Text arguments.** A text argument has no ready-made control recipe;
  `choice_control` and `range_control` cover choices and numbers.
- **Combo-box observations.**
  - A `choice_control` is observed above controls it is drawn below.
  - Its label is only its accessible name.
  - Its open popup's options are not observed.
- **Presentation IDs.** Generated presentation IDs differ between native and
  browser hosts for the same control.
- **Sliders.**
  - A `range_control` slider publishes no value to agents, so its value
    must be published as a fact.
  - It shows no number on screen.

## Delivery

| Increment | Repository | Status | Delivery |
| --- | --- | --- | --- |
| 1. Outside-app route: `stobie new`, `stobie build-web`, `stobie serve`, one packaging path, the outside-app stage and the guide | stobie | Landed | [PR #86](https://github.com/robchristie/stobie/pull/86) (`8bfcdd6`) |
| 2a. Create and import the viewer repository, CI green, pinned to increment 1's merge | emuella-viewer | Landed | Import [`f83ff98`](https://github.com/robchristie/emuella-viewer/commit/f83ff987f39b3f61744209335a086afeb05d0f77) pinned to `8bfcdd6`; carried-over guards in [emuella-viewer#1](https://github.com/robchristie/emuella-viewer/pull/1) (`f2ef8b1`) |
| 2b. Register the viewer repository | stobie-workspace | Landed | [stobie-workspace#5](https://github.com/robchristie/stobie-workspace/pull/5) (`168baf2`) |
| 2c. Remove the viewer from Stobie | stobie | Landed | [PR #87](https://github.com/robchristie/stobie/pull/87) (`b9f5a6c`) |
| 3. README and front-door documents, the fresh-agent probe and closeout | stobie | Complete | [PR #88](https://github.com/robchristie/stobie/pull/88) |

## Evidence

- **Production packaging, increment 1.** Compared with `main` (`f79b477`):
  - the layout, manifest settings and raw assets are unchanged, apart from about
    380 bytes in the Lab and Gallery WASM for the startup mark;
  - Brotli representations are +0.18% and gzip +0.3% on WASM, from the Rust
    encoders;
  - the startup smoke and benchmark pass.
- **Viewer in its own repository.** `bash tools/verify.sh` passed there against
  the git pin, unpatched and `--locked`. All 485 third-party lockfile versions
  match Stobie's.
- **Counts.** At `8bfcdd6`, before the removal:
  - `docs/` held 132 top-level entries (139 files);
  - `tools/` held 90 top-level entries (133 files);
  - the 162 files that moved were 65 docs, 29 tools, 17 tool tests and 51 crate
    files.

  After the removal, `docs/` holds 68 entries (75 files, including the pointer
  [emuella-viewer.md](../emuella-viewer.md)) and `tools/` holds 61 (87 files).
