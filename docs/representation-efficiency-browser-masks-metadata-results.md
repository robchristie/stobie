# Metadata-qualified compact validity browser results

**Scoped mask qualification passed with explicit oversized pressure exclusions.**
The strict 47-window pressure criterion remains failed: **44/47 completed in each
pass, 88/94 total**. No full viewer, imagery quality, timing, speed or process RSS
acceptance follows from this result.

The fresh invocation used the repaired protocol and freshly compiled runtime
`a7cc4990b64bcb701a3e782f13b5a839fbf2fda8`, tree
`8494a3f24809e617012272d4c09379d88f59db13`, after complete canonical verification.
Service SHA-256 is `a7538a747eccde1100f77843317e79ab111b976d09170c70b67d895c8557bf79`;
WASM SHA-256 is `b814e50d113a814b597452f064943f969317a0ce3b24f81d0bb3767e60bdf93f`.
All five compact conversions and original native references were rehashed and
reused unchanged; no imagery was reconverted or re-encoded.

All ten contexts reported Chromium **151.0.7922.34**, pinned Playwright **1.62.1**,
an actual non-fallback **NVIDIA/ampere** WebGPU adapter and **GeForce RTX 3090**
Vulkan renderer, driver **610.43.3.0**. A single CDP GPU-discovery handshake
preceded the sole adapter observation in each context. This binds a hardware
browser environment; the mask comparisons execute in the actual CPU WASM Worker.
Native GPU readback is separate owner evidence.

| Scene | Normal completed jobs | Samples compared | Validity values compared |
| --- | ---: | ---: | ---: |
| Mansfield PAN16 | 252 | 1,955,684 | 1,955,684 |
| Mansfield RGB16 | 504 | 5,867,832 | 3,911,888 |
| Boca PAN16 | 280 | 2,154,788 | 2,154,788 |
| Boca RGB16 | 672 | 7,655,112 | 5,103,408 |
| Tok RGB8 | 504 | 5,867,832 | 3,911,888 |
| Total | **2,212** | **23,501,248** | **17,037,656** |

Every returned sample and validity value agreed with the retained native arrays,
including invalid cells. Difference, false-valid, false-invalid and nonbinary
counts were zero. Seven-level, component-selection, source-boundary, cross-tile
and clipped-edge coverage passed. Complete source-oracle validity agreement is
transitive through the independently checked legacy masks, not a new TIFF decode.

All **1,883** catalogue mask responses agreed with retained canonical bitmap
bytes and independent ALL/ANY oracle plane hashes. The catalogue contained
**1,456 all-valid, 329 all-invalid and 98 mixed** files. Actual compact catalogue
bodies totalled **1,042,931 bytes**, compared with **20,780,351** equivalent legacy
bitmap bytes. The **493 successful audited Worker mask deliveries** totalled **1,584,237
compact bytes** and **11,267,355 equivalent legacy bytes** for those same keys.
All **8,580 oracle-plane comparisons** agreed, with zero byte/padding/ALL-without-ANY
errors. This is byte accounting, not a transfer-rate or speed measurement.

Missing, corrupt and stale cases each emitted one `Failed`, matched the exact
request, decoded nothing and released reservation/pin metadata. Cancellation at
the first actual mask fetch emitted one `Cancelled`, decoded nothing and exposed
**zero** request working-set and pin-metadata bytes; the same region then
completed exactly with a fresh token. Across all **2,311 terminal jobs**, both
release fields were present and zero. Terminal counts were **2,301 Completed,
nine Failed and one Cancelled**; six failures are the pressure exclusions below.

| Scene / original zero-based request index | Passes | Required image bins | Required masks | Required total |
| --- | --- | ---: | ---: | ---: |
| Mansfield RGB16 / 174 | forward and revisit | 1,056,882 | 4 | 1,056,886 |
| Mansfield RGB16 / 286 | forward and revisit | 1,056,882 | 4 | 1,056,886 |
| Boca RGB16 / 118 | forward and revisit | 1,147,574 | 196,612 | 1,344,186 |

