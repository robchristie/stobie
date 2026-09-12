# Compact validity metadata-accounting qualification

**The repaired mask-cache accounting passes the scoped native/browser cohort.**
Strict pressure completion remains **44/47 windows per pass** under the unchanged
1 MiB compressed limit. The three excluded windows exceed that limit in image
bins alone. This report supersedes the rejected candidate’s retained-metadata
qualification; its [original observations](representation-efficiency-masks-results.md)
remain intact. Quality, normal latency, scheduling, RSS and complete-viewer
acceptance remain excluded. The imagery is **quality-rejected-diagnostic-only**.

The measured runtime is `a7cc4990b64bcb701a3e782f13b5a839fbf2fda8`, tree
`8494a3f24809e617012272d4c09379d88f59db13`. All native and WASM binaries were
rebuilt from that committed candidate. The [bounded JSON](representation-efficiency-masks-metadata-results.json)
contains build identities, canonical receipts, native records, retained-input
identities and the browser reconciliation.

## Retained metadata repair

Independent review rejected `1a6a92574f9181e707896f97dcb821906cad9a08` because
one-byte constants could populate uncharged mask-cache map entries. The repair
replaces that map with a dense, exact-length boxed slot array covering the
already authenticated tile/level catalogue. It charges every slot and the
container header to the existing descriptor budget before allocation and
publication. Keys are implicit indices; constants use inline enum states;
mixed/legacy bitmap payloads use exact-length boxed slices. No per-entry key,
map node, growable payload capacity or constant heap allocation remains.

The exact logical charge is **slot count × target slot size + container size**.
The container charge includes its pointer/length, tile-count and occupancy
fields; each slot includes its state and bitmap pointer/length storage.
Constants also retain the conservative one-byte encoded compressed charge,
which overlaps their inline state. Mask eviction releases payload bytes and
occupancy while empty slots remain charged. Refetch cannot grow metadata.
Representation eviction releases the entire array and container charge.

The native target uses **24-byte slots and a 32-byte container**; WASM uses
**12-byte slots and a 16-byte container**. With all five representations
resident, WASM retains **1,883 slots and 80 container bytes**, charging
**22,676 bytes** of mask-cache metadata plus **3,766 bytes** of catalogue string
capacity. Pressure descriptor residency peaks at **350,722 bytes**; the maximum
across all browser contexts is **351,860 bytes**. The browser report checks the
target sizes directly in every terminal record. During pressure, current
mask-cache metadata varies from **520 to 22,676 bytes** as whole representations
are evicted; empty slots remain charged within surviving representations.
These exact logical
allocation charges do not claim heap allocator headers, page-cache or total
process RSS measurements. No retained mask-entry metadata is excluded.

| Representation | Slots | Native slot/container metadata | Additional catalogue string capacity | Native peak descriptor residency |
| --- | ---: | ---: | ---: | ---: |
| Mansfield PAN16 / 4 bpp | 420 | 10,112 | 840 | 33,020 |
| Mansfield RGB16 / 12 bpp | 42 | 1,040 | 84 | 54,734 |
| Boca PAN16 / 4 bpp | 770 | 18,512 | 1,540 | 98,328 |
| Boca RGB16 / 12 bpp | 63 | 1,544 | 126 | 72,642 |
| Tok RGB8 / 4 bpp | 588 | 14,144 | 1,176 | 103,800 |

These charges are included in the unchanged **16 MiB descriptor limit**.
Per-representation encoded delivery, mask storage, regional validity and GPU
packing figures remain separately reported in the JSON. The existing decoded
and GPU limits include the requested regional validity and renderer packing.

## Exactness and fixed protocol

The native refresh completes the same **196 regions**, matching **2,170,388
samples and 1,523,664 validity values**. Every row satisfies the exact slot plus
container equality, occupancy bounds, descriptor budget and terminal reservation
release. The refresh rechecks **4,734 input identity records before and after**:
original/compact masks, image payloads, descriptors, manifests, conversion/oracle
receipts, requests and retained legacy arrays. No compact representation or
source imagery was regenerated.

The source oracle remains the previously authenticated transitive proof:
compact-to-legacy equality and per-band ALL/ANY plane hashes covering
**265,964,676 cells** agree with the retained original GDAL oracle. This is not
per-cell hashing or a new GDAL read. The exact immutable inputs and original
proof receipt are retained in this refresh’s identities.

The [browser refresh](representation-efficiency-browser-masks-metadata-results.md)
executes the same **2,311 terminal jobs in ten contexts**, including **2,212
normal completions**, all three deliberate faults and cancellation/recovery.
Every terminal record checks the new metadata counters and reservation release.
The unchanged pressure sequence executes **47 windows twice**, with **88/94
completions**, mask eviction and actual refetch of previously evicted keys.

| Excluded window on each pass | Image bins | Masks | Required total | Compressed limit |
| --- | ---: | ---: | ---: | ---: |
| Mansfield RGB16 index 174 | 1,056,882 | 4 | 1,056,886 | 1,048,576 |
| Mansfield RGB16 index 286 | 1,056,882 | 4 | 1,056,886 | 1,048,576 |
| Boca RGB16 index 118 | 1,147,574 | 196,612 | 1,344,186 | 1,048,576 |

No real-scene limit, request geometry, output size, schedule or splitting rule
changed. Safe rejection is explicitly excluded from successful pressure
completion. Actual NVIDIA/ampere hardware adapter discovery is distinct from
GPU rendering. The existing authored GPU validity readback remains applicable:
renderer/core/runtime and Cargo inputs are unchanged from its pinned successful
run, while the refreshed client cohorts prove exact validity after cache repair.

## Persistence and verification

The same five immutable compact representations occupy **27,017,220 logical
file bytes**, plus **691,672 bytes** of scene-group ancillary receipts/notices.
Masks remain **20,780,351 → 1,042,931 bytes (94.98% smaller)**. New manifests total
176,571 bytes against 172,805 legacy bytes. The JSON preserves per-scene image,
descriptor, mask, manifest, ancillary and fresh diagnostic-group totals separately.

Full `cargo xtask verify` passes on the committed repair, including formatting,
strict lint, tests, release native/WASM builds, architecture checks and browser
smoke. It ran in the documented private user/mount/network namespace with cached
dependencies, preserving the unrelated host listener. Authored tests cover
2,000 inline constants, exact legacy/bitmap bytes, overflow rejection, an actual
one-byte-short admission failure before slot allocation, eviction/refetch,
failed publication and representation release. Only the authored descriptor
pressure fixture adds the fixed container to its calculated boundary; this
preserves its original eviction invariant and changes no production limit.

Earlier setup failures and the rejected metadata verdict remain retained. The
refresh’s read-only launcher preflight also retained one `Path.parent` typo
failure before any output directory or native process; the corrected archived
launcher then completed successfully. Independent exact-head re-review and
landing remain coordinator-owned.
