# Frozen independent-encoder screen

Status: **ready for coordinator commit; unmeasured**. This component is one
bounded calibration cohort, owned by Polyorama. Its question is whether an
installed independent encoder can meet the same screened format and actual
payload size, and whether its reconstruction passes the existing source-valid
display gates. The evidence owner is the fresh attributed RarePlanes group
`viewer-acceptance-independent-cohort-01`. The exit condition is two terminal
product records: matched and scored, descriptive/unmatched, or unsupported.
No full scenes, encoder search, second sweep or codec edits belong to this work.

The [machine protocol](viewer-acceptance-independent.json) freezes input,
baseline, environment and dependency identities. The
[harness](../tools/viewer-acceptance-independent.py) refuses execution unless all
protocol-bearing repository files equal their contents at the exact supplied
committed HEAD. Other workers' unrelated changes do not invalidate that check.
The coordinator must commit this complete component and supply a timing grant
to a fresh execution context. Readiness is not permission to measure now.

## Authority and installed tool evidence

Only `/usr/bin/ojph_compress` and `/usr/bin/ojph_expand` are used, as black boxes.
Local `pacman -Qi openjph` identifies version `0.30.1-1`, BSD-2-Clause, installed
2 July 2026 and signature-validated. The installed
`/usr/share/licenses/openjph/LICENSE` permits source and binary use, with or
without modification. Its copyright holders include Aous Naman, Kakadu Software
Pty Ltd and the University of New South Wales. That notice grants use of this
OpenJPH package; it is not evidence of a licence for a separate Kakadu encoder.
No acquisition, licence acceptance or external implementation/standards source
consultation occurred. The package metadata, complete installed licence,
binaries and resolved linked libraries are bound by hashes in the protocol.

No-argument installed CLI help documents TIFF input/output, irreversible 9/7,
colour transform control, decomposition count, block/tile dimensions, origins,
LRCP progression and the qstep range 0.00001–0.5. `-h` is unsupported; preparation
only invoked these help forms without input/output paths. No encoding,
reconstruction, synthetic codec probe or quality measurement has run.

Reviewed RarePlanes authority and exact source coverage remain those recorded
by the workspace and testdata owners. Native sample derivatives, reconstructed
rasters, every trial payload, process logs, errors and numerical results stay
under the approved store:
`/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1`.
`fresh_group` retains the notice, attribution and modification lineage. Product
directories inherit that group lineage and each record binds its precise parent
crop, source metadata and existing representation. No protected samples enter
Git or build scratch. The harness disables core dumps and directs subprocess
working directories, temporary files and Python/native diagnostics into the
group. It never deletes evidence or reuses an output group.

## Exact cohort and sample contract

| Product | Existing crop group suffix | Native bands | Geometry | Precision | Target payload |
|---|---|---|---|---|---:|
| Mansfield RGB16 12 | `quality-screen-rgb16-12-01` | 5, 3, 2 | 1024 × 688 | unsigned 16 | 1,056,940 B |
| Tok RGB8 4 | `quality-screen-tok-rgb8-4-01` | 1, 2, 3 | 3072 × 1024 | unsigned 8 | 1,573,332 B |

Both group names have prefix `viewer-acceptance-`. Their `source.tif` files
preserve original bands; their `views.json` files carry the translated frozen
views. The `-encode/representation/payload.j2c` siblings supply the byte targets.
The protocol binds those files, manifests, crop lineage, baseline preparation,
quality results and retained native reference views. Existing exact source/crop
equality evidence is reused; no new crop or original full-scene pass is needed.

The harness creates one uncompressed, three-band TIFF per product, copying the
listed native bands in 128-row strips and proving every sample equal after
reopening. It checks unsigned stored type, identity scale/offset and geometry.
There is no normalisation, U8 stretch, clipping, masking or resampling at input.
The RGB16 precision remains 16 even though observed values occupy fewer bits.

