# Application composition qualification

The bounded public composition journeys are qualified on executable candidate
`a298badac262b531dc2db0c5beefa8528dd628b0`, tree
`91192c067c08ffc7686ce7e1c85b47f0c4dc199a`, based on
`abbe2162ffbbbd2570f4534bb94e2ebf415b898d`. The
[composition guide](application-composition.md) owns public instructions; the
[plan](api-documentation-plan.md) retains the frozen P1–P3 supporting map.
Independent public fresh-reader acceptance also succeeds on
`34a6cfc533ca85647761b7d72f413fa9e4d2e439`, tree
`8eff8f7c930cac04c2da8c44f1fd75a9f6913a1e`; its executable inputs are unchanged.
[PR #45](https://github.com/robchristie/polyorama/pull/45) retains separate final
exact-head source review, required CI, merge and cleanup observations.

The [retained archive](api-documentation-evidence/observations.tar.gz) contains
100 files: actual receipts, complete command logs, runtime snapshots, nine
rendered API captures and inspection recipes. The
[index](api-documentation-evidence/archive-index.json) records each file's size
and SHA-256 plus the compressed archive hash. Extract with
`tar -xzf docs/api-documentation-evidence/observations.tar.gz -C <inspection-directory>`.
Generated HTML is excluded. Archive path names below are relative to the extracted
root; absolute paths inside raw receipts describe the observed host, not portable
consumer instructions.

| Requirement | Actual result | Retained proof |
|---|---|---|
| P1: four useful crate entries and local contracts | Documentation generates with rustdoc warnings denied; each defining crate executes exactly one example, with no ignored, failed or filtered examples | `qualification/command-log-4yoga4ld/`; `qualification/canonical.log` |
| P1: rendered navigation and key items | Chromium opens all four entries plus `validate_intent`, `Runtime`, `ImageRenderRequest`, `PanePresenter` and `PresentationContext`; contract text and example formatting are visible; UI entry → `PanePresenter` → `PresentationContext` navigation succeeds; all nine images inspected | `rendered/inspection.json`, nine `rendered/*.png` and body extracts; `qualification/inspect-rustdoc.mjs` |
| P2: portable source | Maintained `minimal-workspace` compiles natively and on `wasm32-unknown-unknown` using the existing package dependencies and public framework APIs | `qualification/command-log-4yoga4ld/output.log`; exact-candidate `qualification/canonical.log` |
| P2: actual native actions | Physical pointer clicks produce annotations/history/displayed count `0 → 1 → 2`; two project-authored blue polygons are visible; all text audits are empty | `native-canonical/minimal-native-{before,after-1,after-2}.json`, screenshot/runtime log; `native-focused/` |
| P3: documented presenter variation | The exact one-line badge recipe compiles, displays its badge, and preserves both physical actions and empty text audits; source is restored afterwards | `variation/receipt.json`, complete build/smoke logs, snapshots and screenshot; `qualification/qualify-variation.py` |
| Selected link/example enforcement | 41 selected local links pass; five checker regressions pass; deliberate missing guide link is rejected; deliberate unresolved core-example import fails with exit 101, then source is restored | `qualification/command-log-q7lb2lh2/`, `qualification/command-log-tabepu_c/`, broken-link/example receipts and full example log |
| P3: independent public fresh reader | Starting at README/guide/rustdoc, located the seam, rebuilt and physically exercised original and exact badge variation without Lab implementation or author explanation; both journeys show counts/history 0 → 1 → 2 and empty text audits; both images opened | `fresh-reader/reader-report.md`, `identity.json`, four successful command receipts/full logs, original/variation snapshots and screenshots |
| Complete owner gate | `cargo xtask verify` passes on the clean committed candidate, exit 0, 247.166 seconds; all original format/lint/test/architecture/native/browser/deterministic UI obligations remain | `qualification/canonical.json`, `qualification/canonical.log` |

## Environment and input attribution

The canonical host is x86_64 Linux, rustc/Cargo 1.98.1, Node.js 25.8.2,
wasm-bindgen 0.2.127 and the repository's pinned Binaryen 131. Rustdoc generation
and doctest execution are separate operations. The four defining crates and
selected source/Markdown consumers are explicit in `tools/check-api-docs.py`;
this is a bounded coverage policy, not an exhaustive export or Markdown audit.
`cargo xtask docs` owns focused generation, doctests, native/WASM example checks
and links. Full verification additionally builds the native example in release
mode and, on Linux, invokes `tools/minimal-native-smoke.sh` using the established
owned-process/Xvfb route. Existing verification classification is unchanged.

The native example and variation use eframe/wgpu with GL Mesa llvmpipe
(LLVM 22.1.8), recorded in the actual runtime logs. Canonical browser checks use
the existing headful Chromium route. The rendered rustdoc captures use the
repository's Playwright dependency and an existing font directory through a
private Fontconfig file/cache; no installation or global configuration changed.
Retained scratch wrappers describe this host's bundled-library environment and
are observations, not a new maintained verification runner.

`qualification/qualification-inputs.json` records source/tool/lock identities.
`Cargo.lock`, `package-lock.json`, manifests and toolchain settings are unchanged.
The viewer codec remains the locked Git revision
`6586e3d50f95429b242cb2e3535742b002784f2d`; verification consumes no mutable
sibling checkout. The early focused documentation/native evidence is attributed
to its original source. Documentation inputs are unchanged on the canonical
candidate; the canonical log independently repeats doctest/compile/native proof
on that exact committed head. The variation receipt binds its temporary source
delta and hash to the canonical candidate. Later evidence/plan-only commits do
not relabel generated build identities; final-head CI and review are retained
by the owning PR.

Initial full attempts failed because the isolated host view lacked shell/core
utilities and the worktree's pinned Binaryen cache path. The retained failure
logs are `qualification/canonical-{initial,shell-tools,missing-binaryen}.log`.
The final run exposes existing utilities/cache through its private environment
and passes without dependency, lock, source-behaviour or global host changes.

## Independent reader attribution

The reader materialised committed source in its own scratch directory and reused
existing dependency/UI caches; Cargo rebuilt the four framework crates and
application. It read the public README, composition guide, pane/interaction
guides, rendered entries/key-item contracts and maintained smoke scripts. It
did not inspect Lab/framework implementation or author receipts. The archive
retains its actual source/variation hashes, commands, observations and report.

Its first smoke attempts failed because the private `/tmp` mount hid a source
copy beneath `/tmp`. The reader added a read-only bind of that scratch root after
the private mount; both successful smokes used the same isolation adaptation.
The modified scratch lifecycle recipe and failed command receipts are retained.
This changed no committed harness or consumer code. Fresh-reader success proves
public usability for the frozen journey; it is separate from final source
correctness review and makes no additional WASM/runtime/performance claim.

## Boundaries

This qualifies a small native public consumer and its presenter variation.
WASM proof is compilation only: its `main` has no browser bootstrap, canvas,
worker transport or browser package. Existing browser application smokes do not
turn this example into a browser app. Native software-GL observations are
functional evidence, not GPU performance or all-platform certification. The
example has no persistence or asynchronous scalar-image pipeline; that
multi-step guide describes existing source contracts and points to their
relevant API items and regional adapter contract.

No APIs, visibility, runtime behaviour, compiler/dependency settings or large
application implementations changed. Transitive exports, arbitrary image
sources, new docking/scheduling/rendering capabilities, broader browser support
and publication remain outside the frozen supporting map. No newly discovered
runtime defect requires a separate repair in this package.
