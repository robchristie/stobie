# Compose a small Polyorama application

Polyorama is experimental: its public APIs are concrete and shaped by maintained
applications. Start with [Record Desk](../consumers/record-desk/README.md), a useful
independent Cargo workspace with native and browser entry points, local record
editing/history and versioned persistence. It declares its own dependencies,
profiles and lockfile and imports no existing application package or private API.
The smaller triangle example below remains a focused image-command/presenter
demonstration within the Lab package.

## Compose a non-image domain

Record Desk needs only `polyorama-core` and `polyorama-ui-egui`. It disables UI's
default-enabled `image-rendering` feature, removing Polyorama's scalar renderer
and worker runtime from the resolved graph. Eframe's host GPU backend remains
an ordinary consumer dependency. Existing image applications retain the feature
and its public APIs by default.

The consumer owns its `Record`, stable IDs, validation, selected identity,
transient editing draft and transaction history. These are its own domain
document/session equivalents. Framework `Document`, `Session`, `ImageIntent`
and `CommandHistory` describe the analytical image domain and are not mandatory
containers for unrelated records. Do not encode records as annotations.

`dock_workspace_layout` presents the single core `Workspace` and emits a
`WorkspaceResize` for a completed resize. Apply it after presentation; its public
`apply` rejects stale/missing splits and invalid fractions without mutation.
Record history remains application-owned. The existing `dock_workspace` wrapper
returns its compatible `Command::ResizeSplit` for image-history consumers. Tab
activation and movement mutate the canonical workspace directly in both routes;
dirty tracking/persistence must observe the complete tree. `Workspace::validate`
checks structural tab/split invariants as well as stable identities and schema;
applications also validate their allowed pane set when restoring layouts.

Read Record Desk's `model.rs`, `panes.rs`, `app.rs`, then `store.rs`: pane views
emit intents; the application handles current-pass draft/filter outputs before
actions; Apply validates one record transaction; Save persists only committed
records and layout. Its README gives exact standalone commands, keyboard routes,
field/action extension steps and protected malformed-state recovery. Its
[qualification evidence](record-desk-evidence.md) distinguishes direct tests,
physical native/browser input and a bounded fresh-context authoring probe.

`choice_control_with_options` adds current popup option observations to the
compatible `choice_control` recipe. Merge its option nodes into the viewport
snapshot; their floating layer can extend beyond the pane. Option IDs derive
from a stable value representation rather than labels or positions, so physical
inspection can find the currently offered choice without guessing coordinates.
Native text editors use the public `TextInput`/`MultilineTextInput` semantic roles
and `NativeTextControlKind::TextEdit`; coverage explicitly excludes egui's internal
editable text layout. These observations do not supply a browser accessibility
adapter or application command injection.

## Choose the owners

| Owner | Application-facing entry points | Responsibility |
|---|---|---|
| `polyorama-core` | `Document`, `Session`, `Workspace`, `ImageIntent`, `validate_intent`, `CommandHistory`, `TileDemand` | Durable annotations, transient view/selection/gesture state, one canonical dock tree, validated transitions and renderer-independent desired work |
| `polyorama-runtime` | `Runtime`, `RuntimeConfig`, `DecodeRequest`, `DecodeEvent`, `RequestToken` | Reconcile complete desired sets, bound CPU work and validate completion identity; independent of egui/wgpu |
| `polyorama-render-wgpu` | `RenderBridge`, `ScalarRenderer`, `ImageRenderRequest`, `RenderPlan` | Persistent GPU resources, bounded uploads, residency and typed image presentation |
| `polyorama-ui-egui` | `PanePresenter`, `dock_workspace`, `PresentationContext`, `ActionKey` | egui layout/interaction, narrow pane views and output sinks, typed capabilities and current-pass observations |

Keep focus and hover in UI state. Durable annotations belong in `Document`;
selection, cameras, tools and gesture preview belong in `Session`. Although
session/workspace serialisation is supported, those owners remain distinct.
The defaults are the analytical demo's starting state. A vector-only pane can
use an empty camera/tool mapping; `Session::validate_image_cameras` specifically
checks the Lab's image panes 1–4 and is not a generic pane-registry validator.
Record Desk avoids these image-specific types entirely; compatibility defaults
and the image camera contract remain unchanged.

