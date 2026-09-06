# Plan lifecycle

Every `docs/*-plan.md` declares exactly one `Status: active` or
`Status: complete`. An active plan has one non-empty `Next action:` field and
keeps current phase and increment evidence concise. A completed plan has a
`Delivery:` field linking to its owning Polyorama pull request and a short
closeout where useful. An optional `Landed commit:` field contains a full
commit ID when already known. Keep historical acceptance criteria and
observations intact.

Before final review, record a self-stable completed product state: reconcile
the status, delivery table and closeout against the verified outcome, remove
obsolete `Current phase` and `Next action` sections or fields and unfinished
checkboxes. All delivery-table Status cells must be `Landed`, `Complete` or
`Complete for <explicit bounded result>`. Completion does not assert that the
current pull request has merged: retain its review, CI, eventual squash identity
and cleanup evidence in the landing comment rather than requiring another
closeout commit. Record later product work separately. A bounded result cannot
waive missing acceptance proof; it must match the agreed scope and retained
evidence.

`cargo xtask plans` checks those local lifecycle fields and completed-plan
contradictions without network access. Its regression tests and the check run
through `cargo xtask verify`. It does not establish that a commit landed,
verify remote review or CI, or infer stale active status from GitHub; the
conductor still reconciles those facts when closing the work package.

For a prose-only closeout, run `python3 tools/verify.py --base <comparison-commit>`.
The narrow allowlist covers `README.md`, this lifecycle guide and top-level
`docs/*-plan.md` files. The guard checks both sides of renames and deletions and
all committed, staged, unstaged and untracked changes. Any other path (including
AGENTS instructions, tooling, workflows, dependencies, qualification reports,
UI contracts, snapshots and evidence), unsupported file mode, unresolved index,
tracked assume-unchanged/skip-worktree flags, sparse checkout or missing comparison
commit selects the complete `cargo xtask verify` surface.
Ignored build/cache files do not affect classification. Do not use the scoped
route as a substitute for outstanding product acceptance proof.

The scoped route checks whitespace, classifier regressions, plan lifecycle
regressions and the current plans. Pull-request CI compares the checked-out
merge result with the merge base of the event's base commit; push CI uses the
event's before commit. Both routes finish the same required `verify` job;
workflow path filters must not skip it.
