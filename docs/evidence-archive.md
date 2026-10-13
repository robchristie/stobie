# Evidence archive

Raw evidence such as screenshots, logs, runtime snapshots and JSON
observations does not belong in tracked source. Git keeps every binary revision
in full, and repeated captures had grown the repository history to about
97 MiB, almost all of it evidence, until the rewrite described under
[History](#history). Reports keep the conclusions; the raw
captures live in the append-only
[stobie-evidence](https://github.com/robchristie/stobie-evidence) repository.

## What stays tracked

- Markdown reports and summaries, including those under `docs/*-evidence/`.
- Small qualification protocols, build receipts and result summaries that
  tools read or write at fixed paths or reports cite, such as
  `docs/viewer-acceptance-*.json`.
- Reviewed binary sources: UI baselines in `tests/ui-snapshots/expected/`,
  documentation images in `docs/images/` and bundled fonts.

`tools/check-tracked-files.py` enforces these rules in the `checks`
verification stage. It rejects:

- anything other than Markdown under `docs/*-evidence/`
- `docs/*-evidence.json` captures
- binaries outside the reviewed roots
- binaries larger than 512 KiB
- text files larger than 1 MiB

## Where new evidence goes

Smoke tests and `cargo xtask verify` write to ignored `.tools/runtime/`
directories. CI uploads each stage's `.tools/runtime/verification-evidence/`
for 14 days.

When a report needs durable raw evidence:

1. Add the files to stobie-evidence in a new commit, under the path the report
   would otherwise have used.
2. Append their SHA-256 lines to that repository's `evidence-archive.sha256`.
3. Link to them from the report with permalinks to that commit.

Never rewrite stobie-evidence history, because those permalinks depend on it.
Its rulesets block force-pushing or deleting `main` and moving or deleting
`archive-*` tags.

## Archived evidence

On 2026-10-13, 652 raw files were removed from this tree: every non-Markdown
file under `docs/*-evidence/` and three `docs/real-scene-viewing-*evidence.json`
captures. Each was copied byte for byte from source commit
`03ad5c5c3a125e453ec96554a078dda9d6ad4f64` (an original-history ID; see
[History](#history)) to stobie-evidence commit
[`0d0d782`](https://github.com/robchristie/stobie-evidence/tree/0d0d782fee9970d6740bc409fa3a350cf2fe9e00).
[`evidence-archive.sha256`](evidence-archive.sha256) lists each file's
SHA-256 and is byte-identical to the archive's manifest. Report links point to
that commit.

Other files moved within this repository at the same time:

- The synthetic viewer input catalogue moved to
  `apps/emuella-viewer/qualification/synthetic-input-catalogue.json`, beside its
  preparation thresholds.
- The UI baselines moved from `docs/ui-snapshots/` to `tests/ui-snapshots/`.
- The README image became a lossless WebP in `docs/images/`.

Archive base: `https://github.com/robchristie/stobie-evidence/tree/0d0d782fee9970d6740bc409fa3a350cf2fe9e00/docs/`.
Append a path from the first column to browse that directory.

| Archived path | Files | Interpreting report |
| --- | ---: | --- |
| `api-documentation-evidence/` | 2 | [API documentation evidence](api-documentation-evidence.md) |
| `application-identity-evidence/` | 32 | [Application identity evidence](application-identity-evidence/README.md) |
| `application-interface-evidence/` | 5 | [Application interface evidence](application-interface-evidence.md) |
| `browser-startup-evidence/` | 24 | [Browser startup results](browser-startup-results.md) |
| `design-agent-loop-evidence/` | 28 | [Design-system evidence](design-agent-loop-evidence/README.md) |
| `emuella-viewer-evidence/` | 452 | [Emuella viewer qualification](emuella-viewer-qualification.md) |
| `navigation-browser-focus-evidence/` | 5 | [Navigation focus evidence](navigation-browser-focus-evidence/README.md) |
| `navigation-item-evidence/` | 11 | [Navigation item evidence](navigation-item-evidence.md) |
| `presentation-context-evidence/` | 4 | None. These are captures from [PR 34](https://github.com/robchristie/stobie/pull/34) and [PR 35](https://github.com/robchristie/stobie/pull/35). |
| `real-scene-viewing-*evidence.json` | 3 | [Real-scene viewing](real-scene-viewing.md) |
| `record-desk-evidence/` | 7 | [Record Desk evidence](record-desk-evidence.md) |
| `rust-1-99-0-evidence/` | 13 | [Rust 1.99.0 evidence](rust-1-99-0-evidence/README.md) |
| `rust-toolchain-evidence/` | 7 | [Rust toolchain evidence](rust-toolchain-evidence/README.md) |
| `status-chip-evidence/` | 13 | [Status chip evidence](status-chip-evidence.md) |
| `text-hierarchy-evidence/` | 8 | [Text hierarchy evidence](text-hierarchy-evidence/README.md) |
| `typed-icons-evidence/` | 16 | [Typed icons evidence](typed-icons-evidence/README.md) |
| `vertical-slice-evidence/` | 22 | [Vertical-slice report](vertical-slice-report.md) |

To fetch and check the complete archive:

```sh
git clone https://github.com/robchristie/stobie-evidence.git
cd stobie-evidence && git checkout 0d0d782fee9970d6740bc409fa3a350cf2fe9e00
sha256sum --check --quiet evidence-archive.sha256
```

## History

On 2026-10-13, after the archive was copied, this repository's history was
rewritten to remove the same raw captures from every commit: every
non-Markdown file under `docs/*-evidence/` and every `docs/*-evidence.json`.
The synthetic viewer input catalogue was kept at its old path. On the
rewritten branches, `main` and `codex/generic-application-actions`, authors,
dates, messages, parent structure and all other files are unchanged. Commits
left empty were kept, so every merged pull request still has its commit. The
tree of `main` is unchanged, and fresh clones fell from about 97 MiB to about
7 MiB.

Two things did change:

- **Commit IDs.** Every commit on those branches except the root, `033c01b`,
  has a new ID. IDs cited before the rewrite, in reports, plans, receipts and
  pull requests, refer to the original history. Commits that exist only on
  pull-request refs were not rewritten on GitHub and keep their original IDs
  there.
- **Signatures.** 60 of the 63 original commits carried GitHub's web-flow
  signature. A signature cannot survive a rewrite, so the rewritten commits
  are unsigned and GitHub no longer shows them as verified.

The
[`archive-2026-10-13` release](https://github.com/robchristie/stobie-evidence/releases/tag/archive-2026-10-13)
of stobie-evidence preserves the original history:

- `stobie-original-history-2026-10-13.bundle` is a complete bundle of every
  branch and pull-request ref as it stood on GitHub, with `main` at
  `7dae8fcdf92e8ded2234514d7c714295cb73a263`.
- `stobie-original-history-2026-10-13.bundle.sha256` holds the bundle's
  SHA-256:
  `2b98530ad0a5e5b625c12032374da64ff9dba49131c00dc0bb3e817225b119f7`.
- `stobie-commit-map-2026-10-13.txt` (SHA-256
  `606fdd682709dc75baf12892a638f9959436fb6121cdd0ff437f40a9c7f52e0a`) maps
  the original ID of each of the 63 commits on the rewritten branches to its
  new ID. For example, `03ad5c5` became `14139f7` and `7dae8fc` became
  `adbb281`.

Download the bundle and its `.sha256` file, then restore every original branch
and pull-request ref with:

```sh
sha256sum --check stobie-original-history-2026-10-13.bundle.sha256
git clone --mirror stobie-original-history-2026-10-13.bundle stobie-original.git
```

A plain or `--bare` clone of the bundle restores only its branches. To resolve
original commit IDs inside an existing clone, for example for qualification
tools that look up frozen commit IDs, fetch the bundle under a separate
namespace instead:

```sh
git fetch stobie-original-history-2026-10-13.bundle 'refs/*:refs/stobie-original/*'
```

Never push those refs: they bring every archived blob back. In particular,
do not run `git push --mirror` from a clone that has them.

Pull-request pages on GitHub still show their original commits. Clones made
before the rewrite must be re-cloned, or their branches reset to the rewritten
remote branches; working trees at `main` do not change. Rebase local branches
built on the old history onto the rewritten commits, using the commit map to
find each base.
