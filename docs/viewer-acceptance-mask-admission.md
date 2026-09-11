# Scoped mask and compressed-cache admission

This local candidate implements one admission/lifetime repair and explicit worker
continuation exhaustion. It is not a new rate, scheduling, representation or cache
size candidate. Main owns review, commit, runtime freeze and integration.

After descriptor admission, `SharedClient::begin_request` derives the selected
mask keys and demanded bin keys/lengths from the authenticated profile and index.
It rejects a combined working set larger than the unchanged compressed-plus-mask
budget with `working-set admission: required compressed bins and masks need N
bytes, limit L`, before mask transfer or continuation. It reserves the full set,
including absent payload, by evicting unrelated masks, representations and bins.
Only the selected masks are protected, not every mask of the active representation.
The complete representation need not fit.

During the exclusive scope, received bins must belong to the demanded set and
stay within its authenticated lengths; final lengths must match. Full byte and
bin-count reservation prevents the underlying cache's LRU from evicting a required
dependency. Duplicate and out-of-order fragments still use the existing owner's
overlap, final-length and range validation. Mask digest/length checks, exact TID
checks, missing-mask errors and all/any validity semantics are unchanged.

## Lifetime and resources

There is one active source scope per client. Its token cannot be overwritten by
a second admission or released by an old token. Response readers capture the
scope; stale readers fail before admitting bytes. Both executors admit after
descriptors and release before Completed, Failed or Cancelled publication. Browser
`finally` also performs idempotent cleanup. Native exhaustion now returns an error
without a last decode attempt. Browser exhaustion produces a matching explicit
`regional continuation exhausted after 64 rounds` Failed, before accessing pixels.
The original 64-round budget and 63-continuation accounting remain.

Release frees the selected-key metadata and drops incomplete demanded bins,
including holes not advertised by the cache model. Complete bins and masks remain
ordinary evictable cache entries. Cancellation after decoding discards output
before transfer; UI-owned request reservations, stale-generation rejection and
uploads retain their existing ownership. No queue, worker, thread, prefetch,
priority, token or outstanding-output limit changes.

Payload reservation is a subset of the existing shared budget, not a second cache
allowance. Retained scope metadata uses exact-sized boxed slices bounded by 64
mask/tile keys and the existing 100,000-bin limit, plus a validated TID, region and
scope token. `request_reservation()` exposes payload reservation and live metadata
size separately. Worker metrics expose both current and peak values. Metadata
reports include the live scope structure and owned capacities; allocator overhead,
fixed client storage, admission scratch, transport buffers and the owner's bounded
merge scratch are not compressed payload residency or an RSS claim. Existing
16-range/bin, 32 MiB/bin, descriptor, representation, decode-workspace and output
limits remain in force. Failed admission leaves no scope.

## Compatibility and limits

The generic API is additive: existing `ClientLimits`, `ClientMetrics`, identities,
mask formats and legacy request methods keep their public shape and default
multi-request eviction behaviour. Callers opting into an exclusive scope must end
it on every terminal path; interleaved regions or descriptor mutation are rejected
while it is active. Complete legacy dependencies can be reused; incomplete selected
bins are discarded at the transition into the scope.

A legacy unscoped cache may contain unrelated sparse fragments hidden by the
owner's public cache model. If visible unrelated entries cannot reclaim enough
space, scoped admission returns an explicit bounded sparse-occupancy error rather
than promising protection or starting refetch churn. This is covered by an authored
regression. Viewer-owned scoped requests remove incomplete demanded bins at release,
so their cancellation/error paths do not accumulate that invisible pressure.

## Authored verification

All outputs are beneath
`/nvme/development/emuella/.build-targets/viewer-acceptance/mask-admission-build`.
Offline, locked native and WASM release builds and targeted Clippy are used; these
are local candidate artefacts, not a frozen canonical browser package. The source
manifest and build hashes are retained in `source-manifest.json` and
`build-evidence.json` there. Build/test logs are under `authored/`. The targeted run passed 70 checks
(source 8, viewer 16, runtime 22, compatibility 3, masks 3, delivery 11, worker 7);
the optional GDAL test remained ignored. Formatting and whitespace checks passed.

The deterministic Rust fixture encodes three small authored tiles with validity.
It covers a two-tile masked demand under unrelated mask/bin pressure, both orders
of mask/bin arrival, duplicate and sparse fragments, real eviction and exact
refetch, a representation larger than the budget, unrelated-representation
reclamation, error/cancel/stale-reader cleanup, scope exclusivity and a combined
set one byte over budget. Unchanged validity and downstream compatibility checks
remain part of the targeted run.

`node --test apps/emuella-viewer/web/tests/worker.test.mjs` runs the actual worker
control flow in a Node VM with authored transport/client stubs and a fixed clock.
Its non-ready path asserts exactly one matching Failed, 64 responses, 63
continuations, zero decodes and zero transferred arrays. Other cases cover cache
hits, mask/decode errors, transport cancellation, post-decode cancellation and
stale-generation cancellation. It does not execute WASM decoding or a browser.

No actual application, browser, GPU or protected-scene proof was launched. Canonical
verification, the full historical matrix and new timing/RSS measurements were not
run. The corrected historical result remains 2,212 successful normal jobs and
20/47 pressure failures per phase, including 11 continuation exhaustions per phase.
The historical RSS exceedance remains **278,794,240 > 243,269,632 bytes**; quality
and scheduling rejection are unchanged. These tests establish the authored cache
mechanism and failure contract, not pressure acceptance or an RSS repair.

Main's proposed next proof remains one fixed corrected run of nine contexts,
2,309 jobs and 1,883 mask files, plus five fresh native ten-cycle starts with zero
retries, after a new committed runtime identity and separate grant. None of those
runs is performed by this work package. Real combined working-set sizes and full
pressure completion remain to be observed there.
