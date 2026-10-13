# Rename the project to Stobie

Status: complete
Delivery: [PR #60](https://github.com/robchristie/stobie/pull/60)

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

## Verified outcome

Active identifiers and paths use Stobie. Saved-state migration, current-state
precedence and malformed-state preservation passed native and same-origin
browser regressions. All eight canonical verification stages passed: checks,
native-lab, native-other, browser, gallery-browser, production, ui and
record-desk. The first five passed through `cargo xtask verify`; the remaining
surface used the same stage commands and assertions with the local UI sysroot
and scoped Xvfb where needed. Production packaging and response-header checks
were retained from the successful build; startup smoke and its one-pair local
benchmark then passed under the working headful configuration. The unsupported
headless attempt's GPU allocation failure remains diagnostic evidence, not a
passing run.

All 14 current snapshots passed at zero tolerance after inspection. Pixel
changes are confined to the Gallery branding row; metadata is unchanged, and
semantic/text changes follow namespace-derived identities and ordering. All
165 matching archived artefacts remain byte-exact. The first source candidate
also passed [CI 38329426420](https://github.com/robchristie/stobie/actions/runs/38329426420).

The renamed private preview reached launcher readiness. This host's browser
received HTTP 403, so private-preview browser qualification was not established.
The local native/browser journeys above establish the product checks. Exact
final-revision review, pull-request CI, merge and post-merge CI belong to the
pull request's delivery record.
