# Evidence archive

Raw evidence such as screenshots, logs, runtime snapshots and JSON
observations does not belong in tracked source. Git keeps every binary revision
in full, and repeated captures had grown the repository history to about
95 MB, almost all of it evidence. Reports keep the conclusions; the raw
captures live in the append-only
[stobie-evidence](https://github.com/robchristie/stobie-evidence) repository.

## What stays tracked

- Markdown reports and summaries, including those under `docs/*-evidence/`.
- Small qualification protocols, build receipts and result summaries that
  tools read or write at fixed paths, such as `docs/viewer-acceptance-*.json`.
- Reviewed binary sources: UI baselines in `tests/ui-snapshots/expected/`,
  documentation images in `docs/images/` and bundled fonts.

`tools/check-tracked-files.py` enforces these rules in the `checks`
verification stage. It rejects:

- anything other than Markdown under `docs/*-evidence/`
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

## Archived evidence

On 2026-10-13, 652 raw files were removed from this tree: every non-Markdown
file under `docs/*-evidence/` and three `docs/real-scene-viewing-*evidence.json`
captures. Each was copied byte for byte from source commit
`03ad5c5c3a125e453ec96554a078dda9d6ad4f64` to stobie-evidence commit
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

Removing files from the tree stops further growth, but it doesn't shrink clones:
the archived blobs remain in this repository's history. Any history rewrite is a
separate, explicitly authorised change. Before it runs, store a complete Git
bundle of the original history alongside the archive. The permalinks here
already point at stobie-evidence, so a rewrite does not break them.
