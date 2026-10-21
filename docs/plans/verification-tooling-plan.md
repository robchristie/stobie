# Verification tooling

Status: complete
Delivery: [PR #PENDING](https://github.com/robchristie/stobie/pulls)

`cargo xtask verify` assumed one checkout per host. It started binaries from
the worktree's own `target/` ([#100](https://github.com/robchristie/stobie/issues/100)),
let an unlocked `cargo tree` rewrite Record Desk's lockfile in place
([#101](https://github.com/robchristie/stobie/issues/101)), gave each smoke a
fixed Xvfb display, so two verifications at once collided, and walked Tab a
fixed number of times, so each new Gallery story bumped limits in unrelated
smokes. A browser smoke whose Chromium failed to start reported a `TypeError`
from its failure handler instead of the launch error, each new Gallery story
changed every snapshot baseline, a long `TMPDIR` aborted Node inside a test
rather than failing clearly, and idle checks failed now and then under load
([#108](https://github.com/robchristie/stobie/issues/108)). This plan makes
verification work with any build directory and alongside another run on one
host, and keeps one contributor's change from touching unrelated checks.

## Decisions

- **Ask Cargo where it builds.** xtask reads `target_directory` from
  `cargo metadata` for every artefact it binds or bundles (the production
  package builds into `<target>/browser-cargo`, generated applications into
  `<target>/outside-app`). The smokes resolve it through
  `tools/target_directory.py` and `tools/target-directory.mjs`;
  `tools/native-smoke-lifecycle.sh` resolves both workspaces' directories
  before entering its sandbox, which has no `cargo`, and exports
  `STOBIE_TARGET_DIR` and `STOBIE_RECORD_DESK_TARGET_DIR` to the smokes it
  runs. `CARGO_TARGET_DIR`, Cargo configuration and the default `target/`
  all work, with no symbolic link.
- **Locked consumer checks.** The architecture check's `cargo tree` runs with
  `--locked`, and the Record Desk release build in the `record-desk` stage
  too, so a stale consumer lockfile fails locally as it does in CI, with
  Cargo's own message.
- **A free display per run.** Every Xvfb a smoke starts takes `-displayfd`,
  so the server picks a free display and reports it once it accepts
  connections; `owned_start_xvfb` exports `DISPLAY` then, or fails with the
  server's log if it exits first, and replaces the fixed one-second sleep.
  `tools/owned-process.py --pass-fd` keeps the descriptor open through the
  sandbox. `STOBIE_XVFB_DISPLAY=:N` still fixes a display by hand. The UI
  capture's private Xvfb and the Orca container (which shares the host's
  network, and so its abstract X sockets) do the same. Concurrent smokes in
  one checkout share `.tools/sysroot`, so `tools/bootstrap-linux-ui.sh`
  extracts it under a lock and stamps it with the script's hash.
- **A Tab walk with no count.** egui's Tab order is a cycle, so
  `focusByTab` fails when focus returns to an observed control it already
  visited (the target is not in the cycle) or stays on one control for eight
  steps in a row (`STALL_LIMIT`). Focus outside the observed tree, such as
  the Gallery's catalogue, or on nothing during a wrap, neither stalls nor
  counts; `SAFETY_LIMIT` (1024) only stops a walk that never revisits an
  observed control. Callers report the focused node's ID, and no smoke
  passes a limit.
- **The real launch error.** Each browser smoke's `snapshot` is `async`, so
  its failure handler's `snapshot().catch(...)` handles a page that never
  opened instead of throwing a `TypeError` over the launch error.
- **Baselines a new story leaves alone.** The Gallery records only the
  catalogue entries in view towards native-text coverage, so a story added
  below them changes no fixture's `native_text_controls`; the fixture
  manifest test checks that the baselines are exactly the manifest's
  fixtures, each complete, instead of a fixture count.
- **A socket path too long to bind fails clearly.** `SocketTransport`
  refuses a path over the platform's `sun_path` limit (107 bytes on Linux)
  with `invalid_request`, and the socket test binds under `/tmp` when
  `TMPDIR` would make its path too long, rather than aborting Node.
- **Idle under load.** Under load a browser can hold back the frames an
  interaction scheduled, so they arrive after a quiet window. The ARIA
  layer check settles after axe-core before its idle probe, and
  `observeWarmedIdle` settles once more after one late burst, recording it
  in `late_bursts`; a repaint after the second quiet window fails, and
  painting that recurs within about 1.4 s fails either way.
- **Neutral examples.** The Web Push smoke, tests and guide use neutral
  routes and notification text.

## Acceptance

- `cargo xtask verify` passes with `CARGO_TARGET_DIR` outside the worktree
  and no `target` link, while another checkout's verification runs on the
  same host.
- `tools/tests/test_target_directory.py`: `CARGO_TARGET_DIR` moves both
  workspaces' directories, and the lifecycle exports what Cargo reports.
- `dependency_checks_never_rewrite_a_lockfile` (xtask), and by hand: with a
  stale `consumers/record-desk/Cargo.lock`, `cargo xtask architecture` fails
  with Cargo's `--locked` message and leaves the file unchanged.
- `tools/tests/test_xvfb_display.py`: the reported display is exported and
  the server is owned; a fixed display is passed through; a server that
  exits, or reports garbage, fails with its log; and three sandboxed Xvfb
  servers started at once take three displays.
- `tools/tests/browser-launch-failure.test.mjs`: each of the nine Gallery
  browser smokes, given a missing Chromium, exits non-zero with the launch
  error and no `TypeError`.
- `tools/tests/navigation-browser-focus.test.mjs`: a target behind 600
  controls is reached with no limit, and so is one behind 400 controls
  outside the observed tree; a target outside the cycle fails once focus
  repeats, also with unobserved controls between; a brief unfocused wrap
  continues; focus that stays on one control fails after the stall limit;
  the safety limit stops a walk.
- `tools/tests/browser-idle.test.mjs`: one late burst settles again and is
  recorded; a second repaint fails; a repaint every second fails; with no
  allowance the old strict gate holds.
- `tools/tests/application-client.test.mjs` passes with a `TMPDIR` too long
  for a socket path, and a too-long socket path is refused.
- The fixture manifest test, and `cargo xtask ui verify` with the 39
  re-rendered `text.json` baselines, whose only change is
  `native_text_controls`.

## Evidence

- At `6e3f207`, with `CARGO_TARGET_DIR` outside the worktree, the
  `native-lab` stage failed with "application exited before window
  appeared" (#100).
- The idle flake did not recur in 100 runs of the WebGL2 status-chip smoke
  (8 and 20 alone, and six at once for 12 runs each) at `6e3f207`. The
  late-burst allowance is bounded by its tests rather than by a reproduction.
- The Gallery's release WASM before wasm-bindgen grew by 56 bytes
  (13,922,343 to 13,922,399), and by Brotli (quality 11) shrank by 151 bytes
  (3,266,200 to 3,266,049): the only application change is one visibility
  check in its catalogue.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Target directory and locked consumer checks | Complete | Target directory tests, xtask test, stale lockfile check |
| Free Xvfb displays | Complete | Xvfb lifecycle tests, concurrent verification |
| Tab walks, launch errors and baselines | Complete | Focus, launch failure and fixture manifest tests |
| Long `TMPDIR`, idle under load and neutral examples | Complete | Client, idle and push tests, `cargo xtask verify` |
