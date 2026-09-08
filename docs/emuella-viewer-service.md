# Emuella viewer source and local service

`emuella-viewer-source` is the application adapter between the codec, the
selected stateless JPP profile and a transport-owned worker. It builds for native
Rust and `wasm32-unknown-unknown`; generic Polyorama crates do not depend on it.
`emuella-viewer-tools` supplies synchronous GDAL preparation, deterministic
public fixtures, a localhost HTTP service and an actual TCP measurement client.
The viewer application owns transport, cancellation and request generations.

The codec and protocol dependencies use the merged revision
`2568f1c40c83a40f527c7ee8f1600af511e046d0`. [Composed candidate qualification](emuella-viewer-qualification.md)
records the actual NITF/native/browser journeys; this dependency pin alone does
not prove application acceptance. The supported encoded profile has one genuine quality layer;
resolution discards are not additional layers.
The codec's selected geometry permits tile edges 256/512/1024, depths 2/5/6,
unsigned precision 8–16 and one or three components, with no MCT. Requesting
`layers=2` is rejected. Rate targets and index format form part of identity.

## Preparation and fixture commands

Set `VIEWER_OUTPUT` to a registered scratch child, `CODEC_REVISION` to the exact
selected codec revision, and `GDAL_LIBRARY` to the maintained GDAL shared library.
Supply its runtime dependency directories through the ordinary loader environment
and its independently built JP2Emuella plugin through `GDAL_DRIVER_PATH`.
No machine-specific paths are compiled into the tools.

```sh
cargo run --release -p emuella-viewer-tools -- fixture \
  --output "$VIEWER_OUTPUT/scientific-u16" --target scientific-u16 \
  --width 2048 --height 2048 --bits 16 --components 1 \
  --tile 512 --levels 5 --bpp 2 --codec-revision "$CODEC_REVISION"

cargo run --release -p emuella-viewer-tools -- fixture-raster \
  --gdal-library "$GDAL_LIBRARY" --output "$VIEWER_OUTPUT/scientific.tif" \
  --width 2048 --height 2048 --bits 16 --components 1

cargo run --release -p emuella-viewer-tools -- prepare \
  --gdal-library "$GDAL_LIBRARY" --input "$VIEWER_OUTPUT/scientific.tif" \
  --output "$VIEWER_OUTPUT/prepared" --target prepared \
  --bits 16 --bands 1 --tile 512 --levels 5 --bpp 2 \
  --codec-revision "$CODEC_REVISION"
```

`fixture` generates one indexed codestream directly. `fixture-raster` writes a
native Byte or UInt16 GTiff using the same authored signal: gradients, small
bright objects, faint perturbations and straight tile-crossing edges. RGB uses
component-specific backgrounds. Both are deterministic and contain no external
imagery. Large dimensions change work and output size, not the tile memory bound.
For native RGB use `--bits 8 --components 3` or `--bits 16 --components 3`, then
prepare with the matching precision and `--bands 1,2,3`.

Preparation opens source inputs read-only and disables GDAL PAM. TIFF stays in
GTiff. The isolated preparation process deregisters the four JPEG 2000 drivers
that the maintained NITF driver tries before JP2Emuella, matching the project's
NITF integration test strategy. NITF requires a registered JP2Emuella, IC=C8 and the advertised required-source-index
capability. It selects `JP2EMUELLA_REQUIRE_SOURCE_INDEX=YES`; plugin inspection
constructs the bounded retained index during open and subsequent windows reuse it.
See the [separate NITF measurement command](emuella-viewer-nitf-preparation.md);
its outer driver, compression identifier and route policy appear in the metrics.
No third-party codec API is invoked by this adapter. The caller chooses native
precision explicitly: 11-bit NITF is UInt16 storage with `--bits 11`.

GDAL's cache is capped at 64 MiB. Source callbacks fill one tile synchronously;
the streaming encoder writes one codestream and per-tile descriptors without a
global admitted index. No whole-image pixel buffer, independent encoded chips or
stored thumbnails exist. Original source hashing is a separate bounded scan and
its bytes/time are recorded. Publication is an atomic directory rename after
successful encoding; failures remove only that invocation's incomplete directory.
Existing output directories are immutable and never overwritten.

