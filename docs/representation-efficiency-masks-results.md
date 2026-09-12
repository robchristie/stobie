# Compact validity qualification results

> Historical observation: independent review rejected this candidate’s resource
> qualification because mask-cache entry metadata was uncharged. Exactness and
> delivery observations below are retained unchanged; they do not qualify the
> repaired metadata accounting runtime.

**Exact validity and compact payload qualification passed, with three oversized
pressure windows excluded on each pass.** The strict pressure predicate remains
failed at **44/47 per pass**. Historical quality, normal-latency, scheduling, RSS
and complete-viewer exclusions remain unchanged. This imagery is
**quality-rejected-diagnostic-only**; mask compaction does not improve its samples.

The runtime was compiled from `264c92d44546e5e08113709eb6aec79b60d01788`, tree
`db33806a63d4f4d774b3c45be7761101bad854ef`. Browser protocol
`cfb78b652f1b8338086a9d43c671eee417fd7242` changed only its harness, tests and
protocol document. Its build receipt rehashed unchanged Cargo, binaries and
static assets and explicitly records this reuse. The
[bounded JSON record](representation-efficiency-masks-results.json) contains
all source, build, native, storage, resource and browser evidence identities.

## Persistence and immutable imagery

All figures are actual logical file bytes. Payload and descriptor contents are
unchanged; every old/new manifest differs only in validity version/catalogue and
its resulting TID. Original manifests match the historical SHA-256 catalogue,
original source hashes/band order and retained native TIDs. Source files were
hashed before and after the operation and remained unchanged.

| Representation | Image payload | Descriptors | Legacy masks | Compact masks | New manifest | New representation total | Ancillary receipts/notice |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Mansfield PAN16 / 4 bpp | 6,677,852 | 25,166 | 2,781,284 | 420 | 39,208 | 6,742,646 | 151,447 |
| Mansfield RGB16 / 12 bpp | 1,252,060 | 5,042 | 521,826 | 42 | 5,120 | 1,262,264 | 36,399 |
| Boca PAN16 / 4 bpp | 7,738,315 | 37,086 | 5,851,548 | 595,670 | 70,809 | 8,441,880 | 258,335 |
| Boca RGB16 / 12 bpp | 1,563,932 | 6,329 | 1,097,535 | 446,211 | 7,018 | 2,023,490 | 42,828 |
| Tok RGB8 / 4 bpp | 8,422,520 | 69,416 | 10,528,158 | 588 | 54,416 | 8,546,940 | 202,663 |

Mask payload fell from **20,780,351 to 1,042,931 bytes: 94.98% smaller**.
There are 1,456 all-valid, 329 all-invalid and 98 mixed sidecars. Uniform scenes
need one byte per tile/level; mixed Boca tiles retain exact bitmap planes.
New catalogue tags add **3,766 serialised and retained string-capacity bytes**,
charged to the existing descriptor budget. Manifest totals are 176,571 bytes
versus 172,805 legacy bytes. Representation persistence totals **27,017,220
bytes**, plus **691,672 bytes** of scene-group ancillary receipts and notice.
The JSON separately counts native/browser diagnostic groups and profiles; these
are not hidden in representation persistence or claimed as physical disk I/O.

## Exactness and actual native/browser delivery

All **1,883 sidecars** were authenticated and compared to their legacy bitmaps.
Independent per-band ALL/ANY plane hashes covering **265,964,676 cells** matched
the retained original GDAL oracle, with zero false-valid or false-invalid cells.
This is transitive source-oracle agreement: original source and legacy manifest
bindings were verified, compact-to-legacy equality was proved, then the retained
oracle hashes were recomputed. It is neither a new GDAL decode nor per-cell
hashing. Comparator tile-array scratch peaked at **720,897 bytes**, separate
from client residency.

The new native service/cache completed **196 regions**, covering first/last
retained windows for every level and component selection. All **2,170,388
samples** and **1,523,664 validity values** matched retained native arrays.
Every scoped request released its working-set reservation and pin metadata.

| Native scene | JPP body bytes including framing | Mask body bytes | Descriptor body bytes | Peak compressed residency | Peak descriptor residency | Peak codec workspace |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Mansfield PAN16 | 154,726 | 14 | 738 | 154,592 | 22,908 | 2,221,866 |
| Mansfield RGB16 | 443,612 | 14 | 1,677 | 443,230 | 53,694 | 2,205,033 |
| Boca PAN16 | 563,896 | 109,258 | 2,466 | 672,703 | 79,816 | 1,472,510 |
| Boca RGB16 | 787,028 | 327,662 | 2,110 | 1,114,248 | 71,098 | 2,230,081 |
| Tok RGB8 | 246,251 | 21 | 2,360 | 245,740 | 89,656 | 2,247,991 |

