# Design-agent loop baseline audit

Revision: `b8c66317aaa9284c45e712278010bc9cd285c01b`

Observed: 29 August 2026

## Verification result

`cargo xtask verify` passed with formatting, native and Wasm clippy, 83 Rust
tests, architecture checks, release native and Wasm builds, browser WebGPU
smoke and native llvmpipe smoke. The browser reported four scalar render jobs,
five completed worker requests and no warmed-idle repaint reason. Native smoke
completed at 1440×900 with current Rust-owned geometry targeting.

## Visual observation

The existing dark application is coherent enough to operate, and analytical
imagery remains dominant. Chrome is visually flat and mostly restrained. The
baseline nevertheless has weak hierarchy between application, pane and data
surfaces; inconsistent direct colours; underspecified focus; text-only actions;
crowded narrow toolbars; clipped controls at narrow widths; character-count tab
sizing; raw numeric alignment; and no light or high-contrast variants.

The baseline screenshots used for comparison are:

- `docs/vertical-slice-evidence/browser-default.png`;
- `docs/vertical-slice-evidence/browser-narrow.png`;
- `docs/vertical-slice-evidence/native-default.png`;
- `docs/vertical-slice-evidence/browser-diagnostics.json`;
- `docs/vertical-slice-evidence/browser-semantic.json`; and
- `docs/vertical-slice-evidence/browser-performance.json`.

## Source inventory

- Global direct styling: `apps/analytical-workspace-lab/src/app.rs`.
- Dock tab sizing, painting and splitters:
  `crates/polyorama-ui-egui/src/lib.rs`.
- Pane toolbar/status styling:
  `apps/analytical-workspace-lab/src/panes/image.rs` and `panes/mod.rs`.
- Result and thumbnail geometry:
  `apps/analytical-workspace-lab/src/panes/results.rs` and
  `apps/analytical-workspace-lab/src/panes/thumbnails.rs`.
- Annotation colours and handles:
  `apps/analytical-workspace-lab/src/panes/annotations.rs`.
- Current bounded physical geometry:
  `apps/analytical-workspace-lab/src/ui_geometry.rs`.
- Current cross-platform application observation:
  `apps/analytical-workspace-lab/src/app.rs::TestSnapshot`.
- Current verification orchestration: `xtask/src/main.rs`.

## Strongest first failures

1. Replace character-count tab sizing with measured layout and declared
   truncation before changing typography.
2. Introduce typed semantic tokens before migrating raw component colours.
3. Separate generated run artefacts from committed reference evidence before
   adding deterministic UI snapshots or CI upload.
4. Enable and test explicit semantics for custom-painted controls before
   claiming accessibility coverage.

These findings select the dependency order in the active campaign plan; they
are not completion claims.
