# 1. Product intent

Build the first application-shaped vertical slice of a Rust framework for **GPU-driven analytical and scientific workspaces on native and web**.

The framework is deliberately a layer above:

* `egui` as the immediate-mode presentation language;
* `wgpu` as the shared graphics and compute API;
* `egui_wgpu` as the initial integration path;
* and an existing egui docking crate where practical.

Do **not** begin by building a new general-purpose GUI core.

The purpose of this vertical slice is to test whether a specialised framework above egui can cleanly support:

* multiple independently rendered but linkable GPU viewports;
* large sparse and progressively available image data;
* scientific scalar pixel formats and custom shaders;
* client-side tile demand, decoding, caching and upload;
* typed coordinate transformations;
* interactive vector editing;
* very large virtualised result collections;
* a desktop-like dock, pane, tool, focus and selection model;
* native and browser deployment from the same Rust application model;
* and agent-friendly architectural constraints.

The slice should feel like a small but real analytical application, not a component gallery and not a collection of disconnected technical demonstrations.

---

# 2. Desired end state

Produce a runnable demonstration application, provisionally named **Analytical Workspace Lab**, that works:

1. as a native desktop application on the development host; and
2. as a browser application compiled to WebAssembly.

The application must contain:

* a canonical, serialisable dock workspace;
* four separately rendered GPU image viewports;
* progressive rendering of a large virtual tiled scalar raster;
* at least two cameras that can be linked and unlinked;
* a custom shader for displaying non-RGBA scientific pixels;
* editable vector annotations with live interaction preview;
* undo and redo;
* a result table with at least 1,000,000 logical rows;
* a two-dimensional virtualised thumbnail gallery;
* result and selection interactions linked to the image views;
* layout persistence and restoration;
* event-driven rather than unconditional repainting;
* and a detailed live diagnostics surface.

The implementation must demonstrate the architecture through working behaviour. Scaffolding, type definitions, documentation or isolated prototypes are not sufficient by themselves.

---

# 3. Core architectural contract

The central design principle is:

> **Retained behaviour and resources, immediate presentation, explicit state transitions, and an explicit GPU render plan.**

## 3.1 State ownership

Maintain clear ownership for at least these categories of state:

| State category  | Examples                                                               | Required owner            |
| --------------- | ---------------------------------------------------------------------- | ------------------------- |
| Document state  | Annotations, logical layers, saved analytical content                  | Domain/application model  |
| Workspace state | Dock tree, pane instances, pane configuration, active pane             | Canonical workspace model |
| Session state   | Selection, camera link groups, active tools, edit transactions         | Application session model |
| UI memory       | Focus, hover, open menus, drag sessions, measured geometry             | UI behaviour layer        |
| GPU state       | Textures, pipelines, buffers, bind groups, residency and upload state  | Renderer                  |
| Runtime state   | Demands, in-flight work, cancellation, worker generations and failures | Runtime                   |

Do not store authoritative document, workspace or selection state solely inside egui widget memory.

Do not force ephemeral focus, hover, drag or text-entry state into the domain model.

Do not deep-clone the complete application state each frame merely to create a read snapshot. Read views may borrow state or contain inexpensive versioned handles.

## 3.2 Immediate-mode presentation

Treat egui as a view language rather than the application architecture.

Pane UI functions should receive:

* a narrow immutable read projection;
* a feature-scoped intent sink;
* a demand sink;
* a typed render sink or viewport frame;
* and narrowly scoped UI behaviour state where required.

A representative shape is:

```rust
fn image_pane_ui(
    ui: &mut egui::Ui,
    view: ImagePaneView<'_>,
    frame: &mut PaneFrame<'_, ImageIntent>,
) {
    // Layout, presentation and interaction only.
}
```

Pane functions must not receive arbitrary mutable access to:

```rust
&mut AppModel
&mut Runtime
&mut Workspace
&wgpu::Device
&wgpu::Queue
```

The exact API may differ, but the mechanical restriction must remain.

## 3.3 Explicit frame outputs

Keep these concepts distinct:

### Intent

An `Intent` records what the user asked to do.

Examples:

```rust
enum ImageIntent {
    SetActiveTool {
        pane: PaneId,
        tool: ToolId,
    },
    SetCameraLink {
        pane: PaneId,
        group: Option<LinkGroupId>,
    },
    CommitPolygon {
        layer: LayerId,
        vertices: Vec<WorldPoint>,
    },
    SelectResult {
        result: ResultId,
    },
}
```

### Command

