# Evidence archive

Raw evidence (screenshots, logs, runtime snapshots and JSON observations) does
not belong in tracked source. Git keeps every binary revision whole, and
repeated captures had grown the repository history to about 95 MB, almost all
of it evidence. Tracked documentation keeps the conclusions; raw captures live
outside the tree.

## What stays tracked

- Markdown reports and summaries, including those under `docs/*-evidence/`.
- Small qualification protocols, build receipts and result summaries that tools
  read or reports cite, such as `docs/viewer-acceptance-*.json`.
- Reviewed binary sources: UI baselines in `docs/ui-snapshots/expected/`,
  documentation images in `docs/images/` and bundled fonts.

`tools/check-tracked-files.py` enforces this in the `checks` verification stage.
It rejects anything other than Markdown under `docs/*-evidence/`, binaries
outside the reviewed roots, binaries over 512 KiB and text files over 1 MiB.

## Where new evidence goes

Smoke tests and `cargo xtask verify` write to ignored `.tools/runtime/`
directories. CI uploads each stage's `.tools/runtime/verification-evidence/`
for 14 days. When a report needs durable raw evidence, record the SHA-256 of
each retained file or bundle in the Markdown summary and keep the bytes outside
Git. A durable external store has not been chosen yet; see the history section
below.

## Archived evidence

On 2026-10-13, 650 raw files were removed from the tree: every non-Markdown file
under `docs/*-evidence/` and `docs/real-scene-viewing-evidence.json`. They remain
in history at source commit `03ad5c5c3a125e453ec96554a078dda9d6ad4f64`, and
[`evidence-archive.sha256`](evidence-archive.sha256) lists the SHA-256 of each
one. Report links to archived files are permalinks to that commit. The synthetic
viewer input catalogue moved to
`apps/emuella-viewer/qualification/synthetic-input-catalogue.json`, beside its
preparation thresholds, and the README image became a lossless WebP in
`docs/images/`.

To extract and check archived files from a clone with full history:

```sh
repo=$(pwd)
mkdir -p /tmp/stobie-evidence
git archive 03ad5c5c3a125e453ec96554a078dda9d6ad4f64 docs | tar -x -C /tmp/stobie-evidence
cd /tmp/stobie-evidence && sha256sum --check --quiet "$repo/docs/evidence-archive.sha256"
```

Name individual directories instead of `docs` to extract less, and add
`--ignore-missing` to the check.

## History

Removing files from the tree stops further growth but does not shrink clones:
the archived blobs remain in history. Any history rewrite is a separate,
explicitly authorised change. It must first copy the archive and a complete Git
bundle of the original history to a durable external store, then re-point the
permalinks and this document at that store. Until then, the source commit above
is the archive.