Both streams must have the crop's exact zero-origin geometry and tile support,
512-square tiles (including RGB16's 176-row edge), six decompositions,
irreversible 9/7, no MCT, 64-square HT blocks, default precincts, LRCP and one
quality layer. The command explicitly sets every available corresponding CLI
option and disables TLM. It leaves optional tile-part divisions, profile and
comments unset. A narrow marker guard checks both emitted and baseline streams,
including tile overrides, precision/signedness and component sampling. It
requires one observed tile part per tile, skips packet bytes, and rejects
unhandled coding markers/overrides. This guard is not general conformance proof.
Its metadata record contains hashes and dimensions, never packet excerpts.
If this installed tool cannot meet the contract, record unsupported and stop
that product without another tool or settings search.

## Frozen rate algorithm and invocation budget

Run RGB16 first, then Tok, serially in the single granted measurement window.
Each product gets **at most eight** `ojph_compress` invocations, including failed
or timed-out invocations. There are no endpoint probes or preparatory encodes.

1. Initialise lower qstep to decimal `0.00001` and upper to decimal `0.5`.
2. Compute `sqrt(lower * upper)` with Python Decimal precision 50, round-half-even.
   Format it as `.16E` (17 significant digits); this exact string goes to the CLI.
   The first qstep is `2.2360679774997897E-3`.
3. Retain the command, exit status, stdout, stderr, output hash, complete payload
   length and bounded format metadata before deciding the next action.
4. Stop immediately on the first format-conforming positive payload satisfying
   `abs(actual_bytes - target_bytes) * 100 <= target_bytes`. Both sides count;
   the tolerance is inclusive and uses integer arithmetic. Payload means the
   complete raw codestream file including framing, with no padding, descriptor,
   mask, manifest or container adjustment.
5. If too large, replace the lower bound with the exact submitted decimal qstep.
   If too small, replace the upper bound. Repeat from step 2. No source/display
   quality statistic, component allocation, filename result or timing informs it.
6. A process failure, timeout (180 seconds), missing/malformed payload or format
   mismatch ends that product as unsupported. After eight successful but
   unmatched payloads, record descriptive/unmatched. Never select the nearest
   trial, decode an unmatched trial, restart, widen the range or vary settings.

The initial bounds are virtual; reachability and monotonicity are not established
by extra probes. Consequently this budget may not find a match even when one
exists. An unmatched record supports no encoding-efficiency conclusion. Every
trial and failure remains in place. The fixed fresh group is a permanent cohort
claim: a crash also consumes this cohort, and the harness refuses a rerun.

## Reconstruction, scoring and diagnosis

Only after matching, expand that independent payload once with `ojph_expand`
into a native TIFF at full resolution, with resilient decoding disabled. Verify
geometry, components, unsigned precision and identity scale/offset. Score the
same four translated views, original source-valid populations, frozen stretches
and scales 1 and 4 through the existing `source.stretch`, `quality.errors` and
`quality.display_populations` functions. Fourfold arithmetic means precede the
display stretch. Every valid band/display cell needs RMSE ≤3 and p99 absolute
error ≤12 U8 units. Retain ANY/ALL/PARTIAL-valid and historical all-pixel metrics;
validity comes from original samples, never lossy zeros. No capture is necessary
for this numerical question and no agent visual acceptance is inferred.

Also expand the **existing Emuella stream** once with OpenJPH. Compare its four
frozen reconstructed windows to the retained Emuella U16LE references, whose
hashes are already bound to the baseline quality record. Retain exact sample
agreement and raw per-band errors. Exact equality is reported separately and is not an encoding-efficiency gate.
This is independent decoder agreement for
the Emuella stream, not independent encoding efficiency. A mismatch is retained
and suppresses the harness's encoding comparison claim; it does not trigger
another decoder or settings search. Failure of either expansion is unsupported
reconstruction. At most two expansions per matched product; no repeated timing.

The predeclared engineering hypothesis is: **current Emuella per-tile
quantiser/rate allocation leaves recoverable display distortion at this aggregate
payload**. Both existing baselines fail gates. If both matched independent
products pass every gate and both Emuella reconstruction checks agree exactly,
the result supports a separate investigation of that hypothesis. Otherwise this
cohort does not support it. This deliberately conservative rule cannot prove a
specific mechanism: an independent encoder also differs in transform arithmetic,
quantisation and entropy implementation. Thus even a positive result justifies
no particular minimal codec edit by itself. Report per-cell results and byte
deltas, the hypothesis disposition, unsupported/unmatched cases and that causal
limit. No second sweep, automatic policy change or full-scene acceptance follows.

## Commit gate, execution and verification

Synthetic regressions (no codec processes or protected data access):

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/tests -p test_viewer_acceptance_independent.py
```

Readiness verification: 14 independent-harness regressions and all seven
existing quality-harness regressions pass. These cover the exact rate sequence,
inclusive byte tolerance, invocation/failure stops, format overrides, commit/grant
gates, native U16 band ordering across strip boundaries, retained validity and
paired quality-population equality. Owned-file whitespace checks also pass.

The coordinator owns canonical repository verification, independent review and
Git delivery. This readiness-only task runs focused synthetic checks and exits;
it performs no Git commits/pushes, full application build or downstream delegation.

Before execution, commit every `committed_files` entry in the JSON protocol and
retain the exact resulting revision. Frozen dependency/environment hashes must
still match; resolve any concurrent dependency change before committing this
protocol, not by bypassing a check during measurement. Supply a coordinator-owned
grant with the following fields (placeholders must be replaced):

```json
{
  "schema": "viewer-acceptance-independent-grant/1",
  "revision": "<exact 40-character Polyorama HEAD>",
  "protocol_sha256": "<SHA-256 of docs/viewer-acceptance-independent.json>",
  "output_group": "viewer-acceptance-independent-cohort-01",
  "exclusive_measurement_window": true,
  "measurement_owner": "<fresh bounded execution context>"
}
```

Use the existing Python/GDAL environment and set `GDAL_DRIVER_PATH=disable`,
`GDAL_PAM_ENABLED=NO`, `OMP_NUM_THREADS=1`, `OPENBLAS_NUM_THREADS=1` and
`PYTHONDONTWRITEBYTECODE=1`. Keep one process watcher with the execution owner.

```sh
python3 tools/viewer-acceptance-independent.py \
  --revision <exact-committed-HEAD> --timing-grant <coordinator-grant.json>
```

The execution context returns a compact factual report (at most 1,000 words),
with immutable result paths/hashes and the matched-byte and diagnosis limitations.
No protected excerpts or raw process transcripts enter that report or Git.

## Executed cohort result

The single authorised cohort completed against exact protocol source
`78150c8a07feafd19222db6c4ad54ff2f8f90557`: 16 encoder invocations,
two expansions, no process failures and no retries. The group is consumed.
The [result record](viewer-acceptance-independent-results.json) binds immutable
store evidence, all trial hashes and paired per-cell quality metrics. No protected
samples, payloads, decoded rasters or process transcripts enter this repository.

| Product | Byte match | Source-valid quality | Separate Emuella reconstruction check |
|---|---|---|---|
| Mansfield RGB16 12 | First match: trial 8, 1,062,356 B versus 1,056,940 B; +5,416 B (+0.5124%) | 48/48 pass versus baseline 42/48; every cell's RMSE improves | Exact agreement fails in all four views |
| Tok RGB8 4 | Unmatched after eight conforming trials; final trial 1,549,494 B versus 1,573,332 B (−1.5151%) | Not decoded or scored | Not run |

RGB16's worst display RMSE is 1.733231 and worst p99 absolute error is 4 U8
units (gates 3 and 12). Its selected qstep is `2.3804078207652011E-4`.
The separate existing-Emuella-stream expansion exits successfully but emits four
stdout warning lines mentioning quantisation and tiles. View 0 has at most one
native unit disagreement; views 1–3 have large disagreements, with overall maximum
absolute error 16,034 and maximum per-band/view RMSE 12,986.876374 native units.
No causal explanation is established. Detailed raw-band errors and diagnostic
identities remain linked from the result record; process text stays in the store.

The supplied committed harness and execution instruction separate exact
reconstruction from source-quality efficiency; this result uses that eligibility
rule despite the older agreement-dependent wording retained above. RGB16 supplies
bounded evidence of recoverable display distortion at approximately the same
payload. Tok supplies no efficiency conclusion, so the two-product hypothesis is
**not supported by this bounded cohort**. No minimal quantiser/allocation policy
change is justified. This numerical experiment establishes no product, visual,
full-scene or performance acceptance. No additional encodes or full scenes ran.
