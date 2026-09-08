# Emuella viewer candidate qualification

The independently authored NITF-to-HT viewer candidate passes its frozen
preparation, native/browser latency, resource and recovery limits. Original
WorldView delivery and real-scene visual qualification are explicitly deferred
under the revised scope. This record establishes candidate qualification;
reviewed landing and the merged-consumer confirmation belong to the delivery record.

## Current reproducible journey

Follow the [independent NITF commands](emuella-viewer-independent-nitf.md),
[preparation measurements](emuella-viewer-nitf-preparation.md),
[service contract](emuella-viewer-service.md) and
[composed workload commands](emuella-viewer-calibration.md).
The [qualification report](emuella-viewer-evidence/nitf-qualified/qualification.json)
and [file manifest](emuella-viewer-evidence/nitf-qualified/manifest.json) retain
exact source, binary, preparation, request, resource and reference evidence.
No external image pixels are included.

Native/static binaries were built from clean Polyorama
`16296520e146df842555a8d763fcce704207adc8`, which passed full `cargo xtask verify`.
The composed harness ran at `3ad17777371e5069db902049e799008c5e8713e7`; intervening
changes are evidence, documentation and visual-provenance labelling, with unchanged
native/static binary hashes. Codec and JPIP use merged
`2568f1c40c83a40f527c7ee8f1600af511e046d0`; GDAL plugin uses merged
`f5e21e5b9bdd9ffab95f19299e2ceb81a18fee84`, maintained GDAL
`1af54d99959f3b62ba10451a357a969075374663`, and benchmark admission uses merged
`3bdaa98449e2c4b6c0500a1ac9daedb4d61d92bc`.
Independent fixture ownership is testdata `2d519ddaf019f10b9e409ea3338d395438486647`.

## Ingestion and representation

The independently OpenJPEG-encoded NITF C8 source is 43,008 by 43,008, with
11 meaningful unsigned bits in UInt16 storage. Its SHA-256 is
`dcf61436814d33c75c876b173f2dc87fcbd61571be7e766489dd884ab9891c07`.
The recipe independently decodes all 1,849,688,064 samples and checks its authored
coordinate oracle. The source occupies 203,866,694 bytes; its native raster is
3,699,376,128 bytes. Small independent fixtures additionally cover U16 greyscale,
RGB8, RGB16 and lossy U11 ingestion; they do not establish large RGB performance.

GDAL/Emuella prepares one tiled HT codestream with tile edge 512, six
decompositions, 64-sample code blocks, no MCT, 2 bpp target and one genuine quality
layer. The encoded parent occupies 461,122,081 bytes plus 3,021,780 descriptor
bytes. Both preparation invocations produce identical manifests and payloads.
All 7,056 native tile callbacks and 3,699,376,128 returned sample bytes are counted.
There is no whole-image pixel allocation. Required indexing constructs the
bounded retained Part 1 source index during GDAL open; regional reads reuse it.

| Preparation observation | Cold OS source, traced | Warm OS source, uninstrumented |
|---|---:|---:|
| Wall time | 407.298 s | 349.966 s |
| Frozen maximum | 520 s | 440 s |
| Encoder-process peak RSS | 38,244 KiB | 38,476 KiB |
| Peak transient HT tile index | 16,544 bytes | 16,544 bytes |
| Attributed source-file read bytes | 18,457,266,540 | Unavailable by design |
| Attributed source-file read operations | 4,395,262 | Unavailable by design |

The separate [limits](../apps/emuella-viewer/qualification/README.md) were frozen
before either full-size invocation. They extrapolate 8,192/32,768 baselines with
a 25% allowance, after merged-owner 8,192 confirmations reproduced the payload,
source read counts and bounded resource behaviour. Failed runs cannot relax them.

Source-file residency is checked after wrapper hashing and immediately before
launch. The tool's own timed source hash scan warms pages before decoder reads.
Syscall bytes include that scan and rereads; they are not physical-device or NFS
traffic. The encoder RSS excludes the measurement wrapper and operating-system
page cache. The wrapper streams hashes/traces and verifies descriptors; its
process peak is not separately qualified. Process-wide physical read counters are
retained separately without assigning them to the source by inference.

This arithmetic source compresses better with its original lossless Part 1
encoding than the selected HT target: the viewing representation is 2.26 times
its source size. It is about eight times smaller than the native UInt16 raster.
Neither ratio establishes storage savings for actual satellite imagery.

## Native, browser and recovery

One fresh native, one real-browser and one recovery journey pass every unchanged
frozen admission using the NITF-derived parent plus eight previously prepared
U11/U16 greyscale and RGB8/RGB16 parents. All representation files remain
hash-identical before and after viewing; auxiliary preparation is not repeated.
The first attempted group failed before rendering because its X display was no
longer live. The [failed admissions and logs](emuella-viewer-evidence/nitf-qualified/failed-display/assessment.json)
remain retained. A fresh display restored hardware execution with unchanged
binaries, representations and limits; no cause for the earlier display exit is inferred.

The host has a Ryzen 9 9950X3D, 32 logical CPUs and 91 GiB RAM. Native reports
RTX 3090 Vulkan with NVIDIA 610.43.03. Chromium 151.0.7922.34 reports NVIDIA/Ampere
and withholds its device model. Canonical llvmpipe smoke evidence is separate.

