# UI source-organisation plan

Status: active; component split under qualification

Baseline revision: `4ee27756698cbce19926ad2f02709107dda1036a`

Reorientation budget: 120 lines. Detailed implementation, review and CI
evidence belong with the owning pull request.

## Objective

Restore small local source contexts before the next component expansion by
splitting the existing reusable component recipes and gallery stories along
their established conceptual boundaries. Preserve behaviour, public Rust and
web paths, story identities, semantic IDs, rendered output and verification
evidence.

## Boundaries

- This is source organisation only: do not add components, traits, registries,
  abstractions, visual changes or interaction changes.
- Keep `polyorama_ui_egui` component exports at their existing crate-root paths.
- Keep shared component data and measured-text glue private to the component
  parent; do not create a public helper layer.
- Leave dock/splitter and application-bar recipes in the component parent. They
  do not belong to the eight requested leaf families.
- Keep gallery application state, configuration, chrome, navigation and the
  application-owned `GalleryAction` registry in `app.rs`.
- Treat gallery `data.rs` as an internal source grouping for existing property,
  status and virtual-grid stories. Do not change the catalogue taxonomy.
- Keep all story IDs, ordering, descriptions, snapshots and baseline artefacts
  unchanged.

## Acceptance

- `components.rs` becomes a private `components/` module with
  `action_button.rs`, `choice.rs`, `range.rs`, `status.rs`, `property.rs`,
  `result_row.rs`, `thumbnail.rs` and `viewport_status.rs`.
- Gallery story rendering is owned by private `stories/buttons.rs`, `dock.rs`,
  `toolbars.rs`, `data.rs` and `reference.rs` modules; `app.rs` remains the
  application shell.
- Existing public imports and web entry points compile without consumer edits
  whose sole purpose would be a renamed API.
- Focused crate tests, architecture checks and deterministic UI verification
  pass after each increment.
- Final `cargo xtask verify` passes without updating snapshot baselines.

## Delivery graph

| Increment | Outcome | Depends on | Status | Durable evidence |
| --- | --- | --- | --- | --- |
| 1 | Split reusable component recipes into eight leaf modules | — | Candidate | Focused checks and `cargo xtask verify` pass; pull request pending |
| 2 | Split gallery story rendering into five family modules | 1 | Pending | Pull request and canonical verification pending |

## Current phase

Review and land increment 1. The parent retains only shared private glue plus
the existing dock/splitter and application-bar recipes; each requested leaf
imports only its actual dependencies. Focused checks and the canonical gate
pass. Exact-head review and hosted CI remain before the gallery split starts.
