# Exact representation validity

The optional `identity.validity` contract is `source-validity-v1`. Existing
representations without it remain unmasked and retain their existing identity.
It binds the original source SHA-256, original selected band numbers (in output
component order), source-grid geometry from the representation profile, policy
name and the SHA-256 of every tile/level sidecar. Validity is representation
metadata, independent of JPEG 2000 and JPIP; it is binary validity, not alpha.

Preparation reads `GDALGetMaskBand` for each selected original band at native
resolution. Every nonzero mask value is valid. Source pixels and analytical
arrays remain untouched. Explicit `--mask-input` and `--mask-bands` select the
original raster and bands when encoding a derived RGB product; `--mask-bits`
selects original storage precision when different from the product. Dimensions must
match exactly. No reconstruction-value or lossy-zero test is permitted.

For discard d, the output grid is globally anchored at (0,0), scale 2^d.
Cell (x,y) covers [x*scale,min((x+1)*scale,width)) by the corresponding y interval.
`all` is the AND of native validity in that clipped footprint; `any` is its OR.
At d=0 they are identical. Regional output uses ceil(start/scale) through
ceil(end/scale), matching the indexed codec output for unaligned requested edges.
Tile edges are divisible by every supported scale, so cross-tile boundaries
preserve the same global grid. The combined display validity is AND of selected
bands' `all` bits. Invalid display cells are opaque black independently of stretch;
valid source samples are never modified.

Quality evidence must score every native valid sample separately by band,
including valid-side ringing. At reduced scales retain per-band all/any coverage
and score partial footprints separately; combined display validity must never
remove a quality sample. Masking is not evidence of improved codec quality.

Sidecars are `masks/<discard>/<tile>.bin`, with component-major packed bit planes,
least significant bit first and zero padding. Each component has an `all` plane,
then an `any` plane for d>0. Plane length is ceil(tile output pixels/8). Tile and
level geometry are derived from the authenticated profile; payload length,
canonical padding, all-implies-any and digest are checked before admission.
Native and reduced masks together occupy less than one byte per source pixel
per band plus bounded tile padding. Preparation publishes all files atomically.

Delivery is `/mask/<target>/<discard>/<tile>?tid=<identity>` with exact length and
digest checks and stale-identity rejection. Required absent, corrupt or stale
masks fail explicitly before decoded publication. Mask residency shares the
existing compressed-data budget, including actual mask bytes and independent
mask eviction counters; selected regions that cannot fit fail without retries.
Masks are immutable cache entries keyed by representation, level and tile.
The additive `RegionalFrame` envelope preserves original `RegionalPixels` and
`RegionalUpload` struct literals and unmasked methods. Opt-in `complete_frame`,
`take_decoded_frame` and `upload_frame` carry validity; legacy `take_decoded` leaves
masked frames queued rather than dropping validity. Decoded combined validity
and GPU packing count against existing output and
texture budgets. Native and browser workers use the same Rust selection logic.

## Focused validation

The 11 September 2026 authored checks passed: three downstream compatibility/
validity-accounting tests and eight existing runtime regional tests; two
source-mask unit tests; three
mask preparation/service/cache journeys; three source-tool unit tests and eleven
existing delivery journeys. An explicitly run GDAL test wrote its own temporary
TIFF with nodata=191 and selected bands 3,2,1, then proved original GDAL validity
against native samples. It uses `EMUELLA_TEST_GDAL_LIBRARY` and stays independent
of protected imagery. A mistaken first authored window contained no nodata;
the corrected window contains both valid and invalid samples.

`cargo check -p emuella-viewer-tools -p emuella-viewer`, native all-target Clippy
and WASM Clippy passed. A WASM release build passed. Native GPU readback passed
four scalar/RGB cases, including binary validity, orientation, native precision,
opaque invalid black, and actual texture eviction within the existing byte limit.
The observed adapter was NVIDIA GeForce RTX 3090, Vulkan, driver 610.43.03.
Initial default GL device loss, missing Vulkan loader, and browser missing libatk
were environment setup failures, resolved using existing local dependencies;
they are not discarded performance or acceptance runs.

The actual browser Web Worker matched nine native regions (discard 0/1/2,
components 0, 1 and RGB) exactly for validity arrays and native sample FNV hashes.
This proves delivery and reconstruction agreement before UI rendering; it is
separate from protected full-scene quality, browser GPU inspection and final
candidate qualification. The coordinator owns canonical and exact-revision gates.

To reproduce the authored browser comparison, set `EMUELLA_MASK_FIXTURE` to a
fresh registered scratch child and run the `exact_native` mask integration test.
It retains `native-regions.json` beside the authored representation. Build/serve
the viewer with that representation, then run:

```sh
node tools/viewer-mask-agreement.mjs "$VIEWER_URL" \
  "$EMUELLA_MASK_FIXTURE/native-regions.json" "$CAMPAIGN_SCRATCH/browser-agreement.json"
```

The browser harness requires the ordinary pinned Playwright dependencies and
writes bounded aggregate evidence, without screenshots or source pixels.
Run native GPU proof explicitly with
`cargo test -p polyorama-render-wgpu --lib regional_gpu_readback -- --ignored --nocapture`.
Run original GDAL proof explicitly with
`cargo test -p emuella-viewer-tools --test masks original_gdal -- --ignored --nocapture`.
