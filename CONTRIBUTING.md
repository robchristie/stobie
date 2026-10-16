# Contributing to Stobie

These rules apply to every change to Stobie. `cargo xtask architecture` and the
test suite enforce part of them; review covers the rest. The
[documentation index](docs/README.md) lists the guides, and execution plans live
in [docs/plans/](docs/plans/).

## Crate boundaries

- `stobie-core` owns what every application needs: the canonical dock tree,
  stable pane IDs, generic coordinates, virtualisation, repaint reasons and the
  generic undo history. It must not depend on egui, eframe, wgpu, browser
  crates or the image domain.
- `stobie-protocol` owns the inspection wire types, argument contracts and
  the semantic, action and text observation types they carry. It must not
  depend on egui, eframe, wgpu, browser crates or `stobie-ui-egui`; an optional
  `emath` feature converts egui geometry.
- `stobie-imaging` owns the image domain: document, session, cameras, image
  intents and commands, and renderer-independent tile and regional demands.
  Applications without images do not depend on it.
- `stobie-runtime` owns demand reconciliation, workers and completion state;
  it must not depend on egui or wgpu.
- `stobie-render-wgpu` owns all persistent GPU resources and typed render
  requests. No pane or viewport may create a device or queue.
- `stobie-ui-egui` is the sole framework crate that understands egui. Pane
  functions receive narrow views and feature intent/demand/render sinks, never
  mutable access to the complete application model or runtime.
- The serialisable `stobie_core::Workspace` is the only authoritative dock
  tree. Do not introduce a second stateful docking tree.

## State and interaction

- Durable annotations belong to the image document; selection, camera links,
  tools and gesture previews belong to the image session; focus and hover stay
  in the UI. Application demo state belongs to its application.
- Route changes through feature intents and validated commands. One completed
  gesture creates one command and one undo record; render its session preview in
  the same frame.
- Derive semantic widget IDs from stable pane and domain IDs.
- Preserve typed coordinate newtypes across domain, UI and renderer boundaries.
- Demands are idempotent desired state. Reconcile and deduplicate before
  scheduling work; reject stale generations.
- Repaint only for a recorded reason. Never add an unconditional frame repaint.

## Verification

- Run focused tests and the relevant native/WASM build after each increment.
- Full verification is `cargo xtask verify`. It must retain formatting, lint,
  tests, release native build, WASM build, architecture checks and browser smoke.
- Keep execution plans consistent with [docs/plan-lifecycle.md](docs/plan-lifecycle.md).

For ordinary README, plan lifecycle prose and `docs/plans/*-plan.md` closeout changes,
`python3 tools/verify.py --base <comparison-commit>` is the scoped alternative.
It derives committed, staged, unstaged and untracked scope from Git, checks
whitespace, runs classifier and plan lifecycle regressions, and checks all plan
fields. Every other path or unknown comparison state runs `cargo xtask verify`.
Keep qualification reports, snapshots and evidence on the full surface. CI uses
the same guard for pull requests and main pushes under the required `verify`
check. Scope selection does not establish review, merge or acceptance evidence.

## Git history

Never rewrite the history of `main`: no force-push, and no filtering, amending
or rebasing of merged commits. Applications made with `stobie new`, the
[emuella-viewer](https://github.com/robchristie/emuella-viewer) repository and
other consumers pin exact Stobie commits. A rewrite leaves those commits
unreachable from any branch, so their builds eventually fail. The two earlier
rewrites and their commit maps are recorded in
[docs/evidence-archive.md](docs/evidence-archive.md#history).

## Evidence and captures

Never commit raw captures such as screenshots, logs or runtime observation
dumps. Write them under ignored `.tools/runtime/`, archive durable ones in
stobie-evidence and keep conclusions and permalinks in Markdown. Only small
protocols, receipts and result summaries that tools use or reports cite may
stay tracked. See [docs/evidence-archive.md](docs/evidence-archive.md).

## Performance and diagnostics

- Instrument before optimising; profile a representative release scenario
  before changing architecture for performance.
- Keep diagnostics honest: report unavailable GPU timing as unavailable.
- Do not allocate the complete raster, one million result rows, or one hundred
  thousand thumbnail widgets.

## UI work

Start UI implementation and review from [docs/ui-guides/README.md](docs/ui-guides/README.md);
use the frozen [UI evaluation seed](docs/ui-evaluation-seed.md) for repeatable
component and semantic evidence.
