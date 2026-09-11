# Complete-scene diagnostic exact-mask protocol

This is preparation for **quality-rejected diagnostic assets**, not viewer
acceptance, selected-configuration validation or a new quality calibration.
The workspace viewer-acceptance plan at `56591ff55471b15951f0b3f83441c3d0de98f712` declares these five complete
original scenes. Mansfield RGB16 4/8/12 and Tok RGB8 4 failed their frozen
screens; Mansfield PAN16 4 passed its screen only. Those outcomes remain intact.
Boca is a declared diagnostic here, with no fabricated development-selection
receipt and no call to the quality runner's selected-configuration entry point.

| Original acquisition | Product | Original TIFF bands | Total bpp |
|---|---|---|---|
| Mansfield 94_104001000B823500 | PAN16 | 1 | 4 |
| Mansfield 94_104001000B823500 | RGB16 | 5,3,2 | 12 |
| Boca 106_10400100413CDF00 | PAN16 | 1 | 4 |
| Boca 106_10400100413CDF00 | RGB16 | 5,3,2 | 12 |
| Tok 105_104001002F92BB00 | RGB8 | 1,2,3 | 4 |

One preparation per representation, serially, with zero retries. Preserve
indexed HT, tiles512/D6, irreversible 9/7, one layer, existing rate search,
precision and no MCT. No crop, selected rate, retuning or new comparison.
Direct preparation reads the original TIFF for both samples and validity using
`--mask-input ORIGINAL --mask-bands SELECTED --retain-incomplete true`.
No source/MSI/analytical array or original stream is rewritten. Failed outputs
are retained; no protected cleanup is performed.

The reviewed 38-object source-coverage receipt remains required and is bound to
the coordinator grant. Each selected original source's complete SHA-256 and byte
length are checked before preparation and after success or failure. Exact source,
band order, dimensions, precision, encoding contract and mask catalogue are
checked against the manifest. Required identity failures stop execution.

## Frozen build and admission

The native tool comes from a clean `git archive` of
`4a1594b0840eadae26a4d6005d50d9201734a1a7`, under the registered
`/nvme/development/emuella/.build-targets/viewer-acceptance/full-scenes-build`.
It uses an offline, locked release build of `emuella-viewer-tools` and linked
codec `6586e3d50f95429b242cb2e3535742b002784f2d`. Mutable application worker
edits are outside this build. The archived source tree is checked against the
archive; locally installed dependency files are hashed, without consulting
external codec text or acquiring source. Build inputs, both Cargo locks, helper
source, native binaries, GDAL, loaded Python/native dependencies and build logs
are bound in [the build record](viewer-acceptance-full-scenes-build.json).

The authored native adapter is embedded in
`tools/viewer-acceptance-full-scenes.py` and compiled outside the archive against
its public APIs. It validates every sidecar with the native validity validator,
then compares bounded cached regional decoding and masks through the in-process
service/client against complete-selected-tile native reference reconstruction.
It changes no production code. Native references, cached decoding and preparation
use the same codec: this is not independent decoder evidence.

Before measurement, the coordinator commits the runner, authored tests, this
protocol, build record and unchanged frozen views. `run` compares their local
bytes against the supplied full protocol commit. Imported quality/source functions
come from the pinned archive, with their hashes bound by the build record.
No protected measurement is authorised by `build-record`, tests or this document.

Each invocation needs a separate coordinator-authored grant with schema
`viewer-acceptance-full-scenes-grant/1` and these exact fields:

- `disposition`: `quality-rejected-diagnostic-only`.
- `protocol_commit`: the full commit containing the five protocol-bearing files.
- `protocol_files`: repository-relative paths to SHA-256 values, as returned by
  the runner's read-only `committed_inputs(protocol_commit)` function.
- `build_sha256`: SHA-256 of the committed full-scenes build JSON.
- `asset` and `output_name`: the exact cohort identity and fresh output name.
- `source_coverage_sha256`: SHA-256 of the reviewed source-coverage receipt.
- `max_preparations`: 1; `operation_owner`: the coordinator-assigned single
  process owner/watcher. Grant reuse or a fresh output name does not authorise a
  second preparation of an already attempted representation. The runner rejects
  an existing diagnostic protocol for that asset anywhere in the direct campaign
  groups; the coordinator maintains the serial five-representation ledger.