A `Command` is a validated application or document mutation. Commands that alter durable state should support undo and redo where appropriate.

### Effect

An `Effect` describes asynchronous work initiated by a command, such as persistence or a long-running calculation.

### Event

An `Event` reports completion, progress or failure from the runtime.

### Demand

A `Demand` declares data or resources required to represent the current frame.

Examples include:

* image tiles;
* thumbnail ranges;
* result ranges;
* derived raster products;
* histogram inputs.

A demand is idempotent desired state, not an instruction to start a duplicate request every frame.

### Render request

A render request is a compact typed description of GPU work required for one viewport. Pane code should submit render descriptions rather than directly owning pipelines or manipulating persistent GPU resources.

### Repaint request

A repaint request records why another frame is required, such as:

* active pointer interaction;
* newly decoded data;
* completed GPU readback;
* an animation;
* a scheduled timeout;
* or an external event.

A conceptual frame result may resemble:

```rust
struct FrameOutput {
    intents: IntentBuffer,
    demands: DemandSet,
    render_plan: RenderPlan,
    repaint: RepaintRequest,
}
```

The names and exact ownership may change. The semantic separation must not.

## 3.4 Interaction preview versus durable commands

High-frequency interactions must not feel one frame behind merely because durable commands are applied after UI construction.

For camera movement, polygon construction, vertex dragging and splitter movement:

1. maintain a frame-local or session-local preview;
2. render the current preview during the same frame;
3. update transient interaction state while the gesture continues;
4. emit one coalesced durable command when the gesture ends;
5. create one logical undo record rather than one record per pointer event.

## 3.5 Canonical workspace state

There must be exactly one canonical dock-tree representation.

The implementation may:

* wrap an existing docking crate’s tree and make that wrapped form canonical;
* use a custom canonical tree rendered directly through egui;
* or extract the necessary behaviour behind a narrow adapter.

It must not maintain a persistent workspace tree and a second stateful docking tree that require continual bidirectional synchronisation.

The canonical workspace model must support:

* stable pane IDs;
* horizontal and vertical splits;
* tab stacks;
* resizing;
* drag-and-drop rearrangement;
* active-pane tracking;
* pane creation and closure where supported;
* versioned serialisation;
* and deterministic restoration.

## 3.6 One GPU resource universe

Use one shared wgpu device and queue for:

* egui;
* analytical viewport rendering;
* texture uploads;
* vector overlays;
* derived rendering;
* picking or readback where implemented;
* and offscreen resources.

Do not create a separate wgpu device for each viewport.

Do not build the architecture around importing externally owned textures into the GUI. The normal path is that egui and analytical rendering participate in the same wgpu device and frame.

The application may initially acquire the device and queue through eframe’s wgpu render state. Hide the `egui_wgpu` callback mechanism behind the framework integration layer so ordinary pane code does not depend on callback traits or renderer internals.

## 3.7 Typed render plan

A pane should allocate a viewport slot and submit a typed render request. For example:

```rust
let viewport = frame.allocate_viewport(ui, view.pane_id());

frame.render.submit(ImageRenderRequest {
    viewport: viewport.physical_viewport(),
    clip_rect: viewport.physical_clip_rect(),
    camera: preview_camera,
    source: view.source_id(),
    layers: view.visible_layers(),
    display: view.display_settings(),
});
```

A viewport allocation must account for:

* logical coordinates;
* physical pixel coordinates;
* device scale factor;
* clipping;
* focus;
* pointer capture;
* viewport-local pointer coordinates;
* and stable pane identity.

Long-lived textures, pipelines, buffers and bind groups belong to renderer-owned resource structures rather than pane UI objects.

## 3.8 Stable semantic identity

Stable widget and interaction IDs must derive from semantic identity rather than incidental call order or source position.

The framework integration should automatically scope identities using a hierarchy similar to:

```text
WindowId
  └── PaneId
       └── FeatureId
            └── DomainEntityId
```

Do not require every feature implementation to remember ad hoc `push_id` calls.

Pane rearrangement or unrelated refactoring must not silently change the identity of a text edit, drag handle, selected row or editing transaction.

## 3.9 Typed coordinate spaces

Use newtypes or equivalent strong types for coordinate spaces, including at least:

* logical UI points;
* physical pixel points;
* viewport-local points;
* image pixel points;
* world points;
* and camera transforms.

Do not pass ambiguous untyped `(f32, f32)` values across domain, UI and renderer boundaries.

A full geospatial CRS system is not required for this slice. A deterministic image-to-world affine transform is sufficient.

