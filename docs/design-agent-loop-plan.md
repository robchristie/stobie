# Polyorama design system and agent UI loop plan

Status: active; baseline candidate complete

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

The baseline and design audit is complete and awaiting ordinary review and
landing. The clean baseline passed `cargo xtask verify` on 29 August 2026. The
next coherent increment is the visual language and token compiler, including a
single isolated production consumer. Do not restyle the complete application
until typed tokens and measured text primitives exist.

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

Baseline verification currently overwrites several tracked evidence files with
run-dependent timings. Those changes were discarded after recording the pass.
The tooling increment must redirect generated run artefacts to an explicit,
ignored output directory before adding CI evidence publication.

## Audit findings

- The application has one partial `configure_style` function but no semantic
  tokens or component recipes.
- Seventeen static production UI colours remain outside generated scientific
  thumbnail data; typography and component geometry use scattered literals.
- Dock tab width is `title.len() * 7.2 + 22`, the strongest current violation
  of measured-text requirements.
- Custom tabs, splitters, thumbnail cells and annotation controls expose
  physical geometry but no explicit AccessKit metadata.
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
| 1 | Baseline, audit, goal and campaign control plane | — | Candidate complete | This plan and baseline gate |
| 2 | Visual language, token compiler, generated themes, preferences seed | 1 | Pending | PR and token tests |
| 3 | Measured text roles, overflow, observations and layout audit | 2 | Pending | Text fixtures and audit output |
| 4 | Reusable accessible shell components and keyboard focus | 2–3 | Pending | Component tests and captures |
| 5 | Native/browser gallery, stories and reference scenes | 3–4 | Pending | Gallery captures and manifests |
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

Complete increment 1, independently review and land it, then dispatch a fresh
bounded implementation context for increment 2. The token increment must prove
validation, alias resolution, cycle detection, deterministic generation,
no-drift checking, coherent variants and one isolated sample consumer before
application-wide migration begins.
