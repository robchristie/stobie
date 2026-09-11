# Viewer acceptance quality calibration

This is a bounded numerical screen of the unchanged indexed encoder, followed
by complete-scene coverage for a selected development rate. It does not establish
visual acceptance, validity-mask delivery, independent reconstruction, application
latency, RSS acceptance, or analytical/ML suitability.

The workspace protocol was frozen at
`0951825b58869871d9d7b6f159864c4944a39b89`. Original source arrays and the exact
[source-coordinate freeze](real-scene-viewing-source-views.json) remain unchanged.
The original viewer source, clean export build and linked codec identities are
in [the build record](viewer-acceptance-quality-build.json).

## Frozen procedure

`tools/viewer-acceptance-quality.py` screens Mansfield RGB16 at 4, 8 and 12 total
bits per spatial pixel, Mansfield PAN16 at 4 and Tok RGB8 at 4. These are the
complete declared combinations. No encoder policy change or comparison is
selected by this script. The four frozen views, 1:1 and arithmetic fourfold
reduction, source-derived stretches and per-band RMSE <=3 / p99 absolute <=12
U8 gates remain fixed. Mansfield and Tok contain no original nodata, as verified
independently across their complete source products.

Before full preparation, a fresh GTiff derivative holds the smallest enclosing
512-grid-aligned rectangle of the four frozen views. Its outer extent either
ends on the original tile grid or retains the exact original scene edge. The
script verifies every selected band sample against original TIFF tiles, and
rejects any changed tile support. Original band numbering is retained. View
coordinates translate by the crop origin; stretch values and observed windows
do not change. Screen derivatives have distinct source SHA-256 and immutable
representation identities and never count as complete scenes. All derivatives,
raw reconstructed samples and display captures stay in fresh attributed
`viewer-acceptance-*` direct children of the approved RarePlanes store.

For the lowest screen-passing, byte-eligible development RGB16 rate, prepare the
complete original scene once and repeat frozen views plus exhaustive display
coverage. PAN16 4 and Tok RGB8 4 follow the same screen/complete-scene distinction.
Full coverage decodes non-overlapping original tiles, holds only bounded tiles,
and aggregates exact U8 error histograms per scale/stretch/band. Fourfold blocks
are anchored at source origin; partial outer blocks average their actual sample
count. Validity comes only from original per-band nodata. A reduced cell with any
valid contributor remains scored; identical averaging includes all contributors,
so valid-side ringing cannot be hidden. All-valid and partially-valid reduced subsets retain separate metrics alongside
the aggregate ANY-valid gate. Entirely invalid cells have no valid
quality predicate. The frozen regional score remains separately reported. This campaign opts into
`viewer-real-scene-quality.py --source-valid-gate`: original per-band ANY-valid
cells gate both scales, with ALL/PARTIAL subsets retained. Historical all-pixel
metrics remain explicitly labelled, and the historical script default is unchanged.
Validity never depends on decoded values, including decoded zeros at valid inputs.
No source stretch, threshold or candidate may be retuned after reserved Boca
validation. A failed required frozen view is sufficient to reject that candidate;
passing a screen is provisional until complete-scene evidence exists.

## Byte and quantiser observations

The payload ceiling is
`sum(floor(actual_tile_width * actual_tile_height * total_bpp / 8) + 128)`
plus the exact emitted shared main-header bytes and two-byte EOC. This follows
codec `ht_indexed.rs`: the per-tile search tests `EncodedLossyTile::len()` against
the floored target plus 128, then emits a shared main header, individual tile
parts and EOC. The search envelope includes its own local header, so this is a
conservative ceiling, not a tight rate prediction. Descriptor ceiling is 1 MiB
per tile; manifest ceiling is 1 MiB. Actual payload, descriptors, manifest and
mask bytes are separately recorded. Quality-only representations currently have
no mask sidecars, and therefore do not claim complete representation eligibility.
Application-delivered bytes are unavailable from direct-file reference export.

The script reads the owner-defined EHTIDX01 descriptor format at codec revision
`6586e3d50f95429b242cb2e3535742b002784f2d`, including its exact LRCP resolution then
component packet ordering. It records final QCD exponents/mantissas and separate
packet-header/body byte counts by component for every tile. It retains no packet
header or entropy bytes in public evidence. Shared codestream framing is
separate from component packets. The final shared-component quantiser is
observable; search-visit counts and rejected quantisers are not exposed.