## 3.10 Agent-friendly Rust

Prefer:

* concrete types;
* small focused modules;
* exhaustive enums;
* feature-scoped intent types;
* explicit dependency directions;
* nearby examples;
* and compiler-visible architectural boundaries.

Avoid:

* one enormous application action enum passed to every feature;
* deeply nested generic framework machinery;
* global mutable state;
* arbitrary `Rc<RefCell<AppModel>>` access;
* hidden callback registration;
* and an application-sized `update()` function.

No single pane or top-level update function should become the owner of unrelated application behaviour. As a guideline, split a function before it requires broad model access or becomes difficult to understand without loading several unrelated features into context.

---

# 4. Preferred repository shape

Inspect the existing repository before changing its structure. Adapt the following shape where appropriate rather than reorganising working code mechanically:

```text
crates/
  polyorama-core/
  polyorama-runtime/
  polyorama-render-wgpu/
  polyorama-ui-egui/

apps/
  analytical-workspace-lab/

xtask/
docs/
```

## `polyorama-core`

Owns:

* typed IDs;
* coordinate newtypes;
* document and session state;
* canonical workspace model;
* intents and commands;
* reducers;
* undo and redo;
* serialisation schemas;
* renderer-independent demand descriptions where appropriate.

It must not depend on:

* `egui`;
* `eframe`;
* `egui_wgpu`;
* `wgpu`;
* `web-sys`;
* or platform windowing crates.

## `polyorama-runtime`

Owns:

* demand reconciliation;
* prioritisation;
* in-flight request tracking;
* cancellation and stale-generation rejection;
* native worker execution;
* browser-worker execution;
* completion events;
* runtime metrics.

It must not depend on egui or direct UI concepts.

## `polyorama-render-wgpu`

Owns:

* shared renderer resources;
* texture and buffer residency;
* pipelines and bind groups;
* upload scheduling;
* render requests;
* viewport render execution;
* renderer capability information;
* renderer metrics.

It should depend on wgpu but not on egui.

## `polyorama-ui-egui`

Owns:

* egui pane presentation;
* dock-workspace presentation;
* viewport allocation;
* input translation;
* semantic ID scoping;
* UI behaviour state;
* the bridge from typed render requests to `egui_wgpu`;
* application-shell components required by the demo.

This should be the only framework crate that directly understands egui.

## `analytical-workspace-lab`

Owns:

* composition of the framework layers;
* the deterministic synthetic data source;
* demo-specific pane definitions;
* demo-specific application state;
* default layout;
* and the runnable native and browser entry points.

## `xtask` or equivalent

Provide one reproducible verification entry point, preferably:

```text
cargo xtask verify
```

An equivalent documented script is acceptable when an `xtask` crate would add unnecessary complexity.

Every framework abstraction introduced during this slice must be exercised by the demonstration application or by a focused test. Do not create speculative extension systems for hypothetical future users.

---

# 5. Demonstration data

Use deterministic synthetic data so the repository is self-contained and does not depend on external services, private imagery or large binary assets.

## 5.1 Virtual scalar raster

Create a virtual raster with:

* logical dimensions of at least `131072 × 131072` pixels;
* scalar pixels rather than pre-coloured RGBA pixels;
* a tile size such as `256 × 256`;
* a multiresolution pyramid;
* deterministic content derived from a fixed seed and tile coordinates;
* visible structures that make pan, zoom, overview and derived rendering easy to inspect.

Suitable synthetic content may combine:

* gradients;
* noise;
* sharp edges;
* repeated geometric objects;
* sparse bright targets;
* and larger low-frequency structures.

The complete raster must never be allocated in memory.

At least one data path must deliver compressed tile bytes that are decoded away from the UI/GPU thread. Fixture payloads may be generated deterministically at build time or on demand, provided that the runtime actually transports and decodes a compressed representation.

## 5.2 Logical result collection

Expose at least `1,000,000` deterministic logical result rows without allocating one million full row structures.

A result should have a stable `ResultId` and fields such as:

* result ID;
* image/world position;
* confidence score;
* category;
* thumbnail key;
* and selected state.

The result provider may calculate records from index and seed on demand.

## 5.3 Logical thumbnail collection

Expose a two-dimensional gallery containing at least `100,000` logical thumbnail items.

Thumbnail data must be progressively requested through the same broad demand/reconciliation system as image tiles. The gallery must not instantiate or request all thumbnails.

---

# 6. Default application workspace

The initial layout should resemble an analytical desktop application rather than a webpage.

It should include at least these panes:

