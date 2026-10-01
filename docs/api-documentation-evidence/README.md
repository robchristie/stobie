# Application composition observations

The [qualification report](../api-documentation-evidence.md) owns the requirement
mapping, exact source and exercised environment. This directory retains actual
bounded evidence rather than generated rustdoc HTML.

Extract `observations.tar.gz` into a scratch inspection directory. Verify the
archive SHA-256 and each uncompressed file against `archive-index.json`.
The archive contains complete command output, negative-probe receipts, native
baseline/variation snapshots and opened screenshots, rendered API text/navigation
observations and this host's temporary execution/inspection recipes. The
independent reader report and original/variation observations are in `fresh-reader/`.
The recipes describe the observed environment; use the public composition guide's
maintained commands for ordinary development.

The owning PR retains final exact-head review, independent reader acceptance,
required CI, actual squash revision, post-merge CI and cleanup. Build receipts
keep their original source identities through evidence-only closeout commits.
