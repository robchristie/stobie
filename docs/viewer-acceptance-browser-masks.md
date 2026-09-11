# Bounded browser exact-mask diagnostic protocol

Status: **prepared, unmeasured**. The coordinator owns the commit, grant, fresh
execution capsule, architectural review and landing. This package neither
launches a browser during preparation nor changes application, renderer, service,
prepared representations, cache policies or source TIFFs. Starting checkpoint is
`a8705d3c7c3e25a70ccb3e2203f3fbac0c967bc3`; the supplied runtime checkpoint is
`fe82f85984fba5c75d7a5a8f10b2d914b773a838`. The capsule must bind the actual fresh
release build and served assets, rather than infer a generated path from history.

Every outcome remains **quality-rejected-diagnostic-only**. There is no selected
configuration, human/ML claim, speed claim or viewer acceptance. Missing proof
and observed disagreements produce `partial`, with exit code 2; they do not
constitute terminal product rejection. A complete result proves only this
protocol's diagnostic assertions.

The bounded preparation question is whether the unchanged actual browser WASM
worker and HTTP mask path can be checked against retained native request, sample
and mask identities without accumulating decoded arrays. The representative
preparation probe is authored parser/comparison/HTTP-fault testing without a
browser. Polyorama owns this protocol and later protected comparison evidence;
the coordinator owns acceptance selection. Exit preparation with tested authored
code and the fixed budget below. Browser execution requires a later committed
candidate and its exact grant.

## Frozen invocation budget

One invocation, nine serial persistent browser contexts, one actual module Worker
per context, zero warmups and zero runner retries. Each context uses a fresh
profile and closes before the next. There is no retry-until-pass loop or adaptive
pressure expansion. A failed context remains in the result; separately planned
contexts still run if the outer budget permits.

| Context | Worker jobs | Catalogue mask HTTP comparisons |
|---|---:|---:|
| Mansfield PAN16 | 252 | 420 |
| Mansfield RGB16 | 504 | 42 |
| Boca PAN16 | 280 | 770 |
| Boca RGB16 | 672 | 63 |
| Tok RGB8 | 504 | 588 |
| Required missing mask | 1 | 0 |
| Required corrupt mask | 1 | 0 |
| Required stale mask identity | 1 | 0 |
| Mask pressure and revisit | 47 forward + 47 revisit | 0 |
| Total | **2,309** | **1,883** |

The pressure list is every distinct retained native-resolution, all-component
window in cohort/request order: 9/9/10/10/9 windows. The identical list is then
revisited in the same context. The hard forward-list bound is 64; the exact
47-item list and all other job indices are written to `plan.json` before launch
and bound by the grant's plan digest. Native repeated selections remain in the
2,212 agreement jobs. No new decoding region is invented.

Only one job is outstanding. Each job has a 60-second deadline; each context has
a 660-second deadline, including its mask pass. The invocation checks a
6,000-second outer budget before each context; nine context deadlines total
5,940 seconds. A separate 6,100-second process timeout in the execution command
bounds preflight and cleanup too. A process timeout leaves retained partial
files; the execution owner records the missing terminal receipt without retry.

HTTP allows at most two simultaneous requests, 128 requests per job, and
`catalogue_masks + jobs * 128 + 64` requests per context. Catalogue-pass counters
reset per mask; init/static requests use the additional allowance. Each ordinary
response is bounded to 8 MiB, the catalogue to 16 MiB, and context non-static
HTTP bodies to 2 GiB. The unchanged production worker still has its bounded
64-round JPP continuation and three descriptor-admission rounds. Its `retries`
counter is retained; those existing transport rounds are distinct from an
additional runner invocation or retrying a failed job.