## Four GPU viewport panes

1. **Primary View**
   Main scalar-image view with pan, zoom, display controls and vector overlays.

2. **Linked Detail View**
   A second full image view that can be linked to or independent from the primary camera.

3. **Overview View**
   A low-resolution overview or minimap that shows one or more viewport footprints.

4. **Derived View**
   A view of the same source using a meaningfully different GPU display operation, such as thresholding, edge emphasis or an alternate scalar mapping.

## Additional analytical panes

5. **Results**
   A virtualised million-row result table.

6. **Thumbnails**
   A virtualised two-dimensional thumbnail gallery.

7. **Inspector**
   Details for the current selection and active pane.

8. **Diagnostics**
   Live instrumentation and exportable metric snapshots.

These panes may share tab stacks in the default layout. All must participate in the same canonical dock workspace and be rearrangeable.

---

# 7. Functional requirements

## 7.1 Docking and layout

The user must be able to:

* resize split regions;
* move panes between tab stacks;
* create horizontal and vertical arrangements;
* activate panes;
* close and restore optional panes where supported;
* reset to the default layout;
* save the current layout;
* reload the application and restore the saved layout.

Pane IDs must remain stable across layout restoration.

Layout serialisation must have an explicit schema version and a tested round trip.

## 7.2 Scientific GPU rendering

At least one viewport must display a non-RGBA GPU texture format, such as a single-channel integer or floating-point format selected according to available capabilities.

The display path must:

* retain scalar source values;
* apply display mapping in a custom shader;
* expose controls such as window/level, threshold or colormap selection;
* and avoid satisfying the requirement by converting every source tile into an RGBA image on the CPU.

If a particular scalar texture format is unavailable on a target backend, select another appropriate scalar format or expose a documented reduced capability profile. Do not silently remove the scientific-pixel path.

Each GPU viewport must:

* receive a correct physical viewport and scissor region;
* render only inside its allocated pane;
* react correctly to resizing and device-scale changes;
* and share renderer resources where appropriate.

## 7.3 Camera behaviour

Each image view must support:

* pan;
* zoom around the pointer position;
* fit-to-data;
* and display of current image/world coordinates.

At least two views must support:

* joining a camera link group;
* leaving the group;
* propagating camera changes through typed intents or commands;
* and rendering the linked update without visible one-frame lag.

The overview view must show the current footprint of at least the primary view and permit an interaction that recentres a linked view.

## 7.4 Progressive tile demand

Each visible viewport must derive a desired tile set from:

* camera position;
* viewport size;
* image pyramid;
* and a bounded prefetch margin.

The demand system must:

* merge duplicate `TileKey` demands from multiple views;
* distinguish visible and prefetch priority;
* avoid scheduling duplicate decodes for the same key;
* recognise already decoded or GPU-resident resources;
* reject stale completions after invalidation or generation changes;
* expose failed states without repeatedly retrying every frame;
* and stop demanding high-resolution tiles for hidden panes.

Visible low-resolution coverage should appear before all fine-resolution tiles are available.

The UI must display an intentional placeholder or coarser level while fine data is pending.

## 7.5 Worker boundary

Implement a common request/event protocol for native and browser execution.

Native work should execute on background threads.

Browser tile decoding must execute in at least one actual Web Worker or equivalent independent worker context. Shared-memory threading is not required.

Worker code must not own or receive:

* `egui::Context`;
* `wgpu::Device`;
* `wgpu::Queue`;
* textures;
* render passes;
* or UI widget state.

Decoded CPU buffers return to the UI/GPU-owning thread for upload.

Worker completion must wake the application through an explicit repaint request rather than relying on an unconditional frame loop.

## 7.6 Resource residency and upload

Implement a bounded GPU tile cache with:

* a configurable byte budget;
* recency or cost-based eviction;
* shared residency across viewports;
* cache-hit and cache-miss accounting;
* and deterministic behaviour tests.

Implement a configurable per-frame upload budget. Large bursts of completed tiles must be spread across frames rather than producing an unbounded upload spike.

The runtime must expose at least these tile states or their semantic equivalents:

```text
Missing
Queued
Decoding
Decoded
Resident
Failed
```

## 7.7 Vector annotation editing

Implement a minimal vector annotation layer over the image views.

The user must be able to:

* activate a polygon tool;
* add polygon vertices;
* see an in-progress live preview;
* commit the polygon;
* select an existing polygon;
* move at least one polygon vertex;
* delete a polygon;
* undo and redo the durable operations.