`manifest.json` binds source digest, selected bands, native precision, dimensions,
complete profile and encoding-contract version, codec revision, spatial-policy
hash, encoded payload hash, index version and each descriptor hash. The manifest's
identity includes encoded length and main-header length. Display stretch remains
outside identity. Reopening a prepared representation never opens the original
source. `--verify-payload true` performs an explicit encoded-payload hash scan at
service startup; false trusts the immutable local store after checking its length.
Descriptor hashes are checked in both modes. Neither size nor mtime substitutes
for a content digest.

## Transport contract

```sh
cargo run --release -p emuella-viewer-tools -- serve \
  --representation "$VIEWER_OUTPUT/scientific-u16" \
  --representation "$VIEWER_OUTPUT/prepared" \
  --listen 127.0.0.1:8123 --verify-payload true --web "$VIEWER_WEB_ROOT"
```

The bind address must be loopback. The explicit representation list is the
catalogue whitelist; request target names are never filesystem paths. Static
assets must resolve inside the supplied web root, including symlink targets.

The browser application and data endpoints use the same HTTP origin. Every
HTTP/1.1 request requires exactly one `Host`: case-insensitive `localhost`, a
literal IPv4 address in `127.0.0.0/8`, or bracketed IPv6 loopback `[::1]`, with
an optional decimal port from 1 to 65535 (omission means 80). Other DNS names
are rejected without resolution, preventing DNS rebinding through a foreign
hostname. The authority port need not equal the socket port, so a transparent
loopback calibration proxy can forward its own authority.

When supplied, `Origin` must be a single HTTP origin matching that `Host` and
effective port; `null`, foreign origins, HTTPS origins and duplicate boundary
fields are rejected. Native clients and same-origin browser requests may omit
`Origin`. Supplied `Sec-Fetch-Site` must be `same-origin` or `none`; even
`same-site` requests are rejected because another local port is another origin.
Malformed request fields and boundary violations return HTTP 400 before routing
or imagery reads. Responses contain `Cross-Origin-Resource-Policy: same-origin`
and `X-Content-Type-Options: nosniff`, with no CORS permission or exposure fields.
Standard JPIP response fields remain directly available to the same-origin
application. This is a local browser boundary, without client authentication;
local native processes can access the explicitly selected representations.

| Request | Response |
|---|---|
| `GET /catalogue` | JSON array of `Manifest` |
| `GET /manifest/{target}` | JSON `Manifest` |
| `GET /descriptor/{target}/{tile}?tid={tid}` | Authenticated bounded descriptor bytes |
| `GET /jpip?{Request::query()}` | Standard `image/jpp-stream` and JPIP response fields |
| `GET /metrics` | Service-generated byte/read counters and Linux process observations |

JPP bodies contain main-header, tile-header, empty metadata completion and actual
precinct messages. Payloads are positioned reads from the single codestream;
there is no custom image-range endpoint. Responses are bounded by `len` (128 to
8 MiB, excluding EOR), 64 KiB message payloads and a 256-source-tile request cap.
The service maps the requested frame to a supported round-down resolution and
returns effective `JPIP-fsiz`, `JPIP-roff`, `JPIP-rsiz` and immutable `JPIP-tid`.
The codec planner selects required wavelet dependencies. Each scattered region
remains an independent window. The server retains no JPIP session or cache model.

The shared client API is `register`, `missing_tiles`, `install_descriptor`,
`request`, `begin_response`, `receive`, `finish`, `ready` and `decode`. A transport must
validate case-insensitive response fields, reject conflicting duplicates and
reject stale application generations before `begin_response`. An identity change
requires loading/registering the new manifest. One incremental reader belongs to
one response; discard it after any parse error or interruption. Accepted fragments
remain reusable, and the next request declares only actual contiguous prefixes.
No partial bin is asserted complete. An interrupted `finish` returns an error.
Retry while `ready` is false; when it is true, report a codec decode failure
instead of repeatedly issuing requests for already complete bins.

`DecodedRegion` contains planar native unsigned bytes: one byte per sample for
8-bit data and two little-endian bytes for 9–16-bit data. Its projected width and
height use ceiling endpoints for the original half-open source rectangle.
Transport requests round outwards; reconstruction still uses the original region.

One worker should own one `SharedClient` across all images/panes. The selected
qualification profile freezes limits at 64 MiB compressed payload and 16 MiB raw plus admitted descriptor
metadata across representations, with a 64 MiB active codec workspace ceiling.
Descriptor pressure discards admitted metadata while retaining compressed bins.
Compressed pressure evicts other representations by use recency; this coarse
policy can thrash with many active sources and is not claimed optimal. Chunk large
coarse views into at most 64-source-tile independent regions: a 43k overview has
few output pixels but thousands of source tiles. Register again after a reported
representation eviction. GPU/decoded-result budgets belong to the application.

