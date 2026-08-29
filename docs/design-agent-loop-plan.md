# Polyorama design system and agent UI loop plan

Status: active; increment 4 landed; increment 5 implementation candidate

Baseline revision: `b8c66317aaa9284c45e712278010bc9cd285c01b`

Normative outcome: [design-agent-loop-goal.md](design-agent-loop-goal.md)

Reorientation budget: 200 lines. Detailed implementation, review and evidence
belong with the semantic owner or pull request rather than in this plan.

## Scope and authority

Deliver the complete design-system and agent-observable UI foundation without
new analytical capability or changes to the egui/wgpu substrate. Ordinary
code, tests, documentation and CI changes may land under standing repository
authority after exact-head independent review and successful gates. Release,
deployment, publication, breaking compatibility, rights or security-policy
changes remain human-review boundaries.

## Current phase

Increment 4 landed as PR #12 at merge `f551711`: token-aware visual versus
minimum-hit geometry, bounded visible tabs plus overflow, focus rings, stable
domain-derived widget IDs, same-frame selected semantics, roving tab keys and
exact-step keyboard/AccessKit splitter adjustment. Its final repair cancels
egui's pending spatial navigation in the same pass as an owned arrow, so first
and consecutive inputs need no settling frame. Native physical resize,
undo/redo and out-and-back no-op evidence passes. This remains compatible egui
AccessKit tree generation only; OS/browser screen-reader delivery is not
claimed.

Increment 5 now has a runnable native/browser gallery candidate. A closed,
serialisable Rust catalogue owns 18 stable stories, including the required
button, dock, toolbar, property, status, virtual-grid and six composed
reference scenes. The app supports light/dark, standard/high contrast,
compact/comfortable density, 100/125/150% font scale and narrow/regular/wide
story surfaces without a runtime UI DSL. Gallery scenes call the production
dock and token-derived action, property, result, status and thumbnail recipes;
they use bounded deterministic fixture data and never materialise a logical
collection. A four-configuration representative matrix renders every story
headlessly with an empty measured-text audit. Browser WebGPU and native
GL/llvmpipe launch smokes retain manifests, Rust snapshots and selected
captures; the warmed browser frame counter remains stopped while idle.

The UI layer owns typed measured-text
roles, five explicit overflow policies, horizontal and vertical alignment,
bounded line counts, responsive pane classes, serialisable observations and a
deterministic one-point-tolerance audit. Dock tabs use egui galley measurement,
single-line ellipsis, full semantic names and stable geometry without the old
character-count width estimate. Application `TestSnapshot`/`UiGeometry`
exports eight bounded tab observations and the audit without enumerating
ordinary labels or virtualised collections.

The last landed canonical gate passes 117 tests, token drift, architecture,
native and Wasm clippy, release native/Wasm builds, browser WebGPU and native
GL/llvmpipe physical smokes. Increment 5 focused checks add three catalogue /
matrix tests, two idempotent-style tests and production-component semantic
coverage. Exact-head canonical verification remains the candidate landing
gate. Complete shell/content migration remains increment 8; action identity
and the reusable semantic snapshot remain increment 6.

## Baseline evidence

- revision: `b8c66317aaa9284c45e712278010bc9cd285c01b`;
- `cargo xtask verify`: pass;
- tests: 83 pass (19 app, 32 core, 9 renderer, 14 runtime, 9 UI);
- release native build: pass;
- release Wasm application and worker build: pass;
- browser smoke: pass, WebGPU/WGPU, four render jobs, five completed workers;
- native smoke: pass, GL/llvmpipe at 1440×900;
- idle: browser warmed frame counter stopped and repaint reason was `None`;
- environment: cargo 1.97.1, rustc 1.97.1, Node 25.8.2, npm 11.11.1,
  wasm-bindgen 0.2.127, egui/eframe 0.36.1, wgpu 30.0.1;
- existing captures: [vertical-slice-evidence](vertical-slice-evidence).

Baseline verification overwrote several tracked evidence files with
run-dependent timings. Increment 3 corrected the canonical verifier to route
both physical smokes into an explicit ignored output directory; CI publication
of selected evidence remains increment 7.

## Audit findings

- The application has one partial `configure_style` function but no semantic
  tokens or component recipes.
- Seventeen static production UI colours remain outside generated scientific
  thumbnail data; typography and component geometry use scattered literals.
- Dock tab width used `title.len() * 7.2 + 22`; increment 3 replaces this
  violation with egui galley measurement and an architecture source check.
- At baseline, custom tabs, splitters, thumbnail cells and annotation controls
  exposed physical geometry but no explicit AccessKit metadata. Increment 4's
  candidate covers the dock tabs and splitters only.
- Standard buttons provide egui semantics, while keyboard routing is local and
  there is no action registry.
- `TestSnapshot` plus bounded `UiGeometry` is the correct cross-platform seed
  for the semantic snapshot; it must not become a second application model.
- Egui has AccessKit support, but the app's explicit eframe feature list does
  not currently enable the native adapter or web screen reader.
- `egui_kittest` is not locally cached or locked; compatibility must be probed
  against egui 0.36.1 without downgrading the stack.
- Existing physical smoke already targets current Rust-owned geometry for
  migrated controls; preserve that contract.

## Delivery graph

| Increment | Outcome | Depends on | Status | Durable evidence |
| --- | --- | --- | --- | --- |
| 1 | Baseline, audit, goal and campaign control plane | — | Landed | PR #9, this plan and baseline gate |
| 2 | Visual language, token compiler, generated themes, preferences seed | 1 | Landed | PR #10, token tests, generated Rust, design language and [capture](design-agent-loop-evidence/README.md) |
| 3 | Measured text roles, overflow, observations and layout audit | 2 | Landed | PR #11, text fixtures, exported tab observations and empty audit |
| 4 | Reusable accessible shell components and keyboard focus | 2–3 | Landed | PR #12, 31 focused UI tests, AccessKit tree checks and native physical smoke |
| 5 | Native/browser gallery, stories and reference scenes | 3–4 | Candidate; focused native/browser evidence passes | 18-story manifest, matrix tests and selected gallery captures |
| 6 | Action registry and reusable semantic snapshot | 4–5 | Pending | Semantic/keyboard tests |
| 7 | `xtask ui`, snapshot artefacts, CI, guides and eval seed | 3–6 | Pending | CI runs and failure bundle probe |
| 8 | Full Analytical Workspace Lab migration and visual selection | 2–7 | Pending | Required capture matrix |
| 9 | Final performance/idle/native/browser hardening and report | 8 | Pending | Final report and canonical gate |

The graph may be refined only at phase boundaries when evidence reveals a
better coherent dependency boundary. Every increment must leave the workspace
buildable and relevant native/Wasm applications runnable.

## Evidence loop

After each material change:

1. run the smallest affected tests and native/Wasm build;
2. render affected deterministic stories;
3. inspect semantic and text-layout output;
4. inspect screenshots and visual diffs;
5. run relevant physical native/browser interaction checks;
6. record the exact revision and observed result with its semantic owner; and
7. select the next action from the strongest remaining failure.

Profile before architectural optimisation. Never adopt a changed baseline
until geometry, text observations and the responsible token/component source
show that the change is intentional.

## Next action

Run the complete exact-head gate for increment 5, inspect the regenerated
application and gallery evidence, independently review and repair the same
head, then land it. Begin the action registry and reusable semantic snapshot
only after this gallery contract is stable. Application-wide migration remains
deferred.