Use the core `Workspace` as the sole dock tree. `DockBehaviour` retains only
drag/split previews, not a second layout. The host owns its GPU device/queue;
`ScalarRenderer` owns persistent image GPU resources across panes. A pane may
emit demands or render requests but must not allocate its own device/queue.
The minimal vector example below uses egui's painter and therefore needs no
asynchronous tile runtime or scalar renderer instance.

## Build, run and observe a complete action

Run from the repository root with the native prerequisites in the
[README](../README.md#prerequisites): a graphical session and a working wgpu
backend. The maintained source is
[`minimal-workspace.rs`](../apps/analytical-workspace-lab/examples/minimal-workspace.rs).

```sh
cargo run --release -p analytical-workspace-lab --example minimal-workspace
```

The window contains one **Triangles** tab, **Add triangle**, a committed count
and an initially empty canvas. Click **Add triangle** once: the count becomes
1 and a blue triangle appears. Click again: the count becomes 2 and a second,
offset triangle appears. Each click creates exactly one command/history entry.
Closing the window ends the app. This small example does not save its document.

Read the source in this order:

1. `MinimalApp::new` constructs `Document`, a camera-free `Session`,
   `CommandHistory` and a validated single-tab `Workspace`.
2. `Action` implements `ActionKey`; its stable ID and `ActionSpec` describe one
   pane-scoped capability. `ActionTarget::pane` binds that capability to the
   stable `PaneId`. The button emits `ImageIntent::CommitPolygon` into a sink.
3. `TrianglePane` implements `PanePresenter` and borrows only the document,
   intent sink, presentation output and resolved appearance. It paints committed
   geometry and never receives the whole mutable model/runtime.
4. `MinimalApp::ui` calls `dock_workspace`, resolves intents against current
   state with `validate_intent`, executes resulting commands through history,
   then requests repaint because a command changed state after presentation.
   The next frame displays the committed polygon. The example has no unfinished
   gesture; applications with gestures also display session previews that frame.

`validate_intent` is variant-specific validation. Polygon validation requires
at least three vertices and reserves an annotation ID before execution; it is
not a general geometry/layer validator. `CommandHistory::execute` applies a
trusted command without revalidation, so deliver it before unrelated mutations
can invalidate its snapshots. In a larger app, show validation failures as
observable status instead of relying on the example's fixed-input `expect`.
Tab activation/movement update the canonical workspace inside `dock_workspace`;
a completed split resize returns a command for the caller to execute.

The portable example source also has an explicit compile check:

```sh
cargo check --target wasm32-unknown-unknown -p analytical-workspace-lab --example minimal-workspace
```

Its WASM `main` supplies no browser bootstrap, canvas, worker transport or web
package. This check establishes source compatibility only. For the existing
browser applications and their runtime requirements, use the
[browser build/startup guide](browser-startup.md), not this example as a browser app.

## Make a small presenter variation

Inside `TrianglePane::pane_ui`, replace the comment
`// Public variation point: add a measured status badge here.` with the call
`presentation.badge(ui, "state", "Document annotation", polyorama_ui_egui::StatusTone::Success);`.
Run the same native command. The pane now shows the **Document annotation**
badge beneath the committed count; both Add actions still change the count and
canvas. This uses an existing public presentation method, without modifying
the dock or application mutation route.

The `"state"` key describes a stable logical control; do not derive it from
the count or visible text. `PresentationScope` combines that key with stable
pane/domain identity. The context owns only resolved appearance and observations;
the caller still owns layout, state, intents and repaint scheduling. Construct
it in each UI pass and call `finish(ui)` before leaving that pass. Using it in
another context, viewport or pass panics. On repeated layout passes, replace the
latest per-viewport publication rather than appending stale observations. When
combining contexts, recompute text coverage over merged layouts instead of
summing shared viewport-pass inventory counts.

This badge variation reads no new state and needs no invalidation or timed
repaint. A variation that changes model data must emit an intent, validate/apply
it at the application boundary and request repaint for that change. Use
[pane guidance](ui-guides/panes.md) for responsive/local-scroll responsibilities
and [interaction guidance](ui-guides/interactions.md) for action availability,
shortcuts and completed gestures. `PanePresenter` and `PresentationContext`
rustdoc retain these local ownership and pass-lifetime contracts.

## Add asynchronous scalar images when needed

The minimal consumer deliberately uses no tile pipeline. For a scalar-image
application, compose the existing public lifecycle below. The
[regional adapter contract](regional-adapter-contract.md) remains authoritative
for external regional sources; the built-in `Runtime` worker prepares synthetic
tiles and is not an arbitrary image decoder.

1. Construct `Runtime::try_new(RuntimeConfig::default())` and `RenderBridge` at
   the application boundary. On native, install a `Send + Sync` repaint waker:
   it runs on the worker thread and must wake the UI rather than mutate model
   state. WASM construction creates no worker; transport requests from
   `take_external_request` to your worker and return exact key/token events to
   `accept_event` on the application thread. Taking a browser request consumes
   a credit; its completion/failure returns it. Report worker-start/transport
   failure using the public runtime methods rather than leaving work waiting.
2. Derive the **complete** current `TileDemand` set from visible panes. Supply
   `runtime.generation()` and typed coordinates; reconcile once after collecting
   all pane outputs. Overlapping/repeated demands deduplicate. Submit `[]` when
   nothing is desired. Demand withdrawal/epoch changes can make completions
   obsolete; retained keys can keep their original tokens across epochs.
3. At the start of an application update, `poll` native completions (WASM `poll`
   returns zero) or accept external events. The runtime validates full key/token,
   desired state and completion state before decoded hand-off. Move events from
   `take_decoded_for_renderer` into `bridge.push`. On `UploadAdmission::Rejected`,
   keep the returned event for retry; for terminal upload failure call
   `mark_handoff_failed`. Dropping a handed-off event loses the completion path.
4. Drain `bridge.take_evicted()` and `take_resident()` once, forwarding original
   tokens to runtime `mark_evicted`/`mark_resident`. Decoding and bridge admission
   do not establish residency. `runtime.invalidate()` advances source generation
   and clears tracked work; old in-flight events can still arrive and are rejected.
   Supply the new generation to the renderer to clear its stale resources too.
5. Install `ScalarRenderer::new` once in the host's egui-wgpu callback resources,
   using the host device/target format and matching bridge. Stage
   `stage_renderer_maintenance` before pane presentation, even with no image
   pane visible, using one application frame number/current source generation.
6. Use `allocate_viewport` to obtain logical/physical geometry and local pointer
   coordinates. Build `ImageRenderRequest` with stable pane/source IDs, current
   generation, camera, display settings and desired tile keys. Display changes
   do not change decoded identity. Stage each `stage_render_callback`, collect
   its `ImagePlanTarget` and build a complete `RenderPlan` in the same order.
   Publish with `submit_render_plan` before GPU callback preparation begins.
   Counts and pane identities must match and panes must be unique; failed
   publication disables staged callbacks instead of painting mismatched data.
7. Apply validated outputs and record repaint reasons: command/state change,
   active interaction/preview, worker completion, pending upload or scheduled
   work. Retry backpressure while work is pending. Stop repainting when idle;
   never add an unconditional repaint loop.

The application update owns mutable model/runtime state; workers exchange
owned messages and renderer callbacks own GPU mutation. `RenderBridge` clones
share synchronised hand-off/acknowledgement queues. These are public composition
obligations; the Lab's private pane orchestration is an example of them, not an
additional API contract.

## Generate and check the API documentation

```sh
cargo xtask docs
```

This generates the four framework crate entries and key item pages in
`target/doc/`, executes their selected rustdoc examples, checks the maintained
consumer natively and for WASM, and checks the selected guide/README links.
Open `target/doc/polyorama_core/index.html`, then the runtime, render-wgpu and
ui-egui entries from rustdoc's crate navigation. Important local contracts are
on `validate_intent`, `Runtime::reconcile`, `ImageRenderRequest`,
`PanePresenter` and `PresentationContext`.

For separate generation or execution during editing:

```sh
cargo doc --no-deps -p polyorama-core -p polyorama-runtime -p polyorama-render-wgpu -p polyorama-ui-egui
cargo test --doc -p polyorama-core -p polyorama-runtime -p polyorama-render-wgpu -p polyorama-ui-egui
```

The full `cargo xtask verify` includes this documentation route and, on Linux,
the actual example's physical native smoke. Exact exercised environments,
rendered-page inspection and consumer qualification are recorded in the
[documentation evidence](api-documentation-evidence.md). Generated HTML stays
in ignored output; the repository makes no hosted rustdoc availability claim.
