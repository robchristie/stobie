# Stobie project rename

The framework formerly called Polyorama is now **Stobie**. Its repository is
[`robchristie/stobie`](https://github.com/robchristie/stobie). This rename does
not change the separate project that previously used the Stobie name.

## Current development names

The framework packages are `stobie-core`, `stobie-runtime`,
`stobie-render-wgpu` and `stobie-ui-egui`, under matching `crates/` directories.
Rust imports use `stobie_core`, `stobie_runtime`, `stobie_render_wgpu` and
`stobie_ui_egui`. The Gallery package and directory are `stobie-gallery` and
`apps/stobie-gallery`; the browser worker package is `stobie-tile-worker`.
Update experimental consumers to these names and rebuild generated browser
packages with `cargo xtask build-web`.

Project-specific environment variables use the `STOBIE_` prefix. Browser
automation/startup names and semantic IDs likewise use Stobie in their former
namespace positions. The preview manifest and adapter identify the Lab as
`stobie-lab`. This is a deliberate namespace change; old package, environment
and automation aliases are not supplied.

## Existing saved state

Analytical Workspace Lab writes `stobie.vertical-slice.v2`, reading
`polyorama.vertical-slice.v2` only when the new key is absent. Its application
name and native application container remain Analytical Workspace Lab.

Record Desk writes `$XDG_STATE_HOME/stobie-record-desk/state.json`, falling back
to `$HOME/.local/state/stobie-record-desk/state.json`. If that file is absent,
it reads the former `polyorama-record-desk/state.json` in the same state root.
An explicit `RECORD_DESK_STATE_PATH` disables this fallback. In the browser it
writes `stobie.record-desk.v1`, reading `polyorama.record-desk.v1` only when the
new key is absent. Save writes the validated state to the new location and
leaves the former copy intact.

Current state takes precedence. Invalid current state never silently restores
an older legacy copy; Record Desk continues to refuse replacing malformed or
unsupported saved state. The data schemas are unchanged. Browser fallback
works on the same origin: a new preview hostname cannot read another origin's
local storage.

## Historical evidence

Archived observations, traces, logs, source snapshots, build and production
manifests, hash inventories and recorded host paths keep their original bytes,
including the old project name. They describe the source and environment that
were actually measured. Their recorded hashes and commit IDs are not evidence
for newly built Stobie binaries.

Maintained documentation and navigation use the current name. The live
`docs/ui-snapshots/expected/` regression baselines follow the current semantic
identifiers and reviewed branding, through the existing snapshot process.
Remaining Polyorama references therefore identify either historical evidence
or explicit saved-state compatibility, rather than active product branding.