The limit remained **1,048,576 bytes**, with the original 47 windows, two passes,
ordering and output geometry. Each row exceeds it even before all masks fit.
Every failure remains excluded, never counted as a completed window. Actual
pressure observations included **112 mask evictions** and **52 previously loaded
mask keys refetched**. Exact samples/validity and release passed for every one of
the 88 completed pressure jobs. Strict complete-window pressure remains false.

Pressure peak accounting was **1,048,532 compressed bytes**, **196,613 mask bytes**,
**350,722 descriptor bytes**, **984,340 reserved working-set bytes**, **2,318 pin
metadata bytes**, **3,766 additional catalogue metadata bytes** and **1,953,079
codec workspace bytes**. Peak WASM linear memory was **8,388,608 bytes**. These
are separately reported logical/runtime counters, not process RSS. Every one of the 2,311 terminal rows passed exact
slot/container equality, occupancy and current/peak bounds, including cache plus
catalogue metadata within descriptor residency. With all five representations
registered, cache metadata was **22,676 bytes = 1,883 slots × 12 WASM bytes + 80
container bytes**. Normal/fault/cancellation rows retained that complete charge.
Pressure current charge ranged **520–22,676 bytes** as whole representations
were evicted; the peak remained 22,676. Payload eviction does not remove the
retained representation's slot charge. The receipt includes consecutive pressure
eviction observations with unchanged slot capacity and unchanged metadata. Missing/corrupt/stale faults and cancellation exposed zero occupied
entries while retaining the same 22,676-byte charge. Constants have no separate
payload heap; their logical encoded byte remains conservatively compressed-budgeted.
Normal-context peak descriptor accounting was **351,860 bytes**. The JSON
separates normal-context maxima and persistence fields for each representation;
image payload, descriptors, masks, manifests and added metadata are distinct.

The proxy recorded **7,388 requests**, **105,265,035 dynamic HTTP body bytes**,
maximum two in flight and zero proxy errors. That byte counter excludes static
code/WASM bodies. End-of-context Worker counters totalled **18,494,169 JPP
transport bytes**, **102,445 descriptor bytes** and **1,682,542 mask bytes**;
mask reception includes the corrupt-body fault and JPP reception includes framing.
There were no unexpected envelopes, Worker errors or page
errors. The reconciliation checked every ordered journal job against its context
and frozen plan; all immutable-input rechecks passed. The invocation exited
**0**; its owned service exited **−15** and the recorded PID was absent.

The [bounded JSON report](representation-efficiency-browser-masks-metadata-results.json)
binds all input, environment, build, invocation, exit, context and journal hashes.
Protected receipts remain in approved
`representation-efficiency-browser-masks-candidate-03` and matching `-execution`
groups. The initial `candidate-01` hardware-discovery failure retains ten launched
contexts, zero Workers/jobs/mask deliveries and exit 2. Three GPU-only authored
setup diagnostics remain separate; none is a mask measurement or hidden retry.
See the [fixed protocol](representation-efficiency-browser-masks.md). The earlier
[candidate-02 observations](representation-efficiency-browser-masks-results.md)
remain historical exactness evidence; independent review rejected their memory
qualification because cache-entry overhead was uncharged. The fresh candidate-03
run supplies the corrected metadata qualification without changing pressure
acceptance or relabelling earlier outcomes.

The execution group also retains the authored reconciliation source and bounded
merged-confirmation probe with SHA-256 identities, so scratch cleanup cannot
remove their sole replay source. The separate merged probe binds both imported protocol
source hashes, checks the corrected metadata formula, and requires retained cache
metadata alongside zero reservation/pin fields. It requires a fresh
merged-revision/repaired-build-input receipt; these candidate observations do not claim
post-merge verification.
