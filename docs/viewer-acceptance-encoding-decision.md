# Bounded encoding decision

The unchanged 512-tile/D6 encoding configuration is rejected for viewer
acceptance. The [five quality screens](viewer-acceptance-quality-results.json)
retain RGB16 failures at all three declared rates and the RGB8 regression failure.
PAN16 4 passes its screen. No encoding-policy change is selected: the sole
[independent comparison](viewer-acceptance-independent-results.json) identifies
RGB16 quality headroom at closely matched bytes, but does not isolate a minimal
repair that preserves the required quality, byte, streaming and failure contracts.

The matched independent RGB16 stream is 1,062,356 bytes against the Emuella
screen's 1,056,940 bytes, inside the comparison's declared 1% tolerance. It is
nevertheless above the Emuella candidate payload ceiling of 1,057,400 bytes.
Its 48/48 passing display cells therefore do not establish an eligible replacement.
The RGB8 comparison exhausted eight format-conforming trials without a byte
match; no independent RGB8 trial was selected, decoded or scored.

## Independent reconstruction limitation

The installed OpenJPH decoder's retained warnings report unsupported QCD markers
in tile headers. Emuella intentionally emits tile-dependent quantisation, retains
local headers in its descriptors, and derives local quantisers during import.
The first view lies in tile zero, whose quantisation agrees with the main header;
it differs by at most one stored code. Other views intersect differing tile
quantisers and exhibit much larger errors. The missing decoder capability is
observed; attributing the entire numerical pattern to a particular internal
fallback remains an inference without external implementation-source inspection.

This observation cannot establish an Emuella reconstruction defect or general
conformance. The historical 36 Kakadu invocations retain their original bounded
agreement within one stored code; they do not become exact or qualify this new
stream. Retained warning/process records remain in the approved store under
`viewer-acceptance-independent-cohort-01/rgb16-12/emuella-decode.*`.

Codec owner references at `6586e3d50f95429b242cb2e3535742b002784f2d` are
`crates/emuella-j2k-codestream/src/ht_indexed.rs` (`encode_tiled_internal`),
`ht_indexed/persistence.rs` (local canonical descriptor import) and
`ht_lossy.rs` (`search_tile_levels`). No codec or standards source was changed;
no external codec implementation source was consulted.

## Unproved cause and next action

Per-tile target allocation is an identifiable mechanism. Component packet bytes
and final QCD values do not measure marginal distortion gained per transferred
byte. A different encoder changes more than that allocation, and no measured
MCT attribution exists. Borrowing tile budgets or globally searching a quantiser
would require an explicit new owner contract and resource proof; it cannot
silently replace the current per-tile API promise.

The next separate work package is one predeclared owner-authored allocation
hypothesis, falsified on RGB16 12, PAN16 4 and RGB8 4 under unchanged quality,
actual-byte, resource, descriptor and failure gates. This campaign does not open
another sweep or retune reserved validation. Full-scene masks, native/browser
inspection and resource/completion diagnostics remain required campaign evidence;
this encoding decision alone is not terminal viewer acceptance or closeout.