Annotations must be stored in world or image coordinates rather than screen coordinates.

Annotations should appear consistently in linked image views.

A complete polygon construction or vertex-drag gesture should create one logical undo record, not one record per pointer event.

Raster editing is outside this slice.

## 7.8 Result table

The result pane must:

* expose at least 1,000,000 logical rows;
* instantiate only the visible range plus a bounded overscan;
* use stable result IDs;
* preserve selection as rows enter and leave the visible range;
* permit selecting a row;
* and provide an action that recentres an image view on the selected result.

The implementation must not create a `Vec` containing one million complete UI row models.

The diagnostics panel must report:

* visible range;
* rows materialised during the current frame;
* overscan;
* and total logical row count.

At the default demonstration window size, fewer than 500 result rows should be materialised per frame.

## 7.9 Thumbnail gallery

The thumbnail pane must:

* present a two-dimensional virtualised grid;
* calculate the visible item range from scroll position and cell geometry;
* request only visible items plus bounded overscan;
* show placeholders while thumbnails are pending;
* and preserve stable selection.

Selecting a thumbnail must select the corresponding result and provide a path to recenter an image view.

The implementation must not create UI widgets or thumbnail demands for all logical items.

## 7.10 Selection and command routing

Selection must be authoritative application/session state, not duplicated independently in each pane.

Selection changes from:

* a GPU view;
* the result table;
* the thumbnail gallery;
* and the inspector

must converge through explicit intents.

Keyboard commands such as undo, redo, fit view and delete should route according to active-pane and focus context rather than relying on hidden global callbacks.

## 7.11 Persistence

Persist at least:

* the canonical workspace layout;
* pane configuration;
* camera-link membership;
* and the active or last selected pane.

Native persistence may use a local versioned file or the platform storage supplied by the integration layer.

Browser persistence may use local storage through a narrow Rust-owned adapter.

JavaScript must not maintain a second authoritative copy of the application or workspace state.

Provide a visible “Reset workspace” action.

---

# 8. Repaint and scheduling behaviour

The application must be event-driven.

Do not call an unconditional repaint request from every frame.

Another frame may be requested for:

* active pointer or keyboard interaction;
* an animation with an explicit deadline;
* tile or thumbnail completion;
* pending upload work;
* an application command;
* or another recorded reason.

Record application-originated repaint reasons in diagnostics.

Once the application is fully loaded and idle, with no active interaction, pending upload, worker completion or animation, the application must stop deliberately requesting continuous frames.

Provide a diagnostic counter or state that makes this behaviour auditable.

---

# 9. Instrumentation requirements

Instrumentation is part of the architecture, not a finishing task.

Add instrumentation before substantial optimisation.

## 9.1 Live diagnostics panel

The Diagnostics pane must expose, at minimum:

### Frame and UI

* frame number;
* recent CPU frame-time history;
* runtime-poll time;
* UI construction time;
* demand-reconciliation time;
* render preparation and submission time where measurable;
* current and recent repaint reasons;
* whether an interaction or animation is keeping the app active.

### Workspace

* number of registered panes;
* number of visible panes;
* active pane;
* current dock-tree node count;
* layout serialisation size.

### Rendering

* number of GPU viewports submitted;
* number of application render jobs;
* number of application render passes;
* number of application draw calls where the renderer can count them;
* command-buffer count;
* bytes uploaded this frame;
* pending upload bytes;
* resident GPU texture bytes;
* current renderer capability profile.

### Tiles and workers

* total demands;
* visible demands;
* prefetch demands;
* duplicate demands removed;
* cache hits and misses;
* evictions;
* queued requests;
* in-flight decodes;
* completed decodes;
* failed decodes;
* stale completions discarded;
* decode latency summary;
* worker queue depth.

### Virtualisation

* total logical result rows;
* visible row range;
* result rows materialised this frame;
* total logical thumbnails;
* visible thumbnail range;
* thumbnail cells materialised this frame.

GPU timestamp data should be used when available and practical. When unavailable, report it as unavailable rather than presenting CPU submission time as GPU execution time.

## 9.2 Tracing

Add structured tracing spans around significant operations, including:

* frame processing;
* demand reconciliation;
* worker decode;
* tile upload;
* cache eviction;
* render preparation;
* viewport rendering;
* layout serialisation;
* and command dispatch.

Tracing must be usable in both native and browser builds, even if the output backends differ.

## 9.3 Metrics export

Provide a way to copy or save a structured diagnostics snapshot, preferably JSON.

A snapshot must be suitable for inclusion in the final verification report and should include:

