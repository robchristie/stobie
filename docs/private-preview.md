# Analytical Workspace Lab private development preview

The root `.dev-preview.toml` identifies this application as `polyorama-lab` and
launches it through an installed gateway's version 1 inherited-socket contract.
The identifier follows that contract's lower-case, maximum-31-character grammar.
Python 3.11 or later is sufficient for the adapter and its verification tests.
The gateway owns the listening allocation,
HTTPS origin and preview lifecycle; `tools/dev-preview.py` adopts the supplied
`DEV_PREVIEW_LISTEN_FD` directly, without closing it and binding another server.
Host configuration and gateway installation are separate operator work.

From the canonical worktree root, build the development browser assets first:

```sh
cargo xtask build-web
dev-preview up --json
dev-preview status --json
dev-preview logs
```

Open the HTTPS `url` returned by `up` in a WebGPU-capable browser. Keep its exact
worktree and run identity with the pilot evidence. `/_dev_preview/ready` reports
the supplied project, canonical worktree, run ID and origin only while all nine
required Lab assets are readable, non-empty regular files; WASM assets must have
the expected WASM header. Startup refuses missing assets and identity mismatch.
Readiness proves the adapter and assets are available; browser rendering and
interaction still need direct qualification.

The adapter serves only the fixed Lab HTML, CSS, bootstrap, startup helper,
worker and generated JS/WASM files from `apps/analytical-workspace-lab/web`.
It rejects symlinks in asset paths, traversal, directory listings and other
files. It publishes no repository root, evidence directory, Gallery, viewer or
data service. Every response uses `Cache-Control: no-store`; WASM responses use
`application/wasm`. Existing relative imports keep the worker and its package
on the same preview origin.

Development uses browser reloads. After a Rust edit, run `cargo xtask build-web`
again and reload. After an HTML, CSS or JS edit, reload directly. The adapter
reads files on each request; launch revision and dirty state reported by the
gateway remain launch-time snapshots. Capture the `run_id` returned by `up` or
`status` as `preview_run_id`. Restart the preview after changing the manifest or
adapter, guarding the stop against a newer generation:

```sh
dev-preview down --run-id "${preview_run_id:?set the observed run_id}" --json
dev-preview up --json
```

If the manifest or worktree is unavailable, also use its recorded `preview_id`
as `preview_id`; these are the launcher's returned identities:

```sh
dev-preview down --preview-id "${preview_id:?set the observed preview_id}" \
  --run-id "${preview_run_id:?set the observed run_id}" --json
```

The Lab has no application API, WebSocket or hot-module-replacement service.
There is no extra server or HMR port. Normal loopback development and the
[production packaging and cache policies](browser-startup.md) keep their
existing behaviour.

## Bounded browser pilot

Use a freshly built, identified worktree and retain the `up`/`status` JSON,
source commit and dirty observation, build command/tool versions and hashes of
the served assets. Browser qualification should record the browser version,
WebGPU backend and tested environment alongside screenshots and observations:

1. Open the exact returned HTTPS origin. Confirm that the workspace renders,
   required JS/WASM responses succeed with the documented MIME/cache headers,
   and the relative tile worker completes work.
2. Pan Primary View and observe Linked Detail; open Results, Thumbnails and
   Diagnostics. Record visible image content and successful worker activity,
   rather than treating a generic HTTP 200 as application readiness.
3. Make a small reversible HTML/CSS change in the Lab web root, reload and
   observe it. Restore the file and reload again. Record both observations and
   preserve unrelated edits.
4. Stop this worktree's preview with the observed `run_id` guard above. Confirm
   the recorded run is inactive and its URL no longer serves the Lab. Retain the
   cleanup result; stopping this preview must leave other runs alone.

The focused adapter regression command is:

```sh
python3 -m unittest discover -s tools/tests -p test_dev_preview.py
```

It launches the real adapter with an inherited descriptor while the simulated
publisher retains its original socket, checks that a competing bind fails
before/during/after the child run, validates readiness identity and assets, and
checks missing assets, invalid descriptors and path/symlink rejection. The
canonical `cargo xtask verify` includes these tests and the existing full
native/browser verification surface. A focused pass does not substitute for
that command or the browser pilot.

## Mac browser qualification (4 October 2026)

The actual Lab was qualified at clean source
`db0abb8e1f618561f7d754ef7355c0630d6190ae` in Chrome **154.0.8037.95** on a Mac,
with WebGPU reporting `apple` / `metal-3`. The private HTTPS origin was trusted
and the page reported a secure context. Same-origin readiness exactly matched
the recorded project, worktree, run ID and origin. All nine browser-fetched
asset hashes matched the build receipt, with the expected MIME types and
`no-store`; the served-asset digest was
`267860a6bc87187fb8a3bf7115c6c1c1897ead149458973c25e87495108bc61c`.

- The 3412 × 1960 canvas rendered actual image content with 20 draw calls and
  successful tile-worker completions. A physical Primary View pan moved both
  linked camera centres from `(65536, 65536)` to `(53248, 59392)`.
- A reversible HTML marker became visible after reload, then disappeared after
  restoring the original file and reloading. The rendered screenshots were
  inspected. Rust and browser assets were otherwise unchanged.
- Physical Results, Thumbnails and Diagnostics tab clicks succeeded. The
  supplemental run recorded 45 worker completions, decoded thumbnail images
  and inspected panel screenshots.
- A matching `up` reused the active run. Generation-guarded `down` reported
  `observed-inactive`; browser readiness returned HTTP 404 afterwards. The
  exact pilot tab was closed, the temporary observation SSH forward was absent,
  and all task previews were stopped. The existing browser profile was preserved
  without reading or copying it; the separately retained manual synthetic
  application was untouched.

The first supplemental navigation received HTTP 404 while the published route
was becoming observable. That failed observation is retained; a bounded reload
retry passed on the same owned generation without an application or source
repair. Launcher readiness therefore remains distinct from browser navigation
and rendering qualification. These results apply to the recorded Mac/Chrome
environment and Lab build; the reload-based development behaviour is unchanged.

Raw evidence remains in the operator's qualification store. Its basenames and
SHA-256 identities are recorded here without private host bindings:

| Evidence | SHA-256 |
| --- | --- |
| `mac-pilot-up.json` | `634d4de6a152f182feeca6a8f1b6202b610d296ec51835d83fccfc3103b592d6` |
| `mac-pilot-journey.json` | `e4f5a9ee0aaf416dea147d144b848978c32beb62799919e0c17d4b62df3e2ac1` |
| `mac-pilot-panels-up.json` | `5f9da026688e4fc5077f2c16ad74d8ccfaaba060da0b49a0a07a5163cfffcc74` |
| `mac-pilot-panels-attempt1.json` | `50bb4502d549d7d17e2f9b71088cd74baa2e1f46da813edb9a8a42eeec0e9f93` |
| `mac-pilot-panels.json` | `dbd22256dfc06b27b2a91e56b4fa7d14e0a06fb9af59484ff93853c33f20e8b9` |
| `mac-target-cleanup.json` | `21f8b8a97dabaf010be82a6ebfef92a6b797f1a3dce379ad25bd2347236d1696` |
| `operational-cleanup.json` | `dba1207a1ffd6e721974d7888578551d29fe8a2fbeb11005b2fec6750855a6ae` |
