# Verification friction calibration

This bounded change follows the
[workflow intervention checkpoint](https://github.com/robchristie/rob-codex-workflow/blob/ecbc16ce3522c017560bd1f4d1637c1f4a9360f6/docs/workflow-interventions/0009-verification-and-goal-friction.md).
The implementation and regressions in this repository own the component
evidence; review, exact committed candidate proof and landing remain with the
owning pull request.

## Question and smallest probe

Starting source revision: `72fc3fe0746c505b8a2d2743d81acc6e46adea71`.
Can terminating the background `ui_sandbox` wrapper leave its native application
or Xvfb descendants alive, and can an explicitly owned lifecycle clean them up
without affecting unrelated processes?

The smallest probe launches a wrapper with a sleeping child, terminates the
wrapper and inspects the child. It reproduced a surviving child; the probe then
terminated and reaped that child. The regression fixture extends the probe with
a nested orphaned grandchild that ignores TERM and an unrelated sentinel.
Exit criteria are bounded termination and reaping on success, repeated runs,
failure, restart, SIGTERM and SIGINT, followed by both real native smokes.

## Selected implementation

Retain a small shared Linux supervisor and shell lifecycle. Each background
sandbox starts in its own process group. The supervisor acts as a child
subreaper, sends TERM then KILL with bounded grace periods and reaps adopted
descendants. Both scripts install exit and signal cleanup before launching Xvfb;
the native application restart uses the same stop-and-wait path. Sandbox
launches also use `--die-with-parent`. No process-name matching or global kills
participate in cleanup.

Retain the Git-derived verification guard. Only `README.md`,
`docs/plan-lifecycle.md` and top-level `docs/*-plan.md` qualify for scoped
verification. All other paths, unsupported modes and unknown state select full
verification. Separate committed, staged and unstaged comparisons prevent a
working-tree reversal from hiding a staged code edit; untracked files are also
classified. Tracked assume-unchanged and skip-worktree flags and enabled
sparse checkout also select full verification, because ordinary Git diffs can
hide changed tracked bytes in those states. Renames are inspected as deletion plus addition. CI obtains its
comparison from the GitHub PR or push event, fetches history and completes the
same required `verify` job. The execution step reclassifies rather than trusting
a previously reported route. `cargo xtask verify` retains its complete surface
and now includes the new tool regressions.

## Observed results

- Wrapper-only calibration: reproduced surviving descendant; probe cleanup
  explicitly terminated and reaped it.
- Shared lifecycle regression: normal and repeated success, failure, application
  restart, SIGTERM and SIGINT all left no recorded descendant in `/proc`; the
  unrelated sentinel remained alive.
- Initial focused Python suite: 11 tests passed, including mixed and hidden staged code,
  dirty/untracked files, rename/deletion boundaries, symlink/executable modes,
  unresolved index, missing/invalid base, and PR/push/malformed event payloads.
- Both standalone native smokes passed using the existing release binaries and
  private bwrap/sysroot X11 sessions. The application exercised persistence
  restart; the gallery verified 20 stories with empty text and semantic audits.
- `cargo xtask verify` passed: plan/token checks, formatting, native and WASM
  clippy, workspace tests, architecture, release native and WASM builds, both
  browser smokes, all five deterministic UI fixtures and both native smokes.
  Its early tool stage ran the original nine tests; the two subsequent guard
  regressions and final guard hardening passed in the focused 11-test run.
- After canonical completion, process inspection found no worktree-owned
  supervisor, sandbox, Xvfb or native application remaining.

Independent review of candidate `4c94b7d9e481e4b931ba80e109f56387730fab82` reproduced
an additional boundary defect: an assume-unchanged flag hid modified source
bytes from the Git diff while a README delta selected the docs route. Retain
the repair that rejects flag presence itself, including on prose, and enabled
sparse checkout. The repaired focused suite passed all 14 tests, including both
flags with unchanged and modified bytes, normal clean/prose scope and sparse
checkout. A disposable repository exercised the production CLI: normal README
change selected `docs`, each flagged hidden source change selected `full`, and
clearing the flags/restoring source returned to `docs`. This classifier-only
repair did not require repeating native runtime qualification.

The final implementation source bundle is identified by SHA-256
`3daffa0dbbfc57e4d2c108a13ac617125f45c4769c10997df4870b85a71c3f4a`.
This hashes the ordered `sha256  path\n` manifest for `.github/workflows/verify.yml`,
`tools/native-smoke.sh`, `tools/gallery-native-smoke.sh`,
`tools/native-smoke-lifecycle.sh`, `tools/owned-process.py`, `tools/verify.py`,
`tools/tests/test_owned_process.py`, `tools/tests/test_verify.py` and
`xtask/src/main.rs`. The pull request retains the final committed identity and
its representative proof; this calibration does not assert a merge.

## Reproduction and limits

Local logs and the source manifest are registered scratch under
`.tools/runtime/verification-friction/`: `baseline-lifecycle-probe.log`,
`focused-tests.log`, `native-smoke.log`, `gallery-smoke.log`,
`cargo-xtask-verify.log`, `owned-process-audit.log`, `review-repair-tests.log`,
`review-repair-guard-proof.log` and `source-manifest.sha256`.
Canonical runtime artefacts are under `.tools/runtime/verification-evidence/`.
These generated artefacts are ignored; this report retains their material
observations.

The local environment reused the primary checkout's `target`, `.tools/sysroot`
and `.tools/packages` caches through symlinks. It used the default private
sysroot/bwrap UI mode, with `TMPDIR` set to the worktree's
`.tools/runtime/verification-friction` for canonical verification. Standalone
smokes set `POLYORAMA_EVIDENCE_DIR` to that directory's `native` or `gallery`
subdirectory. Shared cache contents were not deleted. The supervisor is Linux
specific, matching the native smoke entry points. Scoped verification proves
prose scope and local lifecycle consistency; it cannot replace outstanding
product acceptance, remote review or landing evidence.