* application and dependency version information;
* active backend and adapter information;
* viewport count;
* cache configuration;
* logical dataset sizes;
* and current instrumentation counters.

## 9.4 Performance baselines

Capture release-build observations for at least:

1. initial loading;
2. four visible GPU viewports while panning;
3. a warmed and idle workspace;
4. rapid zoom causing tile-level transitions;
5. scrolling the million-row result table;
6. scrolling the thumbnail gallery;
7. editing a polygon;
8. restoring a saved workspace.

Do not invent universal pass/fail frame-time thresholds. Record the test machine, build mode, backend, window size and observed median/p95 values where meaningful.

Use measurements to identify bottlenecks before undertaking renderer or framework rewrites.

---

# 10. Verification surface

Provide one documented verification entry point, preferably:

```text
cargo xtask verify
```

It should run the applicable equivalents of:

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo build --workspace --release
<wasm build command>
<browser smoke-test command>
```

Platform-specific feature combinations may require separate clippy or build invocations. Document these explicitly rather than weakening checks silently.

## 10.1 Required automated tests

Include focused tests for:

* canonical dock-tree invariants;
* dock-layout serialisation round trip;
* pane-ID stability;
* camera-link propagation;
* coordinate-space transformations;
* intent-to-command validation;
* command application;
* undo and redo;
* gesture command coalescing;
* tile-demand derivation;
* demand deduplication across viewports;
* visible-versus-prefetch priority;
* stale-generation rejection;
* tile-cache accounting and eviction;
* per-frame upload budgeting;
* virtual result-range calculation;
* virtual thumbnail-grid calculation;
* stable selection;
* and persistence schema handling.

Domain reducers, demand reconciliation and virtualisation calculations must be testable without opening a window or initialising a GPU.

## 10.2 Native verification

Actually launch the native release build on the available development host.

Verify and capture evidence that:

* the default workspace renders;
* all four GPU views display content;
* docking and resizing work;
* camera linking works;
* progressive refinement is visible;
* polygon editing and undo/redo work;
* result and thumbnail selection recenter a view;
* layout restoration works;
* and the diagnostics pane reports sensible values.

## 10.3 Browser verification

Build and launch the WebAssembly application in a real browser.

The browser smoke test, whether automated through Playwright or another suitable mechanism, must at least:

* detect that application initialisation completed;
* fail on an application panic or unexpected console error;
* verify that the main canvas has non-zero dimensions;
* verify that the expected panes and renderer have initialised through an application readiness or diagnostics interface;
* and capture a screenshot.

Also manually or automatically verify:

* worker-decoded tile completion;
* interactive pan and zoom;
* docking or splitter interaction;
* and layout persistence across reload.

Keep JavaScript glue narrow and platform-oriented.

## 10.4 Architecture verification

Add an automated or documented check confirming:

* `polyorama-core` has no egui, eframe, wgpu or browser dependency;
* `polyorama-runtime` has no egui dependency;
* domain reducers can run without a GPU;
* pane UI APIs do not receive mutable access to the complete application model;
* there is one canonical workspace tree;
* and no viewport owns a separate wgpu device.

---

# 11. Acceptance invariants

The following are mandatory:

* Four image viewports use one shared wgpu device.
* At least one viewport samples a scalar non-RGBA texture through a custom shader.
* The complete logical raster is never allocated.
* Duplicate demands for the same tile result in at most one active decode.
* Multiple views may share one resident tile.
* The configured upload budget is respected.
* The tile cache remains within its documented resource-budget policy.
* Hidden panes do not continue issuing high-resolution visible demands.
* One million result rows do not produce one million row objects.
* One hundred thousand thumbnails do not produce one hundred thousand thumbnail widgets or requests.
* Selection is authoritative and shared rather than copied into independent pane stores.
* Camera-link propagation is explicit and testable.
* A complete editing gesture produces a coalesced command.
* Layout restoration uses the canonical workspace representation.
* The application does not deliberately repaint continuously while idle.
* Native and browser builds use the same Rust domain and workspace model.
* Workers do not own UI or GPU objects.
* Diagnostics expose enough evidence to verify these claims.

---

# 12. Non-goals

Do not expand the slice to include:

* a replacement for egui’s general widget system;
* a new text-layout or font-shaping engine;
* a new accessibility framework;
* a general retained visual element tree;
* a backend-neutral abstraction supporting multiple GUI libraries;
* a production-ready public framework API;
* a comprehensive styled component library;
* production geospatial CRS transformations;
* network services or remote data access;
* production image codecs;
* real private or proprietary imagery;
* collaborative editing;
* multiple top-level browser windows;
* complete WebGL2 parity;
* raster painting;
* a full spatial database;
* a generic render graph;
* arbitrary third-party GPU texture import;
* or visual polish that delays architectural verification.

WebGPU is the required browser rendering path for this first slice. A WebGL2 fallback may be explored only when it does not distort the architecture or delay completion of the required path.

Do not fork egui, wgpu or the selected docking library unless a verified blocker leaves no smaller path. Any fork or upstream patch requires a written rationale and minimal reproduction.

---

# 13. Implementation policy

## 13.1 Inspect before changing

Begin by:

1. inspecting the repository and existing instructions;
2. running the current build and tests;
3. identifying existing crates and reusable infrastructure;
4. recording relevant dependency and platform constraints;
5. and writing a brief implementation plan tied to this specification.

Do not assume the repository is empty.

## 13.2 Build thin end-to-end increments

Prefer the following sequence, adapting it as evidence requires:

### Increment 1: runnable shell and diagnostics skeleton

* Native and browser application entry points.
* Egui/wgpu integration.
* Canonical workspace skeleton.
* Diagnostics and tracing foundations.
* One placeholder pane rendered on both targets.

### Increment 2: one scalar tiled viewport

* Deterministic scalar tile source.
* Custom shader.
* Pan and zoom.
* Correct clipping and physical viewport handling.
* Initial render metrics.

### Increment 3: progressive demand pipeline

* Demand set.
* Native worker.
* Browser worker.
* Decode completion events.
* GPU upload queue and budgets.
* Tile cache.
* Event-driven repainting.

### Increment 4: multi-viewport workspace

* Four GPU views.
* Docking and resizing.
* Camera link groups.
* Overview footprints.
* Derived shader view.
* Shared tile residency.

### Increment 5: application interaction

* Tool model.
* Polygon creation.
* Selection.
* Vertex editing.
* Coalesced undo and redo.
* Result-driven recentering.

### Increment 6: large-data presentation

* Million-row virtualised result table.
* Two-dimensional virtualised thumbnail gallery.
* Progressive thumbnail demand.
* Selection synchronisation.

### Increment 7: persistence and hardening

* Layout persistence.
* Browser reload restoration.
* Automated architecture checks.
* Full verification command.
* Release profiling.
* Final evidence report.

Every increment must leave the application runnable. Do not spend several iterations building an abstract framework before exercising it in the application.

## 13.3 Verify after each increment

After each meaningful change:

* run focused tests;
* run the relevant native or browser build;
* inspect instrumentation;
* record regressions or new constraints;
* and choose the next action based on the strongest current evidence.

Do not optimise speculative hot paths.

Do not replace egui or its integration because of an assumed performance limitation. First produce a measured profile showing that the limitation is material after appropriate virtualisation and demand control.

## 13.4 Make reversible decisions

Codex may select without further approval:

* the existing egui docking crate;
* the compression format used by synthetic fixtures;
* the exact scalar texture format after capability probing;
* the worker-message implementation;
* the tracing backend;
* the browser test harness;
* module and crate names;
* and the exact visual design.

Choose the simplest mature option that satisfies the contract. Record non-obvious decisions in concise architecture decision records.

## 13.5 Maintain durable agent guidance

Create or update the repository `AGENTS.md` with concise, enforceable rules covering:

* dependency boundaries;
* state ownership;
* pane UI signatures;
* canonical workspace ownership;
* verification commands;
* instrumentation expectations;
* and prohibited shortcuts.

Do not duplicate this entire document in `AGENTS.md`. Keep that file operational and easy for future coding agents to follow.

---

# 14. Blocked stop condition

Do not mark the Goal complete when a mandatory criterion is unverified.

When progress is blocked by an upstream library, browser environment, GPU capability or unavailable tool:

1. isolate the issue in the smallest practical reproduction;
2. record exact dependency versions and platform details;
3. record the commands used;
4. capture the actual error or observed behaviour;
5. document the attempted solutions and their results;
6. explain which acceptance criteria remain blocked;
7. identify the smallest dependency update, API change, environment capability or user decision that would unlock progress;
8. preserve all completed, verified work;
9. and produce an interim blocker report.

Examples:

* If GPU timestamps are unsupported, mark those metrics unavailable and continue using honest CPU-side metrics; this alone is not a blocker.
* If a selected scalar format is unsupported, select another scalar format and document the capability decision.
* If a browser runner is unavailable, a successful WASM build is not evidence that runtime behaviour works. Report browser runtime verification as blocked.
* If Web Worker integration exposes an upstream defect, produce a minimal worker reproduction rather than moving decode back into the UI frame and claiming success.
* If the chosen docking crate cannot maintain a canonical serialisable tree, adapt or replace that boundary rather than adding a second synchronised tree.

A blocker report is a valid stop condition, but it is not successful completion.

---

# 15. Final evidence report

Create:

```text
docs/vertical-slice-report.md
```

The report must contain:

## Summary

* What was implemented.
* Which architectural hypothesis the slice supports or weakens.
* Native and browser status.
* Important limitations.

## Architecture

* Final crate and module structure.
* State ownership.
* Frame data flow.
* Intent, command, effect, event, demand and render-request relationships.
* Workspace-tree ownership.
* Worker boundary.
* GPU resource ownership.
* Repaint scheduling.

## Acceptance matrix

Use a table with:

| Criterion | Status | Evidence | Notes |
| --------- | ------ | -------- | ----- |

Every mandatory criterion in this specification must appear.

## Verification commands

Record:

* exact commands;
* whether each passed;
* relevant output summaries;
* platform details;
* browser and GPU backend;
* dependency versions or lockfile revision.

## Performance observations

Include release-build observations for the scenarios in Section 9.4.

Separate:

* directly measured values;
* inferred explanations;
* unavailable metrics;
* and untested claims.

## Screenshots

Include at least:

* native application default workspace;
* browser application default workspace;
* rearranged dock layout;
* progressive tile loading or diagnostics;
* result table or thumbnail virtualisation;
* polygon editing.

## Known issues and deferred work

Explain:

* current limitations;
* any reduced capability profiles;
* upstream constraints;
* performance findings;
* and which possible framework abstractions should remain deferred.

## Recommendation

End with an evidence-based recommendation on whether the next iteration should:

* continue as a layer above egui;
* deepen a particular subsystem;
* extract reusable components;
* or investigate a measured lower-level blocker.

Do not recommend replacing egui unless the report contains concrete evidence that an egui boundary is responsible for a material unresolved limitation.

---

# 16. Definition of done

The Goal is complete only when all of the following are true:

* [ ] The native application builds and has been run successfully on the development host.
* [ ] The browser application builds and has been run successfully in a real browser.
* [ ] Four independently rendered GPU image panes are visible and use one shared wgpu device.
* [ ] At least two view cameras can be linked and unlinked.
* [ ] One viewport displays non-RGBA scalar data through a custom shader.
* [ ] A large virtual multiresolution raster is progressively tiled without full-raster allocation.
* [ ] Compressed tiles are decoded away from the UI/GPU thread on native and in a browser worker.
* [ ] Duplicate tile demand is reconciled across viewports.
* [ ] GPU residency and per-frame uploads are bounded and instrumented.
* [ ] Worker completion causes an explicit repaint.
* [ ] The warmed idle application does not deliberately request continuous repainting.
* [ ] The canonical dock workspace supports tabs, splits, resizing and drag-and-drop rearrangement.
* [ ] The canonical layout serialises, restores and passes a round-trip test.
* [ ] Polygon creation, selection, vertex movement, deletion, undo and redo work.
* [ ] Editing gestures use live preview and coalesced durable commands.
* [ ] The result table exposes at least 1,000,000 logical rows without materialising them all.
* [ ] The thumbnail gallery exposes at least 100,000 logical items without materialising or requesting them all.
* [ ] Result, thumbnail and viewport selection converge through authoritative shared state.
* [ ] Selecting a result or thumbnail can recenter an image view.
* [ ] Diagnostics expose frame, workspace, renderer, tile, worker, cache, upload and virtualisation metrics.
* [ ] A structured diagnostics snapshot can be copied or saved.
* [ ] Core reducers and reconciliation logic are testable without egui or a GPU.
* [ ] The required dependency boundaries are preserved.
* [ ] A single documented verification command runs formatting, linting, tests and builds.
* [ ] Native and browser screenshots have been captured.
* [ ] Release-build performance observations have been recorded honestly.
* [ ] `AGENTS.md` contains concise durable architectural instructions.
* [ ] `docs/vertical-slice-report.md` maps every criterion to concrete evidence.
* [ ] No mandatory criterion is marked complete based only on expectation, compilation or narrative reasoning.

The finish line is a verified application-shaped slice and an evidence-backed architectural assessment—not the declaration of a stable general-purpose GUI framework.
