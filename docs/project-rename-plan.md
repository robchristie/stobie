# Rename the project to Stobie

Status: active
Next action: Verify saved-state compatibility and the renamed native/browser packages, inspect current snapshot changes, then complete review and landing.

## Outcome and scope

Use Stobie throughout the active framework, Gallery, in-repository consumers,
build and verification tools, preview adapter and maintained documentation.
The GitHub repository is already `robchristie/stobie`; the fresh workspace maps
to this product checkout. The other project currently called Stobie is outside
this change.

The requested rename deliberately changes the experimental package/import,
environment-variable and browser automation namespaces. It preserves data
schemas, domain behaviour and existing saved-state access. Historical raw
observations retain their original bytes and attribution.

## Acceptance

- Rename all active package, directory, import, asset, token, UI identity,
  environment-variable and automation references consistently.
- Read former saved state only when the current location is absent. Current
  state wins; malformed current state must not select stale former state.
  Explicit Record Desk paths disable default-location fallback. Writes use the
  current location and leave the former copy intact.
- Preserve archived evidence and its hashes. Explain the remaining historical
  references and compatibility literals in the migration guide.
- Inspect generated current snapshots for branding and semantic identity
  changes, with no unrelated layout or behaviour drift.
- Pass `cargo xtask verify`, independent review of the final revision, and
  applicable pull-request and post-merge CI.

## Current phase

Active identifiers and paths are renamed. Saved-state fallback and native and
browser migration regressions are implemented. Verification and snapshot
inspection remain in progress.
