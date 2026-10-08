# Browser navigation final-action observation

Status: complete
Delivery: [PR #55](https://github.com/robchristie/polyorama/pull/55)

## Outcome and scope

Portfolio Rust 1.99.0 package P1 remains open after
[compiler PR #54](https://github.com/robchristie/polyorama/pull/54) landed at
`e7691e2d457c2db2c7146dbcc33509e2758df88c`. Its reviewed/landed tree matches,
and its exact-head CI passed, but required
[merge CI 37758099129](https://github.com/robchristie/polyorama/actions/runs/37758099129)
failed the separate browser navigation focus helper. Ordinary defensive
verification repair and reviewed landing continue under the original authority;
this is not a new product/platform/performance programme.

The merged Rust 1.99.0 selectors, minima, locks, codec pin, architecture,
application source and every native/browser assertion remain. Scope is only the
browser-owned focus loop, direct regression coverage/canonical wiring and evidence.
No deployment, release, data/package publication, protected operation, optional
tool update or production UI/input/model/repaint change is included.

## Bounded question and evidence

The old loop checks before each of forty-five physical Tabs and waits 60 ms
following each key. It omits the final after-state. The failed merge CI retained
forty-five target observations, final pre-action frame 67 and failure snapshot
frame 69 with Tasks actually focused, Home selected and zero activations. The
opened image shows those states. Exact timing relative to the final 60 ms wait
and individual-event attribution remain unknown; no compiler/product regression
is established.

Read-only advice selects the smallest repair: one final target observation after
the existing loop, extracted into a small browser-owned function. Destination
resolution/missing-node failure stays in the original target callback; all
selection/activation guarantees stay in the physical caller's assertions. Exactly
forty-five actions, every existing 60 ms wait, original Playwright limits and the
full keyboard/disabled/pointer/audit/appearance/clipping/idle predicates remain.
No native forty-action or ten-second contract is transferred here.

Probe owner: this plan, the repair PR and retained browser qualification evidence.
Exit: actual old function fails target-after-action-45; production repair passes
with exactly forty-five dispatches/waits and no extra key. Initial/earlier focus,
unreachable final observation and observation/dispatch/wait failure propagation
also pass. One complete physical browser workflow using existing attributed
1.99.0 Gallery WASM then passes all original predicates, with opened capture.
Reconsider if the final target remains unfocused, focus is lost before Enter,
another invariant fails or success needs more actions/time/production changes.

## Acceptance and landing

The actual-function discriminator fails on clean merged source as expected;
seven production-function cases pass on the selected dirty repair. Those original
source boundaries remain. The committed repair at
`c3aa2d28472247fee737d5157ec546f22814237b`, tree
`cf563a43ba320ea69db7987e1e9f52b0c97a83ed`, passes complete `cargo xtask verify`
on actual Rust 1.99.0 in 331.15 seconds, clean before and after. The new Node
regression runs through the existing explicit xtask list; the complete committed
physical browser workflow also passes.

[Applicability proof](navigation-browser-focus-evidence/browser-focus-applicability.json)
hashes all 135 relevant Rust/Cargo/minimum/lock/toolchain inputs. Only xtask's
explicit new Node-test filename differs; application/crate source remains.
All twenty-seven raw production assets, every HTML entry and the actual
viewer/service/Gallery native hashes remain identical. Unchanged native-nine/
browser-six input/environment/predicate evidence is reused with original source
attribution. Production manifests keep their actual canonical source. Independent
exact-head review, fresh repair CI, protected squash, actual post-merge CI and
four-state/process/preview/tunnel/scratch cleanup remain PR delivery gates.
The owning terminal receipt names retained private observations and monitoring
recoveries; this completed product state does not assert future merge results.


The selected repair meets its exit condition: actual original source fails
with exactly forty-five dispatches/waits, all seven new production-function cases
pass, and the complete physical browser workflow passes seventeen states with
no errors. Existing 1.99.0 Gallery WASM identity, original appearance/clipping/
selection/activation/disabled/pointer/audit/idle predicates and bounds are retained.
The focused capture was opened; this is functional proof on the recorded software
browser route, not a new platform or performance qualification. The complete
owner contract now passes on the exact committed repair. The
[indexed evidence](navigation-browser-focus-evidence/README.md) retains the failed
merge trajectory, before/after source attribution and all limits. No renewed
minimum-compiler run or complete Mac pilot is claimed; prior private-preview
and performance limits remain visible in the original qualification receipt.