Normal worker compressed residency remains 64 MiB. The pressure context uses
exactly the inherited **1 MiB** compressed limit, including masks. The unchanged
16 MiB descriptor and 64 MiB codec-workspace limits are checked from actual
worker metrics. One bounded sample/validity output-pair reservation is at
most 4 MiB, within the inherited pressure decoded ceiling. No GPU resources are
created by this harness; the inherited 16 MiB pressure GPU limit is not exercised
or reported as passed. No app/runtime queue or budget setting is modified.
Reservation sizing preserves the Engine's conservative unaligned leading-cell
allowance; actual output comparisons use the globally anchored ceil-start grid.

## Comparison and failure semantics

Each full-scene context first fetches every tile/level mask through the local
HTTP proxy to the unchanged service. The proxy compares each actual returned
body byte-for-byte with the pinned canonical sidecar and validates length,
component-major LSB-first packing, zero padding and all-implies-any. Unpacked
all/any plane digests are compared with the retained independent GDAL oracle in
`masks.json`; component partial-cell counts are retained. This transports all
269 tiles at all seven levels, including clipped edges. The native evidence
remains the original-source validity oracle; no lossy reconstruction value
establishes validity, and no general alpha or new JPEG syntax is introduced.

Catalogue delivery alone does not prove Worker cache admission. The context then
sends every retained request to the actual pinned `worker.js` and its WASM
`WorkerClient`, preserving the real manifest and representation digest. For each
Completed envelope, the page checks request/token identity, typed array shape,
precision, layout and output-pair bound. It obtains just that region's pinned
native U16LE samples and binary validity through local read-only reference
routes, then compares **every returned sample and validity value directly**.
It never trusts an emitted FNV or validity checksum as the comparison. Samples
at invalid cells are checked too. Sample disagreement and false-valid/
false-invalid counts remain separate; either survives in the result.

The Completed envelope and browser reference arrays lose their retained
references in `finally` before the next job. Only aggregate comparisons and
bounded metrics cross back into the Node report. This bounds retained live
arrays; it does not claim immediate JavaScript garbage collection or RSS
acceptance. Native input files are read, length-checked and hashed one region at
a time, before execution and again on use and final recheck. Source TIFFs are
not opened by this runner.

Coverage reports distinguish native/reduced levels, every component selection,
cross-tile windows, clipped scene edges and actual single-component source
transition windows containing both valid and invalid cells. Boca requires
observed reduced partial cells and native transitions. Mansfield/Tok's retained
oracles contain no transitions; the runner records that applicability rather
than fabricating invalid samples.

Three fresh contexts select the first retained Boca RGB16 request. Exactly the
first required mask response is intercepted per context:

- Missing: return local HTTP 404, without writing or removing a service file.
- Corrupt: copy the bounded successful HTTP body and flip one bit, retaining
  length; the original sidecar and service response remain unchanged.
- Stale: forward that mask request to the actual service with a zero `tid`;
  require the service's HTTP 400 `stale mask identity` rejection.

Each requires the corresponding actual Worker Failed envelope, the original
request identity, zero decoded publications/decode count, and a bounded quiet
check for unexpected terminal messages. An unrelated error or timeout is partial
proof. There is no restoration/retry journey hidden in these cases.

Pressure requires positive **mask** eviction counters, actual HTTP refetch of at
least one previously loaded identical mask key, exact native sample/validity
agreement on every revisit, and observed cache bounds. Representation eviction
alone is insufficient. Failure to achieve actual mask eviction with this frozen
list is partial; do not reduce the 1 MiB ceiling or add windows to manufacture a
pass.

Page job-plus-comparison and worker execution durations are separately computed
within their own browser realms. No page/worker stamp subtraction is performed;
negative worker intervals remain observations. These intervals include harness
work and are not normal application latency or speed evidence.

## Input capsule and grant schema

`tools/viewer-acceptance-browser-masks.mjs` accepts `validate --capsule FILE` or
`run --capsule FILE --grant FILE --protocol-commit COMMIT --output-name NAME`.
Importing it never launches a browser. `validate` reads/hashes pinned local
inputs only; it makes no HTTP request, imports no Playwright and launches no
browser. It prints the capsule/plan SHA-256 and exact invocation counts for the
coordinator to bind in the grant. Validation is not permission to execute.