## Evidence and limitations

```sh
cargo test -p emuella-viewer-tools --test journey
cargo clippy -p emuella-viewer-source -p emuella-viewer-tools --all-targets -- -D warnings
cargo check -p emuella-viewer-source --target wasm32-unknown-unknown
cargo run --release -p emuella-viewer-tools -- measure \
  --address 127.0.0.1:8123 --target scientific-u16 \
  --x 247 --y 117 --width 769 --height 513 --discard 2 --rounds 2 --len 262144
```

Journey tests cover actual HTTP, same-origin/native access, foreign origins and
DNS-rebinding authorities rejected before data reads, malformed/duplicate
boundary fields, fragmented partial receipt/retry after restart,
warm compressed reuse without source reads, odd non-origin and edge projection,
reference equality from the same representation, policy identity invalidation,
corrupted descriptor/payload rejection and failure-atomic preparation.

Measurement reports all received HTTP bytes, sent request bytes, descriptor/JPP
body bytes, cache occupancy, entropy-block reads/work, output checksum, latency
and process peak RSS where available. Preparation's logical callback bytes are
returned native pixels, not compressed original-source storage reads. Service
positioned payload/descriptor operations are counted separately. Linux
`read_bytes`, `rchar` and `syscr` deltas are process-wide observations, including
source hashing and library work, and are not attributed to one input by guesswork.
Service JPP byte counts describe generated bodies; the TCP client records received
bytes including retries. Physical storage traffic is unavailable without additional
system evidence. No OS cache conditioning is implied; stage labels distinguish
cold compressed state from warm compressed reconstruction and leave source-cache
state explicitly uncontrolled. GPU timing and residency are outside this package.

## Complete selected-tile reference comparison

```sh
cargo run --release -p emuella-viewer-tools -- reference \
  --representation "$VIEWER_OUTPUT/scientific-u16" --requests "$EVIDENCE_RECORDS"
```

`--requests` is a JSON array of the application's `worker.decoded_evidence`
records, partitioned by representation. Each record requires `tid`, `region`
(`x`, `y`, `width`, `height`, `discard`, `components`), `width`, `height`,
`precision` and `fnv1a64_u16le`. The immutable ID must match the supplied root's
validated manifest. The report repeats its target, ID and codec revision. Capture
records from actual application execution; a checksum generated by the reference
itself is not comparison evidence. Each record is checked, including duplicate
regions that can demonstrate warm reconstruction, with at most 1024 records per
invocation. Partition larger traces into separate invocations without silently
truncating them.

The reference bypasses HTTP and compressed caches. It authenticates and imports
one intersecting tile descriptor at a time, plans that complete clipped tile at
the requested discard/components, reads entropy bytes at absolute file offsets,
and crops the reconstructed native planes to the requested half-open region.
Output endpoints use `ceil(start / scale)..ceil(end / scale)`. Interleaving follows
component selection order; 8-bit samples are zero-extended to U16 before the
application's FNV-1a U16LE checksum. Dimensions and native precision must also
match. A failed comparison still prints the JSON report and exits unsuccessfully;
invalid identities, descriptors or reads fail closed.

Output is capped at 16 MiB per region, codec workspace at 64 MiB and selected
raw-plus-admitted descriptor metadata at 16 MiB. The reference retains only one
complete tile's planes and the requested output, never a whole-image raster or
whole codestream. The immutable local payload is trusted after a length check,
as in ordinary service reopening; selected descriptor SHA-256 hashes are checked.
Run the separate service integrity scan when qualification requires whole-payload
hash validation.

Reported file bytes/operations, complete-tile block and synthesis work, tile/output
memory bounds and elapsed time describe **reference cost**, not viewport
performance. `compared_pixels` counts pixels covered by the supplied records;
`mismatched_records` counts checksum, dimension or precision failures. Checksums
do not localise or count individual differing samples. Small authored regression
fixtures additionally compare every native sample across four formats, eight
windows and all seven D6 discard levels, including crossing and odd edge regions,
with an actual TCP/JPP client journey. This establishes application composition
and cache consistency using the shared codec. Independent reconstruction
algorithm correctness belongs to the codec's separate complete-tile oracle tests.
