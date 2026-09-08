# Regional source adapter contract

The additive `regional` modules in core, runtime and renderer provide independent
parent-image demands, bounded external worker scheduling and integer GPU display.
The Analytical Workspace Lab continues to use its existing synthetic tile path.
These contracts are a foundation; they do not qualify a real codec, transport,
large-image viewer or browser worker journey.

## Identity and demand

The source adapter assigns an immutable `RepresentationId([u8; 32])`. Its digest
must cover source content, preparation policy/version and encoding choices that
affect reconstruction. A changed policy produces a changed identity. Display
stretch, gamma and colour mapping are excluded.

`RegionKey` contains that identity, a non-empty half-open `ImageRegion` in parent
image pixels, reduction by powers of two, ordered source components and opaque
`SourceStage(u64)`. The source owns quality/precision-stage semantics, component
meaning, image bounds, reduction alignment and dependency readiness. The initial
unsigned sample payload supports one scalar or three RGB components, precision
1–16 bits, with tightly interleaved `Vec<u16>` values. Source-specific colour
transforms happen in the adapter before an RGB payload reaches the renderer.

A `RegionDemand` adds consumer identity, visible/prefetch priority and an upper
bound on decoded sample allocation bytes. `regional_grid_demands` calls the
source lookup only for a `VirtualGrid`'s materialised items. It does not enumerate
all detections or invent thumbnail assets. Exact keys deduplicate across consumers;
partially overlapping and scattered rectangles remain separate requests. Source
adapters may reuse compressed dependencies beneath those independent requests.

## Shared runtime

Create one `RegionalRuntime` for all images, detail panes and gallery cells.
Each frame, collect every consumer's desired state and call
`reconcile(generation, demands)`. Generations are monotonically non-decreasing;
older snapshots fail atomically. Identical requests retained across a generation
keep their token. Representation, rectangle, reduction, components or stage
changes create separate cache identities.

1. Forward returned cancellation tickets to the native/browser worker executor.
2. Call `dispatch` and submit its compact `RegionalRequest` values to that executor.
   Visible requests precede prefetch; coarser reductions precede finer requests
   within the same priority. The runtime performs no decoding on the UI thread.
3. The worker enforces `max_decoded_bytes` before allocating output and returns
   `RegionalPixels` to `complete`, or reports failure through `fail`. The bound
   includes spare `Vec` capacity. Codec scratch and compressed data require
   separate adapter-owned global budgets; this runtime cannot infer them.
4. A cancelled request retains its worker slot and output reservation until it
   completes or `acknowledge_cancelled` confirms the worker stopped and released
   its allocation. Signalling cancellation alone does not release capacity.
5. `take_decoded` transfers payload ownership to the renderer queue. Those bytes
   remain charged until `finish_upload` acknowledges consumption or rejection.

`metrics().accounted_decoded_bytes()` includes worker output reservations,
retained decoded allocations and transferred payloads. Repeated or superseded
tokens cannot satisfy new work. Unused decoded entries can be evicted to admit
new work; resident textures remain reusable until the renderer reports eviction.
After a recoverable failure, the adapter explicitly calls `retry` for the key.

## GPU hand-off and display

Create one `RegionalRenderer` for the shared device and target format. Configure
global texture bytes/items, total upload bytes per frame and total draws per
frame through `RegionalGpuLimits`. The upload limit bounds packed CPU scratch
and the amount submitted for staging in one frame; the application submits the
shared GPU queue each frame. Texture metrics are logical owned sample storage,
excluding driver overhead and resources retained by pending GPU work.

Call `begin_frame`, then upload results before preparing any pane:

- Validate each payload with `runtime.is_upload_current(key, token)` immediately
  before admission. Drop stale payloads and call `finish_upload(..., false)`.
- `renderer.upload` consumes an accepted payload and returns its residency plus
  exact evicted keys/tokens. Report every eviction through `evict_resident`, then
  call `finish_upload(..., true)` after the decoded payload has been consumed.
- Rejection returns the boxed payload and reason. Retain a temporary capacity
  rejection until the next frame, or drop it before acknowledging rejection.
  A request must fit the configured per-frame upload limit, texture limit and
  device dimensions; source demand construction must choose bounded regions.
- Prepare only materialised `RegionalDraw` values with `prepare`, then render
  using `paint`. Draws specify viewport-local NDC bounds, normalised UV crops and
  independent `RegionalDisplaySettings` in native sample units. Back-to-front
  draw order permits a coarse region behind finer ready regions.

Scalar textures use `R16Uint`; RGB textures use `Rgba16Uint` with an unused fourth
channel. Display stretch and gamma are shader operations and do not invalidate
compressed or decoded identity. Uploads after preparation are rejected so cached
draw bindings cannot keep evicted textures outside ownership accounting.

## Focused verification

```sh
cargo test -p polyorama-core -p polyorama-runtime -p polyorama-render-wgpu
cargo clippy -p polyorama-core -p polyorama-runtime -p polyorama-render-wgpu --all-targets -- -D warnings
cargo check -p polyorama-core -p polyorama-runtime -p polyorama-render-wgpu --target wasm32-unknown-unknown
cargo test -p polyorama-render-wgpu regional_gpu_readback -- --ignored --nocapture
```

The final command requires a native GPU adapter and is explicitly opt-in. It
reads back asymmetric 11-bit scalar and 16-bit RGB patterns, checks displayed
codes within one 8-bit code and verifies cross-image texture eviction. Select
the Vulkan loader/ICD through environment variables when the host requires it;
record software and physical GPU results separately. WASM compilation alone is
not browser execution evidence. Full repository and application qualification
remain `cargo xtask verify` and the consuming application's real worker journeys.