The coordinator owns commits, measurement scheduling, the single watcher and
canonical application verification. This work package performs no commits,
pushes, downstream delegation or protected measurements.

Preparation verification on 11 September 2026 passed all ten authored tests
(including the explicit native/GDAL journey) and all seven existing quality
regressions. The release tool and authored adapter builds passed. All 797 recorded
build-input identities and 96 linked/loaded library identities were rechecked,
along with the exact archive and embedded adapter source. Canonical application
verification remains with the coordinator; no complete-scene measurement has run.

## Exact masks, regional evidence and quality

All source tiles and all seven levels are inspected, including clipped original
scene edges. Original `GDALGetMaskBand()!=0` is the only mask oracle; decoded
values, including lossy zero, never determine validity. Independent bounded
Python AND/OR reductions compare each component's native/all/any planes with
canonical sidecars. Records include actual false-valid/false-invalid counts,
sample counts, source-valid counts and expected/actual unpacked-mask hashes.
Manifest SHA-256, exact lengths, zero padding and all-implies-any are checked.
The native validator independently checks the complete sidecar catalogue.

Regional probes include frozen views, tile seams, all scene corners and the
first observed original valid/invalid transition per band, found with a halo
across tile seams. Each window uses every discard 0–6, every single component
and all components, twice. Repeated selections are identical legal sorted
requests; duplicate component indices remain invalid under the native contract.
Requests use the globally anchored ceil(start/scale) to ceil(end/scale) grid.
Original GDAL footprints independently verify native combined masks; cached
regional samples must equal the complete-tile reference, and single-band samples
must equal their corresponding multiband planes. Thus valid-side samples next to
source boundaries remain checked. The output retains requests, masks and native
sample hashes for subsequent browser app comparison. Browser proof remains pending.

Before using unchanged quality functions, an exhaustive tile pass proves their
original nodata populations exactly equal GDAL masks per selected band. A
discrepancy stops quality execution; it cannot silently change the population.
The archived `quality.evaluate` scores unchanged frozen source views, source
stretches and scales 1/4. The archived `full_coverage` scores every original tile
and clipped edge, using its existing ANY/ALL/PARTIAL populations and histograms.
Reduction averages all contributors identically, including invalid contributors
in partial cells, and retains every cell with any original valid contributor.
Combined display masks never remove a quality sample or partial cell.
RMSE ≤3 and p99 absolute ≤12 U8 predicates remain unchanged. Failed numerical
predicates are recorded as diagnostic results; even all passes cannot change
the diagnostic disposition or screen rejection.

## Storage and evidence boundaries

Every protected mask, payload, reference, capture, log, protocol receipt and
result stays in fresh attributed/lineaged `viewer-acceptance-*` direct children
of `/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`.
The main group contains `representation/`, `protocol.json`, `prepare-command.json`,
preparation/native logs, `masks.json`, `requests.json`, `native.json`, regional raw
files, `regional-agreement.json`, complete-coverage raw files,
`full-coverage.json`, `source-after.json` and `result.json` or `failure.json`.
The sibling group `<output-name>-views` contains the unchanged quality function's
raw views, display derivatives, references and `quality.json`. No protected
outputs go to Git or build scratch. Authored tests use synthetic data only.

Persistent accounting lists exact hashed payload, descriptor, manifest and mask
files, plus separate ancillary preparation metadata bytes. It preserves the
existing payload bound (tile budgets plus actual shared framing), 1 MiB per
descriptor and 1 MiB manifest bounds, and adds the frozen mask allowance of one
byte per original pixel per band plus 4096 bytes per tile/level. Actual totals,
individual predicates and total eligibility are recorded separately.

Direct-file accounting sums instrumented reference payload reads and descriptor
reads for frozen views, complete coverage and regional probes, plus explicit
native mask validation/regional read totals. Native in-process service/client
metrics remain separately retained in `native.json`; they are not browser or
network measurements. Integrity hashing, Python/GDAL reads and OS cache effects
are outside these logical counters. Application-delivered bytes remain null.
Preparation metrics are retained as diagnostics, with no benchmark latency,
production performance, RSS acceptance or analytical/ML claim.

## Checks and later execution

Run authored tests, including the explicit native/GDAL adapter journey:

