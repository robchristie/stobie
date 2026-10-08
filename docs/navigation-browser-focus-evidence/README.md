# Browser navigation final-action observations

This is the bounded continuation of portfolio Rust 1.99.0 package P1 after
[compiler PR #54](https://github.com/robchristie/polyorama/pull/54) merged at
`e7691e2d457c2db2c7146dbcc33509e2758df88c`. The compiler change remains qualified;
its [actual merge CI](https://github.com/robchristie/polyorama/actions/runs/37758099129)
failed a separate browser observer. The new repair's canonical source is clean
`c3aa2d28472247fee737d5157ec546f22814237b`, tree
`cf563a43ba320ea69db7987e1e9f52b0c97a83ed`. Later documentation commits do not
relabel that build or its production manifest.

[observations.tar.gz](observations.tar.gz) contains twenty-five files, including
the complete failing merge-CI log, structured failure and opened capture,
actual-source fail-before probe, seven production-function cases, focused and
committed canonical browser reports/captures, complete canonical log/receipt,
production manifest, actual helper/caller/test/xtask source and applicability
proof. [archive-index.json](archive-index.json) binds every member's size/SHA-256
and archive SHA-256 `b3648dee83ca2fcd8753ede9e44b3097830df8c00ae8f619ca1b3fc4b5478b8f`.
No input payload, executable, profile, cookie or protected material is included.

## Failure and minimal repair

The old loop observes before each of forty-five physical Tabs, with a 60 ms wait
after each key. Its final key has no after-state observation. The failed merge
run retained forty-five target observations, last pre-action frame 67, then a
failure snapshot at frame 69 with Tasks focused, Home selected and zero
activations. The opened image confirms the visible states. Exact timing relative
to the final 60 ms wait and individual-event attribution remain unknown. This
evidence establishes an omitted observation; it does not establish a compiler
or production UI regression.

The browser-owned helper adds only the final target read after the existing
loop. Its production caller retains destination resolution/missing-node errors,
physical Playwright Tab dispatch and waits. Every selection/activation assertion
remains in the caller. The helper makes no new selection/activation guarantee.
Exactly forty-five actions, each existing 60 ms wait, original Playwright bounds
and all focus/Enter/Space/disabled/pointer/audit/appearance/clipping/idle
predicates remain. The native forty-action, single-ten-second and semantic-focus
contracts are separate and unchanged.

## Actual qualification and source boundaries

The probe extracts the actual clean merged function and runs controlled target,
physical-key and awaited-wait callbacks. Target becomes focused after wait 45;
the old function throws after exactly forty-five dispatches and forty-five
60 ms waits. The repaired production function accepts that final observation
without a forty-sixth key. All seven direct cases pass: initial and earlier
target, final target, unreachable final observation, and observation/dispatch/
wait rejection propagation without another traversal.

The initial focused unit/physical receipts correctly record merged `e769…`
with the selected dirty repair. That complete physical workflow passes seventeen
states in 9.992 seconds, without errors, on Chromium 151.0.7922.34 headful software
rendering and existing attributable Rust 1.99.0 Gallery WASM SHA-256
`4eba76d7e5ce209fc52364de0c136b800cb9900a5d4a5f9e64a360e9cbb0efc6`.
The focused capture was opened. These observations are retained with their
original attribution; the committed canonical run supplies the clean-source
integration proof.

[Canonical receipt](browser-focus-canonical.json) binds clean committed `c3aa…`,
actual rustc/Cargo 1.99.0 and the recorded four-job Linux/bundled-library/font
environment: complete `cargo xtask verify`, exit zero in 331.15 seconds.
Formatting, native/WASM lint/tests/releases, architecture/API documentation,
fourteen snapshots, every native/browser workflow and independent Record Desk
pass. The new seven cases run through xtask's explicit Node list. The committed
browser report and opened capture again show Tasks focused with Home selected.
[Production manifest](browser-focus-production-manifest.json) keeps the actual
canonical commit rather than a later evidence head.

[Applicability](browser-focus-applicability.json) hashes all 135 relevant Rust,
Cargo/minimum/lock/toolchain inputs against initial application qualification.
Only `xtask/src/main.rs` differs: its sole change adds the new Node test filename.
Application/crate Rust and Cargo inputs remain unchanged. All twenty-seven raw
production assets, every HTML entry and the actual viewer/service/Gallery native
hashes are unchanged. Earlier immutable nine-input/native-thirty-stage and frozen
six-input/browser-pressure evidence therefore remains applicable with its
[original source and limits](../rust-1-99-0-evidence/README.md). The earlier three
archives and both native CI failure trajectories remain intact.

No renewed minimum-compiler run, complete Mac pilot, representative/protected
image qualification, broader GPU or performance claim is made. The original
private preview's incomplete Thumbnails/Diagnostics/reload observation and
unavailable transport follow-up remain visible. Independent exact-head review,
fresh required CI, live merge checks and actual merge-revision CI remain distinct
delivery gates, recorded by the repair PR's terminal receipt.
