# CI cache ownership and inspection

The required `verify` job still runs the full verification selected by
`tools/verify.py`. A restored cache never skips a build, test or smoke check.
Caches are disposable: a miss or service error leaves Cargo and the pinned tool
installer to do the required work.

`Swatinem/rust-cache@v2` owns Cargo registry and dependency build artifacts under
the ordinary `target` directory. Its recursive cleanup also reaches
`target/browser-cargo`, where the production browser build uses a separate
Cargo target directory. The action retains dependency artifacts and fingerprints
for native and WASM profiles, but removes workspace-crate artifacts, generated
package files and incremental output before saving. Keeping workspace crates would
increase the archive and require a refresh policy for changed source, so it is
not enabled. Cargo checks freshness after every restore.

The action's cleanup can leave empty nested directory skeletons under `target`.
After successful full main verification, CI removes the generated
`target/browser-staging` and `target/browser-production` trees before saving;
otherwise the browser packager would reject a restored, non-empty skeleton as
unrelated output. `target/browser-cargo` remains available for dependency reuse.
Excluding Cargo binaries changes the Rust archive's path-derived cache version,
so the older archive containing those skeletons cannot be restored by this
workflow even when its visible key matches.

Rust cache compatibility includes runner OS and architecture, installed Rust
toolchain versions, selected Cargo/Rust/build environment, `Cargo.lock`, Cargo
manifests and Cargo configuration. An Ubuntu image update can change a
*non-selected* installed toolchain and cause a cache miss even while the pinned
compiler remains 1.97.1; do not bypass the action's compatibility check to
force a hit. Only a successful full `main` push writes the Rust archive. PRs
restore from an accessible `main` cache and do not write a PR-scoped Rust entry.
Documentation-only and failed jobs do not write incomplete Rust entries.

The dedicated `actions/cache` entry owns only
`~/.cargo/bin/wasm-bindgen`, keyed by CLI version, OS and architecture. The Rust
action excludes Cargo binaries. The installer checks the executable's reported
version, reinstalls the pinned CLI if it is absent or wrong, and checks again.
Only a successful full `main` push saves a missing binary entry. `setup-node`
continues to manage npm's package-download cache independently.

To investigate a miss or slow restore, inspect the Rust and wasm-bindgen restore
and save steps in the `Verify` Actions job, then compare the requested key,
restored key, `refs/heads/main` or `refs/pull/*/merge` scope, cache version,
archive size, and save result with the repository's Actions cache listing.
Check the active and other installed Rust versions in the action's
`Environment considered` block. Compare Cargo `Compiling` and `Finished` lines
for the native, ordinary WASM and `target/browser-cargo` phases; an exact cache
hit alone does not show effective reuse. A reservation warning can mean a
competing writer in the same scope, so check whether a usable entry was saved.

## Measurement plan

The baseline is the successful Diagnostics main run 36499413327 and an unchanged
fresh-run rerun of that revision, compared phase by phase. The required cache
change PR and post-merge runs provide controlled cold observations for changed
Rust cache paths. If the merged full run saves a new main entry, rerun that
revision once on a fresh hosted runner to measure warm main-scope restoration.
Use a bounded Rust source-change probe to confirm normal Cargo invalidation.
The PR description and landing comment own run IDs, timings, cache metadata and
the retain-or-reject decision. No wall-clock threshold controls CI success.
