# Emuella image viewer

This application composes the Emuella regional source with Polyorama's shared
runtime and integer GPU renderer. The existing Analytical Workspace Lab and
component Gallery retain their separate synthetic demonstration paths.

One native background thread or one real browser Web Worker owns the shared
compressed source cache. The main thread owns desired state and display;
codec reconstruction happens exclusively in the executor. Primary, linked
or comparison, detection detail and visible thumbnail regions share immutable
parent representations. **Compare image** opens the next catalogue image beside
the primary image. Bookmarks retain an image and camera; repeated **Recall**
cycles through stored bookmarks. There are at most 16 bookmarks.

The deterministic 10,000 detections alternate clustered and scattered parent
coordinates. Metadata is computed on demand. The grid creates only visible
widgets and requests two rows of overscan. No independent thumbnail/chip files
are encoded. Opening a detection requests full-resolution parent data. Coarse
regions remain behind detail regions while their additional dependencies arrive.
Both panes preserve image aspect ratio; thumbnail paint is clipped to the grid.

## Run

Prepare public deterministic representations and start `emuella-viewer-tools`
using its component documentation. Supply more than one representation for the
comparison and bookmark workload. Native startup is:

```sh
cargo run --release -p emuella-viewer -- --server http://127.0.0.1:8088
```

Build a complete static root in registered campaign scratch and serve that exact
root through `emuella-viewer-tools serve --web`:

```sh
POLYORAMA_VIEWER_WEB_DIR="$CAMPAIGN_SCRATCH/viewer-web" cargo xtask build-viewer-web
```

The static root contains local HTML, JavaScript and WASM, with no CDN assets.
Browser requests use the same origin. `worker.js` loads its own WASM instance;
its exported `WorkerClient` is never instantiated by the canvas application.
The source crate owns identities, descriptors, JPIP queries/messages and cache
semantics. Application transports only parse HTTP headers and move bytes.

## Budgets and cancellation

Default global limits are compressed data 64 MiB, admitted descriptor/index
metadata 16 MiB, decoded reservations 16 MiB and GPU textures 64 MiB. One in-flight
regional request bounds concurrent codec workspace, whose independent source
limit is 64 MiB. Regional output reservations cover planar codec output plus
interleaved U16 output. Each region spans at most 8 × 8 source tiles and at most
512 × 512 output samples. A 43,008-square overview at discard 6 is 121 bounded
regions, rather than one request admitting all 7,056 source tiles.

Native `--compressed-mib`, `--decoded-mib` and `--gpu-mib` override those three
budgets. Browser fragment parameters `#compressed_mib=1&decoded_mib=4&gpu_mib=16`
provide the same controls. Requested regions must fit their configured output
and GPU upload limits; an impossible configuration reports an error.

JPIP response requests are capped at 256 KiB. Transports additionally reject
bodies over 8 MiB and catalogues over 16 MiB or 32 representations. Retries are
bounded at 64 responses and occur only while dependencies are incomplete;
codec errors are surfaced directly. Native cancellation checks between HTTP
chunks and before/after decoding; the synchronous HTTP operation has a 15-second
timeout. Browser sample payloads use transferred `Uint16Array` buffers and one exact-sized
WASM copy, keeping transport samples inside the decoded reservation. Browser
cancellation aborts fetch. A yield after synchronous browser
WASM work lets queued cancellation run before decoded publication. Reservations
and worker slots remain charged until the worker acknowledges stopping or its
result is rejected as stale. Display stretch/gamma changes do not change keys.

## Reproducible workload

```sh
cargo run --release -p emuella-viewer -- \
  --server http://127.0.0.1:8088 \
  --script-output "$CAMPAIGN_SCRATCH/viewer-native.json"

EMUELLA_VIEWER_URL=http://127.0.0.1:8088 \
POLYORAMA_EVIDENCE_DIR="$CAMPAIGN_SCRATCH/viewer-browser" \
cargo xtask viewer-smoke
```

The browser smoke defaults to the repository's software WebGPU launch route.
Set `EMUELLA_BROWSER_GPU=hardware` for the explicit Vulkan route on a configured
host. Both the requested route and observed adapter must be retained; software
results do not establish target GPU performance. The current host's software
route lost its device, while its explicit NVIDIA route completed the workload.
That observation is provisional environment evidence, not a portable support claim.

The trace switches images, pans/zooms, scrolls widely separated detections,
opens details, changes stretch, revisits bookmarks and displays distinct images
simultaneously. A separate fresh browser client uses 1 MiB compressed, 4 MiB
decoded and 16 MiB GPU limits. The harness delays an actual JPP transfer and changes
image to exercise cancellation acknowledgement, then revisits multiple images.
It writes bounded snapshots, network requests, opened-image candidates and
resource/work counters. Native writes final metrics plus a `.stages.json` trace.
Each worker retains up to 64 recent decoded-region records with exact immutable
identity, parent region, precision, dimensions and FNV-1a U16LE checksum. These
allow comparison before display; FNV is diagnostic, not a source identity hash.

Focused checks are `cargo test -p emuella-viewer --lib`, native/WASM clippy and
`cargo xtask build-viewer-web`. Full repository qualification remains
`cargo xtask verify`. Deterministic public fixture journeys do not establish
protected real-image qualification, codec quality/tolerances, every browser/GPU,
assistive-technology support or large-image performance thresholds. Those
require the system campaign's exact owner revisions and representative evidence.

For the representative 43,008-wide, nine-image native/browser workload, immutable
benchmark trace export, baseline selection and recovery evidence, use the
[composed calibration guide](../../docs/emuella-viewer-calibration.md). Native
scripted phases now require all distinct demands to become GPU resident and fail
after 60 seconds; they no longer advance after an unsettled timeout.

The [public candidate qualification](../../docs/emuella-viewer-qualification.md)
records measured limits, exact build/input identities, complete-tile reference
comparisons and the remaining representative real-image proof. It separates
public candidate evidence from final system acceptance.