The coordinator supplies JSON schema `viewer-acceptance-browser-masks-input/1`:

| Field | Required value |
|---|---|
| `schema` | `viewer-acceptance-browser-masks-input/1` |
| `disposition` | `quality-rejected-diagnostic-only` |
| `runtime_commit` | Full actual committed runtime revision |
| `build_record` | Pinned fresh release build receipt identity |
| `static_root` | Absolute exact prepared static root |
| `static_assets` | At most 32 `{path, bytes, sha256}` entries; `path` relative to `static_root` |
| `chromium` | Exact local Chromium executable identity |
| `playwright_version` | `1.62.1` |
| `service` | `{url, catalogue_sha256, identity}`; URL must be a loopback HTTP origin |
| `native_evidence` | Pinned `docs/viewer-acceptance-full-scenes-results.json` identity |
| `representations` | Five `{asset, root}` entries in the order below |
| `temporary_alias` | Optional `private-linux-tmp/1` binding defined in [the environment repair protocol](viewer-acceptance-browser-environment.md); absent preserves the original paths |

An identity is `{ "path": "/absolute/path", "bytes": INTEGER,
"sha256": "64 lowercase hexadecimal characters" }`. Static asset paths are the
only relative identity paths. Required static entries are `worker.js`,
`response.js`, `pkg/emuella_viewer.js` and `pkg/emuella_viewer_bg.wasm`; include any
other actual release-package imports. Unknown paths are never served. The build
receipt and service identity receipt are hashed opaque coordinator records:
the coordinator must bind release source/options, native executable/process,
static root, immutable prepared data and service ownership in them. This runner
checks receipt identities and exact delivered catalogue/manifests; it cannot
infer a service process's build provenance from an HTTP response.

Representation order is:

1. `94_104001000B823500-PAN16`
2. `94_104001000B823500-RGB16`
3. `106_10400100413CDF00-PAN16`
4. `106_10400100413CDF00-RGB16`
5. `105_104001002F92BB00-RGB8`

Each `root` is the exact immutable representation directory whose owner is
already identified in the supplied native aggregate. That aggregate supplies
pinned `native.json`, `requests.json`, `masks.json` and
`regional-agreement.json` paths; the latter pins each raw native reference.
The runner neither guesses historical paths nor assumes a build is ready.
Service catalogue SHA-256 is over the exact HTTP JSON body; its manifests must
also equal all five native manifests. The caller supplies the catalogue pin
from the service owner's receipt, not by discovering an unpinned service during
execution.

The separate coordinator grant uses
`viewer-acceptance-browser-masks-grant/1`, with these exact bindings:

- `schema`, `disposition`, `protocol_commit`, `protocol_files`,
  `capsule_sha256`, `plan_sha256`, `output_name`.
- `protocol_files` maps the runner, this document, the environment repair protocol and the authored test path to
  their committed SHA-256 values. The exported `committedProtocol(COMMIT)`
  verifies local bytes against Git before returning that map.
- `max_invocations: 1`, `max_browser_launches: 9`, `runner_retries: 0`.
- Nonempty `operation_owner`, `attribution` and `lineage` (one to sixteen strings
  naming the approved source-coverage/rights owner and native parent groups).

The exact plan digest is SHA-256 of `JSON.stringify(makePlan(scenes))`, not of
pretty-printed `plan.json`; use `validate`'s returned value. The grant authorises
one exclusively created output group, not a second launch under a new name.
Changing the capsule or grant requires an explicit new coordinator decision.

## Protected storage and later execution

