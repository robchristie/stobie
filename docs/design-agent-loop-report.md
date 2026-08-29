# Polyorama design system and agent UI loop report

Status: complete candidate; awaiting exact-head review and hosted verification

## Outcome

The design-system and agent-control-plane goal is implemented without changing
the egui/wgpu substrate or adding analytical capability. Polyorama now has one
typed token source, one measured component vocabulary, persisted orthogonal
preferences, AccessKit-aligned semantics, a shared action registry, a native
and browser gallery, deterministic UI verification, and a completely migrated
Analytical Workspace Lab.

The product remains a layer above egui and wgpu. The existing document,
session, command, dock, runtime, render-plan and repaint boundaries remain in
force. No runtime stylesheet, UI DSL, DOM renderer, hosted design dependency or
second workspace model was introduced.

## Qualification identity

- baseline revision:
  `b8c66317aaa9284c45e712278010bc9cd285c01b`;
- final application migration merge:
  `89d9ec40ed2abc07f92955bf859493c3af63790e` (PR #16);
- final release-observation source revision:
  `7a5e63f47fe078b68816b50098c5dd338dae7d0e`;
- canonical command: `cargo xtask verify`;
- local qualification date: 30 August 2026, Australia/Adelaide;
- Rust: `rustc 1.97.1 (8bab26f4f 2026-07-14)` and
  `cargo 1.97.1 (c980f4866 2026-06-30)`;
- Node/npm: `v25.8.2` / `11.11.1`;
- `wasm-bindgen-cli 0.2.127`, Playwright `1.62.1`, Chromium
  `151.0.7922.34`;
- framework: egui/eframe/egui-wgpu `0.36.1`, wgpu `30.0.1`;
- browser backend: WebGPU through eframe/wgpu;
- native backend: GL through Mesa llvmpipe at 1440×900; and
- licence: repository and every workspace package use Apache-2.0.

The exact observation revision passed the canonical local command. The command
performed format and strict native/Wasm lint checks, 157 Rust tests, token
drift and architecture checks, optimised native and Wasm builds, application
and gallery browser smokes, five deterministic UI fixtures, and application
and gallery native physical smokes.

## Evidence classification

Each mandatory criterion below is classified as one of:

- **Directly verified**: an executable check or retained semantic/physical
  observation establishes the claim;
- **Approximate**: representative evidence supports the claim but does not
  exactly establish it;
- **Blocked**: a retained reproduction identifies an external blocker; or
- **Unavailable**: the platform does not expose the measurement.

All mandatory criteria are directly verified. GPU timestamp timing is
unavailable on the qualified browser adapter and is reported as unavailable in
diagnostics; no mandatory criterion depends on it. Gallery wall times are
release observations, not portable performance budgets.

## Architecture and non-goals

`cargo xtask architecture` and the architecture phase of the canonical gate
check that core and runtime stay independent of egui/wgpu, panes use the narrow
UI boundary, production text remains measured, viewport code cannot create a
device or queue, and the serialisable workspace remains the only dock tree.
Token generation produces typed Rust at build/development time; production
frames neither parse token JSON nor construct a runtime styling language.

The campaign did not add Figma, DOM/CSS rendering, A2UI, MCP control, an LLM UI
runtime, a GUI abstraction, hosted design services, automatic baseline
approval, a replacement text renderer, or new Geometis-specific capability.

## Retained proof

The evidence index is
[design-agent-loop-evidence/README.md](design-agent-loop-evidence/README.md).
It records exact revisions, capture commands, environments and SHA-256 values
for selected artefacts. Principal proof includes:

- generated token source and drift tests in
  [polyorama.tokens.json](../design/tokens/polyorama.tokens.json) and
  [generated_tokens.rs](../crates/polyorama-ui-egui/src/generated_tokens.rs);
- the documented grammar in
  [design-language.md](design-language.md);
- five pinned expected fixtures and their semantic/text metadata in
  [ui-snapshots](ui-snapshots);
- the increment 8 dark, light, high-contrast/150%, narrow, display-control,
  long-text, loading and error captures;
- final browser application observations in
  [increment-9-browser-performance.json](design-agent-loop-evidence/increment-9-browser-performance.json);
- final browser gallery observations in
  [increment-9-gallery-browser-evidence.json](design-agent-loop-evidence/increment-9-gallery-browser-evidence.json);
- final application semantic evidence for native and browser in the increment
  8 files; and
- focused component, pane, interaction, token, accessibility and review guides
  under `docs/ui-guides/`, plus six frozen scored tasks in
  `docs/ui-evaluation-seed.json`.

## Release observations

Application values below are application-update CPU time from release Wasm,
not end-to-end frame or GPU time. Percentiles select from the recorded bounded
sample set. The baseline and final runs use the same 1440×900 logical scenario
and canonical smoke structure, but they are individual local observations, not
a statistical performance study.

| Scenario | Baseline median / p95 | Final median / p95 | Result |
| --- | ---: | ---: | --- |
| Initial loading | 0.7 / 21.0 ms | 0.8 / 19.6 ms | Stable warmed cost; lower observed initial tail |
| Four linked GPU views panning | 0.7 / 0.9 ms | 0.6 / 1.2 ms | Stable; four render jobs retained |
| Rapid zoom transitions | 0.7 / 0.8 ms | 0.6 / 0.7 ms | Stable |
| Million-row result scrolling | 0.7 / 1.0 ms | 0.7 / 0.9 ms | Stable and bounded |
| 100,000-item thumbnail scrolling | 0.6 / 0.9 ms | 0.5 / 0.8 ms | Stable and bounded |
| Theme switching | Not present at baseline | 0.7 / 6.2 ms | New capability, physically exercised |
| Font scaling | Not present at baseline | 0.8 / 6.3 ms | New capability, physically exercised |

Final interaction observations also record polygon editing at 0.6/0.8 ms,
splitter interaction at 0.6/0.9 ms, pane dragging at 0.5/0.7 ms and saved layout
restoration at 0.7/4.7 ms. The warmed application frame counter remained
`440 → 440`; the gallery counter remained `26 → 26`. Neither runtime requested
continuous repaint.

The baseline did not contain the gallery. The final release gallery became
ready in 691 ms in this run; seven selected production-story transitions took
12–21 ms wall time and one or two event-driven frames each. These values include
browser automation and are retained as observations rather than budgets.

The profile did not identify a reason for architectural optimisation. No
performance-motivated product change was made in this increment.

## Mandatory acceptance matrix

### Tokens and preferences

| Criterion | Classification | Evidence |
| --- | --- | --- |
| One documented bounded DTCG-style JSON subset authors all tokens | Directly verified | Token compiler tests, design language and the single token JSON source |
| Types, aliases, missing references, cycles and finite values are checked | Directly verified | Positive and negative `xtask::tokens` tests in the canonical gate |
| Light, dark and high-contrast form one coherent system | Directly verified | Contrast tests, typed variant generation and selected application/gallery captures |
| Compact and comfortable share one component vocabulary | Directly verified | Orthogonal token resolution tests and gallery configuration matrix |
| Generated typed Rust is deterministic, checked in and drift-checked | Directly verified | `cargo xtask tokens check` and deterministic generation test |
| Runtime consumes typed values, not token strings | Directly verified | Generated typed API and architecture source check |
| Five preferences are orthogonal, versioned, validated and persisted outside documents | Directly verified | Preference migration/unit tests and physical save/reload equality in browser smoke |
| Font scale is bounded and obsolete values fall back predictably | Directly verified | Preference migration and bounds tests |
| Production rejects unmanaged style and character-width shortcuts | Directly verified | Canonical architecture scan covers colour, spacing, radius, font size, glyph and width patterns |

### Text and components

| Criterion | Classification | Evidence |
| --- | --- | --- |
| Production component text uses egui font measurement | Directly verified | Measured-text implementation, production scan and component tests |
| Character-count sizing is absent from production UI | Directly verified | Architecture gate and measured dock-tab regression tests |
| Reusable components declare overflow and semantic full text | Directly verified | Component recipes, text observations and ellipsised viewport semantic test |
| Long/narrow text works at 100%, 125% and 150% | Directly verified | All-story matrix, pane tests, narrow/150% captures and empty audits |
| Numeric result columns remain deterministically right aligned | Directly verified | Result/technical-row measured tests and audit |
| Observations expose allocation, paint, clip, lines and truncation | Directly verified | `TextLayoutObservation` schema, snapshots and inspection commands |
| Audits reject undeclared overflow, overlap, invalid geometry and alignment deviation | Directly verified | Negative text-audit tests and `cargo xtask ui audit-text` |
| No mandatory story has accidental clipping or overlap | Directly verified | All 18 stories render with empty text audit; selected snapshots pass exact comparison |
| Production and gallery share typed component implementations | Directly verified | Gallery imports production `polyorama-ui-egui` recipes; architecture and all-story tests |

### Gallery, semantics and actions

| Criterion | Classification | Evidence |
| --- | --- | --- |
| Gallery runs natively and in a browser | Directly verified | Canonical browser WebGPU and native GL/llvmpipe gallery smokes |
| Stable typed stories cover required matrices and composed scenes | Directly verified | 18-entry typed manifest, finite matrix tests and selected captures |
| Every custom control exposes role, name, state, actions, focus and usable target | Directly verified | AccessKit/component tests, semantic audit and current geometry bounds |
| Buttons, tabs and splitters are keyboard-operable | Directly verified | egui kittest, focus-navigation and splitter-adjustment tests |
| One `ActionId` drives controls, shortcuts, accessibility, tests and targeting | Directly verified | Registry completeness/parity tests and both physical smokes |
| Availability and disabled reasons are observable | Directly verified | Context availability tests and semantic snapshots |
| Reusable `UiSnapshot` exposes bounded IDs, geometry, actions, text and references | Directly verified | Snapshot schema tests and retained native/browser JSON |
| AccessKit and augmented semantics cannot disagree silently | Directly verified | Parity tests and empty semantic audits |
| Physical automation targets current semantic geometry | Directly verified | Rust-owned target lookup in browser/native harnesses, including preference controls |
| Compatible egui/AccessKit semantic test path exists without stack downgrade | Directly verified | Released `egui_kittest` query/activation test on egui 0.36.1 |

### Tooling, CI and agent guidance

| Criterion | Classification | Evidence |
| --- | --- | --- |
| Five `cargo xtask ui` operations provide stable outputs and explicit directories | Directly verified | Command schemas, output ownership checks and increment 7 evidence |
| Selected snapshots pin dimensions, data, preferences, fonts and renderer | Directly verified | Fixture manifest and expected metadata under `docs/ui-snapshots` |
| Snapshot failure emits full evidence and CI never updates baselines | Directly verified | Negative probes for diff bundles, missing baseline and launch failure; CI policy test |
| CI runs all required gates and uploads failure evidence | Directly verified | Checked-in Verify workflow and successful PR #16 hosted run |
| Focused guides cover all six requested contribution areas | Directly verified | Guides under `docs/ui-guides/`; root guidance remains architectural |
| At least five frozen tasks and a measurable rubric exist | Directly verified | Six tasks and four-dimension 0–2 rubric in `docs/ui-evaluation-seed.json` |

### Application and integration

| Criterion | Classification | Evidence |
| --- | --- | --- |
| Analytical Workspace Lab uses the complete design/control plane | Directly verified | Increment 8 migration, architecture scan, semantic snapshots and selected capture matrix |
| Appearance, contrast, density and font scale are usable and persisted | Directly verified | Physical browser operations, exact saved/reloaded preferences and unit tests |
| Required visual states were inspected | Directly verified | Retained dark, light, high-contrast, narrow, long-text, 150%, focus, loading, error and gallery captures |
| Existing GPU/runtime/workspace/interaction features remain green | Directly verified | Canonical native/browser smokes assert render-plan correspondence, workers, bounded caches, docking, persistence, linked cameras, annotation undo/redo and virtualisation |
| Warmed idle is event-driven and tokens/JSON are not parsed per frame | Directly verified | Application `440 → 440`, gallery `26 → 26`, recorded repaint reasons and generated token architecture |
| Before/after release observations cover the required scenarios | Directly verified | Baseline performance JSON, final two increment 9 observation files and explicit not-present baseline entries for gallery/preferences |
| Canonical verification passes and every criterion is classified | Directly verified | `cargo xtask verify` at observation revision and this matrix |

## Physical and semantic interaction proof

The browser harness physically pans linked views, zooms, edits and undoes a
polygon, scrolls million-row results and the progressive thumbnail grid,
resizes and rearranges the canonical dock, changes display controls, operates
all five preference fields, saves and reloads state, and then proves idle. Its
postconditions compare authoritative cameras, render-plan cameras, command
history, workspace hashes, materialised ranges, bounded caches, selected
records, preference values and semantic audits.

The native harness repeats representative registry-targeted controls, linked
camera movement, exact undo, progressive thumbnails, vertex editing, dock
resize/rearrangement and persistence against GL/llvmpipe. Both final semantic
snapshots contain 76 bounded nodes and empty semantic audits.

## Known unavailable measurements

- Browser GPU timestamps are unavailable for the qualified adapter. The UI
  reports `gpu_timestamp_ms: null` and labels application-update CPU timing
  honestly.
- The headless browser adapter string is empty even though WebGPU rendering,
  draw calls and non-blank output are directly asserted. Hardware WebGPU was
  separately inspected with Lantern during increments 2, 3 and 8.

These are instrumentation limitations, not unverified mandatory behaviours.
There are no blocked mandatory criteria and no input is required to resume.

## Conclusion

The campaign establishes a usage-led design and agent UI loop above egui and
wgpu. Production and gallery share typed components; semantics, geometry,
actions and text are machine-observable; visual changes are deterministically
reviewable; and the full analytical workspace remains operational on native
and browser backends with bounded virtualisation and event-driven idle.

The next major increment can therefore be driven by real application usage.
It does not need another abstract expansion of the framework foundation.