The [actual hardware-browser result](representation-efficiency-browser-masks-results.md)
completed all **2,212 normal jobs**, matching **23,501,248 samples** and
**17,037,656 validity values**. All catalogue bodies, 493 successful audited
Worker mask deliveries and 8,580 oracle-plane comparisons agreed. Missing,
corrupt and stale mask faults passed, as did cancellation at the first mask
fetch followed by exact recovery. All **2,311 terminal jobs** exposed zero
remaining reservation and pin bytes.

Actual audited Worker mask bodies totalled **1,584,237 compact bytes**, versus
11,267,355 equivalent legacy bytes for those same keys. End-of-context Worker
counters recorded 18,494,169 JPP bytes, 102,445 descriptor bytes and 1,682,542
mask bytes; the latter includes the injected corrupt body. Browser/HTTP counters
and integrity/source reads have distinct boundaries, documented in the JSON.

## Pressure and resource boundaries

The original 47 windows, two passes, ordering, geometry and **1,048,576-byte**
compressed limit remained fixed. Pressure completed **88/94 jobs**, observed
112 mask evictions and refetched 52 previously loaded mask keys. These are the
three excluded windows in each pass:

| Scene / original request index | Image bins | Masks | Total |
| --- | ---: | ---: | ---: |
| Mansfield RGB16 / 174 | 1,056,882 | 4 | 1,056,886 |
| Mansfield RGB16 / 286 | 1,056,882 | 4 | 1,056,886 |
| Boca RGB16 / 118 | 1,147,574 | 196,612 | 1,344,186 |

Each image-bin dependency alone exceeds the limit. Their safe failures remain
excluded, never counted as completed windows. The historical fourth exclusion,
Boca RGB16 / 286, now completes with compact masks. No output splitting, smaller
request, scheduler change or budget increase was used.

Pressure peaks were 1,048,532 compressed bytes, 196,613 mask bytes, 338,594
descriptor bytes, 984,340 reserved working-set bytes, 2,318 pin-metadata bytes,
3,766 new catalogue-metadata bytes and 1,953,079 codec-workspace bytes. WASM
linear memory peaked at 7,536,640 bytes. Constants remained one-byte encoded
cache entries; only requested regional validity expanded. Exact mask-vector
capacities, source-tag capacities and existing decoded/GPU packing accounting
are checked. The JSON reports regional output sizes and clearly labels derived
GPU packing sizes separately from actual rendering.

The pre-existing mask BTreeMap's node/allocator overhead remains outside payload
counters. Compaction introduces no new map fields but can retain more entries;
this overhead is **not claimed as measured**. The evidence is logical accounting,
not total allocator or process RSS qualification. Independent resource-contract
review must assess this inherited boundary; no new RSS campaign was performed.

## Verification and retained setup failures

`cargo xtask verify` passed on the runtime candidate, including Python/Rust/Node
tests, native/WASM lint and release builds, architecture, deterministic UI
snapshots, browser smoke and native smoke. The browser-only successor passed
all ten affected authored tests and reused unchanged build/check inputs with
explicit provenance. Ordinary canonical browser smoke uses its established
software renderer; it is not the hardware-browser mask proof.

The explicit native GPU readback passed on NVIDIA GeForce RTX 3090, Vulkan,
driver 610.43.03, covering scalar/RGB orientation, precision, invalid-black
rendering and texture eviction within the existing 64-byte authored limits.
Renderer/core/runtime source and dependencies were unchanged by the later
mask-encoder allocation repair. All ten real browser contexts separately
observed a non-fallback NVIDIA/ampere adapter and RTX 3090 Vulkan renderer.
The actual mask reconstruction/comparison runs in the CPU WASM Worker; adapter
observation alone is not a whole-scene GPU viewing claim.

Two canonical setup attempts are retained: `.tools` symlink rejection by
existing UI safety tests, then an unrelated listener occupying smoke port 4174.
The successful unchanged checks used real directory binds into registered
scratch and an isolated loopback namespace with cached dependencies. The
unrelated process was preserved. The first hardware-browser invocation launched
ten contexts but zero Workers/jobs/mask deliveries because initial adapter
discovery returned null. Authored setup probes isolated a single initial CDP
GPU-discovery handshake; the corrected one-invocation protocol passed without
workload retries. All failed receipts and protected evidence remain retained.

The codec remains pinned at `6586e3d50f95429b242cb2e3535742b002784f2d`.
Encoding work landed only diagnostic documentation at
`08fd8dfc80c3475209680032ce2c097409716246`; codec source/build inputs are identical,
so no consumer pin change was needed. Independent exact-head review, final CI
and merged confirmation remain delivery evidence, not facts predicted here.