```sh
LD_LIBRARY_PATH=/home/rob/pixi/.pixi/envs/default/lib \
GDAL_DRIVER_PATH=disable OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
EMUELLA_FULL_SCENES_AUTHORED=1 \
python3 -m unittest discover -s tools/tests -p test_viewer_acceptance_full_scenes.py
python3 -m unittest discover -s tools -p test_viewer_acceptance_quality.py
```

The authored journey uses per-band nodata=191, valid zero, odd scene edges,
source transitions, all levels and repeated native regional selections. It also
checks complete-coverage ANY/PARTIAL sample counts against original GDAL masks. Unit
tests separately cover very small odd clipped footprints, canonical packing,
actual error counts, global unaligned reductions, source/hash/length changes,
uncommitted protocol bytes and rejected grants. No protected source is read by
these tests. The first 519×517 native authored fixture hit the pinned codec's
resource admission check; the representative native fixture is 643×645, while
small-edge mask semantics remain covered independently. This is fixture
calibration, not an omitted protected measurement.

After the coordinator commits the protocol and grants the window, one example
invocation is below. Replace the uppercase placeholders with that actual commit,
grant and unique output name; repeat only for separately granted cohort entries.

```sh
LD_LIBRARY_PATH=/home/rob/pixi/.pixi/envs/default/lib \
GDAL_DRIVER_PATH=disable OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 \
python3 tools/viewer-acceptance-full-scenes.py run \
  --asset 94_104001000B823500-PAN16 \
  --output-name viewer-acceptance-full-scenes-mansfield-pan16-4-RUN \
  --protocol-commit FULL_COMMITTED_PROTOCOL_REVISION \
  --grant /PATH/TO/COORDINATOR-GRANT.json \
  --source-coverage /nvme/development/emuella/emuella-workspace-viewer-acceptance/docs/evidence/viewer-acceptance/source-coverage.json
```

## Executed diagnostic results — 11 September 2026

All five frozen `run` invocations completed serially under operation owner
`full-scenes-execution-cli`, once per granted representation, with exit code 0
and zero retries. Protocol `4184189763c5fb844fb199b1fb616a2c35b6fa92` used the
archived native source `4a1594b0840eadae26a4d6005d50d9201734a1a7`.
The runner enforced the frozen source, binary and library bindings; all original
source identities matched before and after execution. This section was appended
only after all five invocations completed.

| Representation | Tiles × levels | False-valid / false-invalid | Frozen / full numerical predicates | Persistent bytes | Mask bytes |
|---|---:|---:|---|---:|---:|
| mansfield-pan16-4 | 60 × 7 | 0 / 0 | pass / pass | 9,522,670 | 2,781,284 |
| mansfield-rgb16-12 | 6 × 7 | 0 / 0 | fail / fail | 1,783,964 | 521,826 |
| boca-pan16-4 | 110 × 7 | 0 / 0 | pass / pass | 13,696,218 | 5,851,548 |
| boca-rgb16-12 | 9 × 7 | 0 / 0 | pass / pass | 2,674,688 | 1,097,535 |
| tok-rgb8-4 | 84 × 7 | 0 / 0 | fail / fail | 19,073,334 | 10,528,158 |

Every original tile and clipped edge was covered. All complete-catalogue and
regional mask comparisons matched the original GDAL oracle, and cached regional
samples matched the same-codec complete-tile references. Boca contained invalid
source samples and observed transitions; the selected Mansfield and Tok sources
were entirely valid. All payload, descriptor, manifest, mask and total storage
predicates passed.

[The aggregate result record](viewer-acceptance-full-scenes-results.json) contains
exact result/evidence SHA-256 values, per-band valid/invalid populations, frozen
and full quality metrics, full ANY/ALL/PARTIAL populations, persistent bytes and
logical direct-read accounting. All protected outputs and process logs remain
in the grant-named approved-store groups and their `-views` siblings.

The disposition remains **quality-rejected-diagnostic-only**. Numerical passes
do not revise historical screening or establish selected-configuration, viewer,
human or analytical acceptance. Browser proof and independent decoder evidence
remain absent; application-delivered bytes are null. Logical reads exclude
hashing, Python/GDAL reads and OS cache effects; timing supports no performance
claim. Canonical application verification remains with the coordinator.

Next safe action: use the retained request/sample/mask identities for the
separately owned browser/app comparison and coordinator verification. These
representations are consumed and must not be silently re-prepared.