All browser profiles, downloads, temporary files, captures if any, comparisons,
errors and results stay under a fresh attributed/lineaged direct child of
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`.
The runner creates `viewer-acceptance-browser-masks-NAME` exclusively, writes
`lineage.json`, `plan.json` and `inputs.json` before launch, and sets temporary
and XDG paths inside the new group before importing Playwright. Each context
retains its own profile and `result.json`; the group retains append-only bounded
`comparisons.jsonl` and final `result.json`. No screenshots, downloads, traces,
raw transcript or reviewer-event capture are requested. Browser downloads and
service workers are disabled. Raw input samples and masks are never copied into
JSON evidence. No protected output is placed in Git or build scratch, and no
protected originals or failed groups are deleted.

The separately selected private mount alias changes only temporary-path routing:
when explicitly bound in the capsule, Node and Chromium use namespace `/tmp`,
backed by this invocation's approved execution-group `tmp`. Profiles, downloads,
XDG homes and results retain their original approved output paths. The runner
checks pinned device/inode equality and private mount provenance before importing
Playwright and immediately before every context launch. Ordinary host `/tmp`
never satisfies this opt-in contract. `validate` checks the alias schema only;
it does not attest a future mount. Use the single corrected invocation in the
[environment repair protocol](viewer-acceptance-browser-environment.md) for that
candidate; the command below remains the default-path protocol.

The execution owner must place capsule, grant and the enclosing process log in
its separately fresh attributed/lineaged approved execution group. With
`CAPSULE_GROUP`, `PROTOCOL_COMMIT` and `OUTPUT_NAME` set to those **actual pinned
values**, the exact later command is:

```sh
LD_LIBRARY_PATH=/home/rob/.cache/ms-playwright/chromium-1234/chrome-linux64:/nvme/development/polyorama/.tools/sysroot/usr/lib \
  timeout 6100s node tools/viewer-acceptance-browser-masks.mjs run \
  --capsule "$CAPSULE_GROUP/browser-masks-input.json" \
  --grant "$CAPSULE_GROUP/browser-masks-grant.json" \
  --protocol-commit "$PROTOCOL_COMMIT" \
  --output-name "$OUTPUT_NAME" \
  >"$CAPSULE_GROUP/browser-masks-process.log" 2>&1
```

Do not run this command until the coordinator commits and grants the candidate.
No actual browser launch was performed to author this protocol.

Final schema `viewer-acceptance-browser-masks-result/1` contains diagnostic
status, protocol/runtime/capsule/grant identities, operation owner, planned jobs,
context summaries, missing-proof reasons and immutable-input recheck status.
Each context result retains ordered job indices, native reference digests,
separate direct sample/validity agreement counts, bounded worker counters,
request identity result, same-realm intervals, HTTP/fault aggregates and
applicable coverage/pressure predicates. `comparisons.jsonl` additionally retains
per-mask byte differences, padding/implication errors and oracle plane digests.
Failures and disagreements are preserved in their original context; no best-run
selection or rerun erases them.

## Authored verification and separate work

```sh
node --check tools/viewer-acceptance-browser-masks.mjs
node --check tools/tests/viewer-acceptance-browser-masks.test.mjs
node --test tools/tests/viewer-acceptance-browser-masks.test.mjs
```

Authored fixtures are created exclusively in fresh children of the registered
`/nvme/development/emuella/.build-targets/viewer-acceptance/browser-proof`.
Tests exercise real local HTTP fault forwarding with synthetic bytes, direct
comparisons, clipped/reduced mask packing, pin/grant parsing and resource bounds;
they never import Playwright or read protected sources. All 18 authored tests and
both Node syntax checks passed during preparation on 11 September 2026. Canonical application
verification remains with the coordinator: `cargo xtask verify` includes real
browser smoke and is outside this no-browser preparation window.

Actual GPU application inspection, normal native/browser latency, and inherited
seven-event recovery remain separately required later work. The existing
`tools/viewer-composed-browser.mjs`/composed-journey contracts own normal hardware
journeys on development/Boca/Tok; `tools/viewer-browser-recovery.mjs` owns its
existing recovery events. This Worker-only proof does not satisfy those gates
or the native diagnostics in `docs/viewer-native-diagnostics.md`.
