# Polyorama working rules

## Boundaries

- `polyorama-core` owns document, session, canonical dock-tree, commands and
  renderer-independent demand types. It must not depend on egui, eframe, wgpu or
  browser crates.
- `polyorama-runtime` owns demand reconciliation, workers and completion state;
  it must not depend on egui or wgpu.
- `polyorama-render-wgpu` owns all persistent GPU resources and typed render
  requests. No pane or viewport may create a device or queue.
- `polyorama-ui-egui` is the sole framework crate that understands egui. Pane
  functions receive narrow views and feature intent/demand/render sinks, never
  mutable access to the complete application model or runtime.
- The serialisable `polyorama_core::Workspace` is the only authoritative dock
  tree. Do not introduce a second stateful docking tree.

## State and interaction

- Durable annotations belong to the document; selection, camera links, tools
  and gesture previews belong to the session; focus and hover stay in the UI.
- Route changes through feature intents and validated commands. One completed
  gesture creates one command and one undo record; render its session preview in
  the same frame.
- Derive semantic widget IDs from stable pane and domain IDs.
- Preserve typed coordinate newtypes across domain, UI and renderer boundaries.
- Demands are idempotent desired state. Reconcile and deduplicate before
  scheduling work; reject stale generations.
- Repaint only for a recorded reason. Never add an unconditional frame repaint.

## Verification and instrumentation

- Run focused tests and the relevant native/WASM build after each increment.
- Instrument before optimising; profile a representative release scenario
  before changing architecture for performance.
- Keep diagnostics honest: report unavailable GPU timing as unavailable.
- Keep execution plans consistent with [docs/plan-lifecycle.md](docs/plan-lifecycle.md).
- Full verification is `cargo xtask verify`. It must retain formatting, lint,
  tests, release native build, WASM build, architecture checks and browser smoke.
- Do not allocate the complete raster, one million result rows, or one hundred
  thousand thumbnail widgets.

For ordinary README, plan lifecycle prose and `docs/*-plan.md` closeout changes,
`python3 tools/verify.py --base <comparison-commit>` is the scoped alternative.
It derives committed, staged, unstaged and untracked scope from Git, checks
whitespace, runs classifier and plan lifecycle regressions, and checks all plan
fields. Every other path or unknown comparison state runs `cargo xtask verify`.
Keep qualification reports, snapshots and evidence on the full surface. CI uses
the same guard for pull requests and main pushes under the required `verify`
check. Scope selection does not establish review, merge or acceptance evidence.

## UI guidance

- Start UI implementation and review from [docs/ui-guides/README.md](docs/ui-guides/README.md);
  use the frozen [UI evaluation seed](docs/ui-evaluation-seed.md) for repeatable
  component and semantic evidence.
