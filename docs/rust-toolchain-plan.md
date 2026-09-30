# Rust 1.98.1 maintenance

Status: complete
Delivery: [Polyorama PR #43](https://github.com/robchristie/polyorama/pull/43)

## Outcome and scope

Programme package `rust-1.98.1` / P1 selects exact Rust 1.98.1 for Polyorama
development, normal CI and the production browser compiler contract. The
authorised maintenance includes selectors, maintained setup guidance, necessary
compiler repairs and existing native/WASM/browser/viewer acceptance. It preserves
behaviour, public APIs, lints, dependencies, editions and resolver policy.
No release, deployment, package/data publication, protected-resource operation,
machine/default-toolchain change or other-project write is included.

The starting product is `e1f31e11bc3210e763cea1b1cbd5f23c02356db4` (tree
`d654955c046686dc6dbc270c6c42acfb4dcc3f53`), entered through the registered
Polyorama workspace. The programme prerequisite is
[portfolio baseline PR #3](https://github.com/robchristie/portfolio-workspace/pull/3).
[E1's completed owner plan](https://github.com/emuella/emuella-workspace/blob/9cb91a253e38111348b925c52f18debde8671aa4/docs/plans/completed/rust-1-98-1.md)
and [terminal audit](https://github.com/emuella/emuella-workspace/pull/101#issuecomment-5911765234)
establish the upstream migration. E1 requires no API or consumer-lock change;
Polyorama retains codec revision `6586e3d50f95429b242cb2e3535742b002784f2d`.

## Inventory and compatibility

The eleven-member workspace initially has no root or scoped toolchain file.
Normal CI and the production browser guard are the operational 1.97.1 selectors.
This package adds a root rustup pin with Rustfmt, Clippy and the WASM target and
updates those two selectors plus README, browser startup and CI cache guidance.
The root Cargo minimum stays 1.97.1. Nine members inherit it;
`emuella-viewer-source` and `emuella-viewer-tools` retain their omitted declarations.
Historical measurements, qualification reports, archived build identities and
benchmark/provenance references remain attributed to their original compilers.

## Acceptance and sequence

1. Inspect all operational selectors and actual compiler dispatch; use focused
   formatting, xtask tests and native/WASM lint feedback for necessary repairs.
2. Run the existing complete `cargo xtask verify` contract under 1.98.1,
   retaining actual logs and generated verification observations. Historical
   1.97.1 PR #42 evidence does not qualify this compiler.
3. Establish applicable 1.97.1 native/WASM compatibility for changed source
   inputs and the retained lock, then exercise the existing deterministic viewer
   service through native and real browser-worker journeys on the candidate.
4. Complete the plan and programme disposition in the product PR, obtain
   independent exact-head review, then land with CI and post-merge CI. The PR's
   landing comment owns future merge identities and four-state/scratch cleanup.

Dependency order is E1 terminal evidence → P1 compiler/compatibility qualification
→ viewer consumer proof → reviewed product landing → portfolio reconciliation.
Polyorama owns every product write and long-running check/CI watcher.

## Delivery and closeout

| Increment | Status | Result |
| --- | --- | --- |
| Exact compiler selectors and maintained setup | Complete | Development, normal CI and production browser contract select 1.98.1; Cargo/README minimum remains 1.97.1 |
| Necessary compiler repairs | Complete | Fixed-array chunk iteration satisfies new Clippy lint without changing remainder behaviour |
| Full owner acceptance and retained minimum | Complete | Clean committed source `50a14dc5650b15cea569da8caa7e92947e5314ee` passes complete `cargo xtask verify`; locked 1.97.1 native/WASM checks and all 13 viewer-source tests pass |
| Shared viewer adapter and consumer journeys | Complete | Ceiling request offsets match the codec's reduced sample grid; strict admission retained; native nine-input/30-stage and ordinary browser nine-input/16-state journeys pass; complete unchanged six-input browser pressure gate passes |
| Durable P1 disposition | Complete | Full positive/negative observations, input/compiler/build identities and bounded qualification are retained in [owner evidence](rust-toolchain-evidence/README.md); PR #43 owns independent review, CI, merge and cleanup |

P1 has achieved its bounded product outcome. Normal selection is exact 1.98.1;
the previous minimum, optional declaration omissions, API/dependency/lint policy
and codec lock remain. Historical 1.97.1 reports keep their original attribution.
E1's terminal owner evidence is accepted without rerunning upstream components.
No workspace PR, other-project write, machine/default-toolchain change, release,
deployment or package/data publication is part of this delivery.

The viewer runtime probe exposed a pre-existing edge-window mismatch: old
1.97.1 browser/service builds reproduced it with byte-identical fixture payloads
and descriptors. The minimal adapter repair changes the request start from floor
to ceiling projection, matching the codec's existing authenticated half-open
demand. Regression proof exercises the emitted request and service effective
region, checks original-demand pixels/masks and preserves empty-output rejection.
A data favicon avoids an incidental request outside the service's closed routes.
No admission guard, pressure cap, cancellation or eviction assertion is weakened.

The original nine-input pressure probe correctly failed GPU eviction because it
reached only 9,170,944 bytes under a 16 MiB cap. The service sorts catalogue
targets, so an attempted argument reorder was rejected by the input identity
guard before running the browser. The final frozen subset uses original indices
`0,1,3,5,7,8`, retaining all bytes/identities and placing RGB images at the
pressure-visited positions. The complete unchanged harness passes: 62 GPU
evictions, four representation evictions, two worker abort acknowledgements, one
interrupted real JPP transfer, no errors and all caps respected. This proves
bounded six-input browser pressure coverage; native nine-input and ordinary
browser nine-input observations remain separately attributed.

[Retained receipts and complete observations](rust-toolchain-evidence/README.md)
identify actual source `50a14dc5650b15cea569da8caa7e92947e5314ee`, tree
`9f56909a825554a28e3dc3b6669627956e918240`, compiler dispatch, dependencies,
commands, environment and input hashes. Later plan/evidence-only commits reuse
that unchanged executable surface; production build manifests keep their actual
source revision. The owning PR supplies exact reviewed head, required CI and
post-merge CI on their actual revisions. Plan completion records a verified
product state, not a claim that this PR has already merged.

Remaining qualification limits belong to the existing protected-image and
representative system/performance plans. These public functional journeys and
observed software adapters establish no unrestricted browser/input or target
GPU performance claim. The initial host headless-startup failure is retained;
final full acceptance uses the existing headful route used by normal CI. There
is no unresolved compiler-package implementation finding.