| Observed boundary | Native | Browser |
|---|---:|---:|
| First primary region resident | 83.57 ms | 161.40 ms |
| Complete overview and visible gallery | 3,695.01 ms | 14,376.40 ms |
| Detection detail | 103.43 ms | 233.30 ms |
| Gallery completion | 653.80 ms | 1,848.60 ms |
| Warm GPU/bookmark revisit | 53.63 ms | 213.90 ms |
| Compressed-cache reconstruction | 3,857.26 ms | 14,475.80 ms |

The overview comprises 121 bounded primary demands. First-region residency is
neither complete-image readiness nor scanout. These are bounded integration
confirmations, not new five-run baselines or population tail estimates.

The workload covers nine open image identities, linked/comparison panes, five
bookmarks, pan/zoom, 10,000 logical clustered/scattered detections, visible items
and bounded overscan, detail opening, cancellation and stale completion rejection.
Warm compressed reconstruction requests no additional JPP data; warm GPU revisit
performs no further decode or upload. Recovery separately proves interrupted
receipt/retry, reconnect, eviction, cancellation acknowledgement and rejection of
an actual delayed completion. Its three Worker creation events are sequential
fresh contexts; concurrency remains one.

Global caps remain 64 MiB compressed bins, 16 MiB descriptor metadata, 16 MiB
decoded data, 64 MiB logical GPU textures and 64 MiB codec workspace. Network/TCP
bytes include headers and retries; service read counters and codec block/pixel/
synthesis work are separate observations in each trace. Browser memory uses
sampled descendant RSS and observed per-PID high-water sums, with shared-page and
sampling limitations. Physical GPU allocation overhead and GPU timing are unavailable.

## Correctness and visual assessment

The [complete-reference comparisons](emuella-viewer-evidence/nitf-qualified/reference/comparisons.json)
pass all 18 groups: 807 application records and 12,109,360 pixels, with zero
checksum, dimension or precision mismatches. They read complete selected tiles
from immutable files and crop, bypassing JPP and compressed-cache assembly.
Native and browser agree exactly on all 352 shared retained records. FNV-1a U16LE
is a diagnostic checksum, not a cryptographic image identity or exhaustive pixel
difference count. The reference shares codec algorithms; owner tests separately
provide independent source-encoder evidence and full-tile reconstruction checks.
The reference command's 224 actual-array cases cover four formats, odd edges,
tile crossings and every D6 discard level.

[Opened captures and assessment](emuella-viewer-evidence/nitf-qualified/visuals/assessment.json)
show coarse fallback during a held detail response, small authored circles and
rectangles, modular-ramp and vertical source edges, and aggressive stretch.
Stretch changes no compressed request or decode count. No blank regional strip
is apparent; numerical comparison independently checks region boundaries.
One-code faint perturbation fidelity is not established visually at this lossy
target. Satellite object quality and sensor-specific stretches remain deferred.

## Supported scope and next increment

| Capability | Supported proof / remaining boundary |
|---|---|
| Source integration | Independently authored NITF C8 through maintained GDAL/Emuella; original vendor qualification deferred |
| Codec/index | Reusable bounded Part 1 source index; immutable tiled HT packet descriptors and sparse regional decode |
| Delivery | Actual stateless JPP windows, resolution/components, supported layer selection, partial databins and explicit cache prefixes |
| Standards | Part 9:2023 / T.808 12/2022; Part 15:2019 / T.814 06/2019; June 2026 Part 9 differences unverified, no newer conformance claim |
| Quality | One genuine HT layer with resolution progression; multiple HT sets and layered Part 1 reference measured by the codec owner |
| Presentation | Native/browser workers, multiple panes, virtualised parent-derived thumbnails and global budgets |
| Excluded | Full JPIP optional features, standalone chips, spatially varying compression, reprojection, Geometis migration/deployment and publication |

The [codec quality calibration](https://github.com/emuella/emuella-j2k/blob/2568f1c40c83a40f527c7ee8f1600af511e046d0/docs/ht-quality-calibration.md)
records stored and cumulative delivered bytes, work and visual trade-offs.
Extra layer counts and placeholder passes are not treated as refinement payloads.
Representation identity includes source, band selection, encoding contract,
codec revision and spatial-policy hash; future policy changes cannot reuse stale
compressed data. Display stretch remains outside compressed identity.

The next increment should measure and reduce source read amplification through
bounded read coalescing at the GDAL/VSI boundary, then reduce synthesis work for
warm compressed revisits. Those costs are now separately observable. Calibrate
real-scene storage and quality only after an appropriate rights record is available.
Eventual Geometis integration should consume these independent regional-demand,
immutable-representation and JPIP contracts after representative vendor qualification.
No Geometis implementation was inspected for this proof.

Earlier [five-native/five-browser qualification](emuella-viewer-evidence/qualified-public/qualification.json),
[complete reference](emuella-viewer-evidence/complete-reference/comparisons.json) and
[response-admission repair](emuella-viewer-evidence/response-repair/qualification.json)
remain historical evidence with their own exact revisions. The earlier 1 MiB
probe's failed extra assertion about unused representation registrations is retained;
it is not replaced by a passing claim. Current qualification above adds actual
independently encoded NITF preparation and the merged codec index repair.