Each invocation records the protocol-bearing script hashes, source revision,
source identity, frozen-view hash, exact build record and tool/GDAL hashes before
preparation. Tool, GDAL and checkout Cargo.lock identities must match that build
record before any preparation. Original source identity is checked again after reconstruction.
The source cache is uncontrolled; preparation time is descriptive and carries
no cold-cache or speed claim. One coordinator-granted window and one operation
owner/watcher cover serial measurements.

## Verification and reproduction

Run `python3 -m unittest discover -s tools -p test_viewer_acceptance_quality.py`.
These synthetic regressions cover immutable input binding, reserved-rate enforcement,
reduced validity subsets, original partial-tile support, complete edge
averaging, frozen higher-quantile parity and component descriptor attribution.
Canonical application verification and independent review remain delivery gates.

Invoke `python3 tools/viewer-acceptance-quality.py --help` for required paths.
Use the build record's tool/GDAL paths, the checked-in source-view JSON, a local
coordinator timing-grant file and a fresh approved output name. Set
`LD_LIBRARY_PATH` to the GDAL library directory, `GDAL_DRIVER_PATH=disable`,
`OMP_NUM_THREADS=1` and `OPENBLAS_NUM_THREADS=1`. Screen runs use `--phase screen`;
only a declared full-scene candidate uses `--phase full`. The timing-grant file
records scheduling authority, not a waiver of rights or acceptance criteria.

Reserved validation additionally requires `--selection PATH` with schema
`viewer-acceptance-quality-selection/1`, the original `views_sha256`, exact
`tool_sha256`, `rgb16_bpp`, and `development_complete_passed: true`. The coordinator
owns this receipt after complete development evidence; reserved runs reject a
missing receipt, another rate, another binary or view freeze, and all screen
requests. PAN16 remains 4 bpp. The receipt is retained and hashed in each run.

## Declared screen result

**The unchanged encoder fails the complete RGB16 development bracket.** All
five serial preparations and reconstructions completed without process failure
or retry at script checkpoint `1a1580b1a4e3557737ae3d0adcdfe1a8c48942f9`.
[The numerical record](viewer-acceptance-quality-results.json) binds every source,
crop, exact tile support, script, build, preparation, quality result and component
packet attribution. This is a screen rejection; no full-scene configuration was
selected and reserved Boca validation was not run.

| Screen | Payload bytes | Descriptors | Manifest | Failed display cells | Worst RMSE | Worst p99 |
|---|---:|---:|---:|---:|---:|---:|
| Mansfield RGB16 4 bpp | 352,530 | 3,299 | 1,234 | 24/48 | 18.6973 | 50 |
| Mansfield RGB16 8 bpp | 704,788 | 3,530 | 1,234 | 24/48 | 8.30454 | 22 |
| Mansfield RGB16 12 bpp | 1,056,940 | 3,665 | 1,237 | 6/48 | 3.53161 | 9 |
| Mansfield PAN16 4 bpp | 1,966,903 | 6,680 | 2,010 | 0/16 | 2.74760 | 7 |
| Tok RGB8 4 bpp | 1,573,332 | 11,042 | 1,811 | 24/48 | 9.63651 | 25 |

All payload, descriptor and manifest sizes satisfy their frozen ceilings. These
are cropped screen representations: RGB16 covers four original tiles, PAN16 15
and Tok 12. Mask storage and application-delivered bytes remain separate,
unproved predicates. At 12 bpp all six RGB16 failures are percentile-stretch
RMSE failures; p99 passes. The worst remains the frozen tile-crossing view's
third selected component (original MS band 2), at 1:1. Increasing bytes improves
quality through the bracket but does not meet every frozen gate. PAN16 passing
a crop screen does not establish complete-scene acceptance.

Final QCD values and actual packet header/body counts are available per tile and
component. They establish which component received bytes and its final shared
quantiser; they do not establish that a different allocation or encoder policy
would pass at matched bytes. No extra rate, encoder change or independent
comparison was run.

GDAL emitted missing PROJ database diagnostics during each crop's geospatial
metadata handling. Cropping required no reprojection, and every derivative's
selected sample arrays and tile supports matched the original exactly. No CRS
interpretation is claimed. All original source hashes match after measurement.
The single measurement operation and its single watcher completed. The next
coordinator decision is a bounded quality rejection or one separately selected
remaining authorised comparison/policy probe; a second rate sweep is excluded.
