#![allow(clippy::collapsible_if)] // Keep lifecycle actions grouped in the immediate UI flow.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use serde::{Deserialize, Serialize};
use stobie_core::*;
use stobie_imaging::*;
use stobie_render_wgpu::{
    DisplayMap, DisplaySettings, RenderBridge, ScalarRenderer, UploadAdmission,
};
use stobie_runtime::{DEFAULT_CACHE_BUDGET, DEFAULT_UPLOAD_BUDGET, DecodeEvent, Runtime};
use stobie_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionTarget, AppHost,
    Application, ApplicationIdentity, Availability, CollectionObservation, CompletedPass, Frame,
    HostRepaint, Observation, PassContext, SemanticUiId, UiNode, UiPreferences, UiRole, UiSnapshot,
    action_button, application_bar_frame, application_bar_height, measured_inline_label,
    preferences_control, stage_renderer_maintenance, submit_render_plan,
};
use tracing::info_span;
use web_time::Instant;

use crate::{
    APPLICATION_NAME,
    actions::{ActionContext, LabAction, availability},
    demo::{RESULT_COUNT, analytical_workspace, default_session, image_panes, result_at},
    metrics::{DiagnosticsSnapshot, VirtualisationMetrics},
    panes::{
        AnnotationUiState, FrameOutput, PaneFeatureState, PaneIntent, PaneReadModel, PaneSurface,
        UiBehaviour, should_cancel_camera_drag,
    },
    thumbnail_cache::ThumbnailCache,
    ui_geometry::UiGeometry,
};

const STORAGE_KEY: &str = "stobie.vertical-slice.v2";
// Read the former key on the same origin/container; all writes use STORAGE_KEY.
const LEGACY_STORAGE_KEY: &str = "polyorama.vertical-slice.v2";
const DIAGNOSTICS_PANE: PaneId = PaneId(8);
const DIAGNOSTICS_ANCHOR: PaneId = PaneId(7);

fn toggle_diagnostics(workspace: &mut Workspace) -> bool {
    if workspace.closed_optional_panes.contains(&DIAGNOSTICS_PANE) {
        workspace.reopen_pane(DIAGNOSTICS_PANE, DIAGNOSTICS_ANCHOR)
    } else {
        workspace.close_pane(DIAGNOSTICS_PANE)
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct PersistedState {
    schema_version: u32,
    #[serde(default)]
    preferences: UiPreferences,
    workspace: Workspace,
    document: Document,
    session: Session,
    display: BTreeMap<PaneId, DisplaySettingsDto>,
}

fn load_persisted(storage: &dyn eframe::Storage) -> Option<PersistedState> {
    // An invalid current value must not resurrect an older saved workspace.
    let key = if storage.get_string(STORAGE_KEY).is_some() {
        STORAGE_KEY
    } else {
        LEGACY_STORAGE_KEY
    };
    eframe::get_value::<PersistedState>(storage, key).filter(persisted_state_is_valid)
}

#[derive(Clone, Copy, Serialize, Deserialize)]
struct DisplaySettingsDto {
    low: f32,
    high: f32,
    map: u8,
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TestAction {
    SetCamera {
        pane: u32,
        centre_x: f64,
        centre_y: f64,
        pixels_per_screen_point: f64,
    },
    CommitPolygon {
        vertices: Vec<(f64, f64)>,
    },
    Undo,
    ResizeSplit {
        node: u64,
        fraction: f32,
    },
    QueueZeroViewportUpload,
    RestoreDefaultWorkspace,
    MakeWorkerUnavailable,
    SetUiPreferences {
        appearance: stobie_ui_egui::AppearancePreference,
        contrast: stobie_ui_egui::ContrastPreference,
        density: stobie_ui_egui::DensityPreference,
        font_scale: f32,
        motion: stobie_ui_egui::MotionPreference,
    },
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TestSnapshot {
    cameras: Vec<CameraState>,
    render_cameras: Vec<CameraState>,
    visible_tile_keys: Vec<TileKey>,
    annotations: Vec<Polygon>,
    annotation_count: usize,
    selected_annotation: Option<AnnotationId>,
    undo_depth: usize,
    workspace_hash: String,
    thumbnail_resident_keys: Vec<TileKey>,
    virtualisation: VirtualisationMetrics,
    runtime: RuntimeMetrics,
    render: RenderMetrics,
    visible_panes: Vec<PaneId>,
    frame_number: u64,
    repaint_requests: u64,
    physical_wheel_events: u64,
    ui_geometry: UiGeometry,
    ui_snapshot: UiSnapshot,
    preferences: UiPreferences,
}

impl From<DisplaySettings> for DisplaySettingsDto {
    fn from(value: DisplaySettings) -> Self {
        Self {
            low: value.window_low,
            high: value.window_high,
            map: match value.map {
                DisplayMap::Viridis => 0,
                DisplayMap::Greyscale => 1,
                DisplayMap::Threshold => 2,
            },
        }
    }
}

impl From<DisplaySettingsDto> for DisplaySettings {
    fn from(value: DisplaySettingsDto) -> Self {
        Self {
            window_low: value.low,
            window_high: value.high,
            map: match value.map {
                1 => DisplayMap::Greyscale,
                2 => DisplayMap::Threshold,
                _ => DisplayMap::Viridis,
            },
        }
    }
}

fn persisted_state_is_valid(state: &PersistedState) -> bool {
    if state.schema_version != LAYOUT_SCHEMA_VERSION
        || state.workspace.validate().is_err()
        || state.session.validate_cameras(&image_panes()).is_err()
    {
        return false;
    }
    let expected_panes: BTreeSet<_> = (1..=8).map(PaneId).collect();
    let mut panes = Vec::new();
    state.workspace.root.pane_ids(&mut panes);
    let open_panes: BTreeSet<_> = panes.into_iter().collect();
    if !state
        .workspace
        .closed_optional_panes
        .is_subset(&BTreeSet::from([DIAGNOSTICS_PANE]))
        || open_panes
            .union(&state.workspace.closed_optional_panes)
            .copied()
            .collect::<BTreeSet<_>>()
            != expected_panes
    {
        return false;
    }
    let expected_images: BTreeSet<_> = (1..=4).map(PaneId).collect();
    if state.display.keys().copied().collect::<BTreeSet<_>>() != expected_images
        || state
            .session
            .active_tools
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != expected_images
        || state.display.values().any(|settings| {
            !settings.low.is_finite()
                || !settings.high.is_finite()
                || settings.low < 0.0
                || settings.high > 1.0
                || settings.low >= settings.high
                || settings.map > 2
        })
    {
        return false;
    }
    if state
        .session
        .selected_result
        .is_some_and(|result| result.0 >= RESULT_COUNT)
        || state.session.selected_annotation.is_some_and(|selected| {
            !state
                .document
                .annotations
                .iter()
                .any(|annotation| annotation.id == selected)
        })
    {
        return false;
    }
    true
}

/// Undo entries kept. Camera moves are undoable, so the Lab keeps more than
/// the default.
const HISTORY_CAPACITY: usize = 1000;

/// One undoable Lab change. Layout resizes share the image history; tab
/// activation and moves apply directly.
#[derive(Clone, Debug)]
enum LabCommand {
    Image(ImageCommand),
    Layout(WorkspaceCommand),
}

/// The state Lab commands change, borrowed for one history operation.
struct LabTarget<'a> {
    document: &'a mut Document,
    session: &'a mut Session,
    workspace: &'a mut Workspace,
}

impl Command<LabTarget<'_>> for LabCommand {
    fn apply(&self, target: &mut LabTarget<'_>) -> Result<bool, String> {
        match self {
            Self::Image(command) => {
                command.apply(&mut ImageTarget::new(target.document, target.session))
            }
            Self::Layout(command) => command.apply(target.workspace),
        }
    }

    fn revert(&self, target: &mut LabTarget<'_>) -> Result<(), String> {
        match self {
            Self::Image(command) => {
                command.revert(&mut ImageTarget::new(target.document, target.session))
            }
            Self::Layout(command) => command.revert(target.workspace),
        }
    }
}

/// The Lab's application state. The UI reads it and emits [`Intent`]s; actions
/// run in [`Application::perform`]. The host owns the frame lifecycle.
pub struct Lab {
    workspace: Workspace,
    document: Document,
    session: Session,
    history: History<LabCommand>,
    runtime: Runtime,
    render_bridge: RenderBridge,
    display: BTreeMap<PaneId, DisplaySettings>,
    diagnostics: DiagnosticsSnapshot,
    frame_started: Instant,
    pending_renderer_upload: Option<DecodeEvent>,
    status: String,
    preferences: UiPreferences,
    save_requested: bool,
    last_render_cameras: Vec<CameraState>,
    last_visible_tile_keys: Vec<TileKey>,
    last_ui_geometry: UiGeometry,
    last_ui_snapshot: UiSnapshot,
    #[cfg(target_arch = "wasm32")]
    browser_worker: Option<crate::web_worker::BrowserWorker>,
}

/// UI-owned state: camera gestures, thumbnail textures, virtualisation
/// metrics, and the physical-test geometry and render plan of the latest pass.
#[derive(Default)]
pub struct LabView {
    ui_behaviour: UiBehaviour,
    thumbnail_cache: ThumbnailCache,
    virtualisation: VirtualisationMetrics,
    geometry: UiGeometry,
    render_cameras: Vec<CameraState>,
    visible_tile_keys: Vec<TileKey>,
    ui_ms: f64,
    /// Whether a gesture or preview was in progress in the latest pass.
    interaction_active: bool,
    /// Return focus to View/Panels after Diagnostics is toggled.
    focus_panels: bool,
}

/// The hosted Lab, as run natively and in the browser.
pub type AnalyticalWorkspaceApp = AppHost<Lab>;

/// Data changes from one UI pass. They apply in emission order: the gesture
/// preview first, then pane presentation, commands, image intents and demands,
/// as the Lab applied its frame outputs before hosting.
pub enum Intent {
    /// The session's gesture preview after this pass's edits. The pass has
    /// already rendered it.
    Gesture(Option<GesturePreview>),
    Pane(PaneIntent),
    Command(ImageCommand),
    Image(ImageIntent),
    Layout(WorkspaceChange),
    /// This pass's desired tiles.
    Demands(Vec<TileDemand>),
    Preferences(UiPreferences),
    Status(String),
}

impl From<WorkspaceChange> for Intent {
    fn from(change: WorkspaceChange) -> Self {
        Self::Layout(change)
    }
}

impl Lab {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let persisted = cc.storage.and_then(load_persisted);
        let preferences = persisted
            .as_ref()
            .map_or_else(UiPreferences::default, |saved| {
                saved.preferences.validated()
            });
        let (workspace, document, session, display) = if let Some(saved) = persisted {
            (
                saved.workspace,
                saved.document,
                saved.session,
                saved
                    .display
                    .into_iter()
                    .map(|(pane, value)| (pane, value.into()))
                    .collect(),
            )
        } else {
            (
                analytical_workspace(),
                Document::default(),
                default_session(),
                default_display(),
            )
        };
        let render_bridge = RenderBridge::default();
        let mut diagnostics = DiagnosticsSnapshot {
            application_version: env!("CARGO_PKG_VERSION").into(),
            dependency_versions: BTreeMap::from([
                ("eframe".into(), "0.36.1".into()),
                ("egui".into(), "0.36.1".into()),
                ("egui-wgpu".into(), "0.36.1".into()),
                ("wasm-bindgen".into(), "0.2.127".into()),
                ("wgpu".into(), "30.0.1".into()),
            ]),
            platform: if cfg!(target_arch = "wasm32") {
                "wasm32/WebGPU".into()
            } else {
                std::env::consts::OS.into()
            },
            tile_cache_budget_bytes: DEFAULT_CACHE_BUDGET,
            upload_budget_bytes: DEFAULT_UPLOAD_BUDGET,
            ..Default::default()
        };
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            let info = render_state.adapter.get_info();
            diagnostics.backend = format!("{:?}", info.backend);
            diagnostics.adapter = info.name;
            render_state
                .renderer
                .write()
                .callback_resources
                .insert(ScalarRenderer::new(
                    &render_state.device,
                    render_state.target_format,
                    render_bridge.clone(),
                ));
        }
        #[allow(unused_mut)] // The browser records an unavailable worker.
        let mut runtime = Runtime::default();
        #[cfg(target_arch = "wasm32")]
        let browser_worker = match crate::web_worker::BrowserWorker::new(cc.egui_ctx.clone()) {
            Ok(worker) => Some(worker),
            Err(error) => {
                crate::startup_fail("worker_create", &format!("{error:?}"));
                runtime.record_browser_worker_unavailable(format!(
                    "browser worker could not be created: {error:?}"
                ));
                None
            }
        };
        #[cfg(not(target_arch = "wasm32"))]
        runtime.set_repaint_waker(std::sync::Arc::new({
            let context = cc.egui_ctx.clone();
            move || context.request_repaint()
        }));
        Self {
            workspace,
            document,
            session,
            history: History::new(HISTORY_CAPACITY),
            runtime,
            render_bridge,
            display,
            diagnostics,
            frame_started: Instant::now(),
            pending_renderer_upload: None,
            status: "Initialising progressive data…".into(),
            preferences,
            save_requested: false,
            last_render_cameras: Vec::new(),
            last_visible_tile_keys: Vec::new(),
            last_ui_geometry: UiGeometry::default(),
            last_ui_snapshot: UiSnapshot::default(),
            #[cfg(target_arch = "wasm32")]
            browser_worker,
        }
    }

    fn persisted(&self) -> PersistedState {
        let _span = info_span!("layout_serialisation").entered();
        PersistedState {
            schema_version: LAYOUT_SCHEMA_VERSION,
            preferences: self.preferences.validated(),
            workspace: self.workspace.clone(),
            document: self.document.clone(),
            session: self.session.clone(),
            display: self
                .display
                .iter()
                .map(|(pane, settings)| (*pane, (*settings).into()))
                .collect(),
        }
    }

    /// Apply a browser test fixture. The caller requests a repaint and reads
    /// the resulting snapshot.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn test_action(&mut self, action: TestAction) -> Result<(), String> {
        let command = match action {
            TestAction::SetCamera {
                pane,
                centre_x,
                centre_y,
                pixels_per_screen_point,
            } => LabCommand::Image(validate_intent(
                ImageIntent::SetCamera {
                    pane: PaneId(pane),
                    camera: Camera {
                        centre: ImagePoint::new(centre_x, centre_y),
                        pixels_per_screen_point,
                    },
                },
                &mut self.document,
                &self.session,
            )?),
            TestAction::CommitPolygon { vertices } => LabCommand::Image(validate_intent(
                ImageIntent::CommitPolygon {
                    layer: LayerId(1),
                    vertices: vertices
                        .into_iter()
                        .map(|(x, y)| WorldPoint::new(x, y))
                        .collect(),
                },
                &mut self.document,
                &self.session,
            )?),
            TestAction::Undo => {
                if !self.undo()? {
                    return Err("nothing to undo".into());
                }
                self.record_repaint(RepaintReason::Command);
                return Ok(());
            }
            TestAction::ResizeSplit { node, fraction } => {
                let node = DockNodeId(node);
                let before = self
                    .workspace
                    .root
                    .split_fraction(node)
                    .ok_or("unknown split node")?;
                LabCommand::Layout(WorkspaceCommand::Resize(WorkspaceResize {
                    node,
                    before,
                    after: fraction,
                }))
            }
            TestAction::QueueZeroViewportUpload => {
                self.runtime.invalidate();
                let key = TileKey {
                    source: SourceId(1),
                    level: 0,
                    x: 7,
                    y: 11,
                };
                self.runtime.reconcile([TileDemand {
                    key,
                    priority: DemandPriority::Visible,
                    generation: self.runtime.generation(),
                }]);
                let request = self
                    .runtime
                    .take_external_request()
                    .ok_or("zero-viewport probe could not acquire a worker request")?;
                self.runtime.accept_event(DecodeEvent::Completed {
                    key,
                    token: request.token,
                    scalar_u16_le: vec![0; (TILE_SIZE * TILE_SIZE * 2) as usize],
                    preparation_ms: 0.0,
                    decode_ms: 0.0,
                });
                let decoded = self
                    .runtime
                    .take_decoded_for_renderer()
                    .ok_or("zero-viewport probe did not create a renderer hand-off")?;
                if !self.render_bridge.push(decoded).accepted() {
                    return Err("zero-viewport probe renderer bridge was full".into());
                }
                self.workspace.root = DockNode::Tabs {
                    id: DockNodeId(1),
                    tabs: (1..=8).map(PaneId).collect(),
                    active: 4,
                };
                self.workspace.active_pane = PaneId(5);
                self.record_repaint(RepaintReason::PendingUpload);
                return Ok(());
            }
            TestAction::RestoreDefaultWorkspace => {
                self.workspace = analytical_workspace();
                self.record_repaint(RepaintReason::Command);
                return Ok(());
            }
            TestAction::MakeWorkerUnavailable => {
                self.browser_worker = None;
                self.runtime
                    .record_browser_worker_unavailable("semantic probe: Worker unavailable");
                let key = TileKey {
                    source: SourceId(1),
                    level: 0,
                    x: 13,
                    y: 17,
                };
                self.runtime.reconcile([TileDemand {
                    key,
                    priority: DemandPriority::Visible,
                    generation: self.runtime.generation(),
                }]);
                self.record_repaint(RepaintReason::WorkerCompletion);
                return Ok(());
            }
            TestAction::SetUiPreferences {
                appearance,
                contrast,
                density,
                font_scale,
                motion,
            } => {
                // The host applies the design system in the next pass and
                // records that repaint as `Appearance`.
                self.preferences = UiPreferences {
                    appearance,
                    contrast,
                    density,
                    font_scale,
                    motion,
                    ..self.preferences
                }
                .validated();
                return Ok(());
            }
        };
        self.execute(command)?;
        self.record_repaint(RepaintReason::Command);
        Ok(())
    }

    pub(crate) fn test_snapshot(&self, view: &LabView) -> TestSnapshot {
        let mut visible_panes = Vec::new();
        self.workspace.root.active_panes(&mut visible_panes);
        TestSnapshot {
            cameras: self.session.cameras.clone(),
            render_cameras: self.last_render_cameras.clone(),
            visible_tile_keys: self.last_visible_tile_keys.clone(),
            annotations: self.document.annotations.clone(),
            annotation_count: self.document.annotations.len(),
            selected_annotation: self.session.selected_annotation,
            undo_depth: self.history.undo_len(),
            workspace_hash: format!("{:016x}", stable_workspace_hash(&self.workspace)),
            thumbnail_resident_keys: view.thumbnail_cache.keys(),
            virtualisation: self.diagnostics.virtualisation.clone(),
            runtime: self.runtime.metrics.clone(),
            render: self.render_bridge.snapshot(),
            visible_panes,
            frame_number: self.diagnostics.frame.frame_number,
            repaint_requests: self.diagnostics.frame.repaint_requests,
            physical_wheel_events: self.diagnostics.frame.physical_wheel_events,
            ui_geometry: self.last_ui_geometry.clone(),
            ui_snapshot: self.last_ui_snapshot.clone(),
            preferences: self.preferences.validated(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write_native_test_snapshot(&self, view: &LabView) -> Result<(), String> {
        let path = std::env::var("STOBIE_TEST_SNAPSHOT_PATH")
            .map_err(|_| "STOBIE_TEST_SNAPSHOT_PATH is not configured".to_owned())?;
        let json = serde_json::to_string_pretty(&self.test_snapshot(view))
            .map_err(|error| format!("native test snapshot could not be serialised: {error}"))?;
        std::fs::write(&path, json).map_err(|error| {
            format!("native test snapshot could not be written to {path}: {error}")
        })
    }

    /// Count one repaint for the diagnostics pane. The host requests the pass.
    fn record_repaint(&mut self, reason: RepaintReason) {
        self.diagnostics.frame.repaint_requests += 1;
        self.diagnostics.frame.repaint_reason = reason.clone();
        self.diagnostics.frame.recent_reasons.push_front(reason);
        self.diagnostics.frame.recent_reasons.truncate(8);
    }

    fn action_context(&self, pane: Option<PaneId>) -> ActionContext {
        ActionContext {
            undo_depth: self.history.undo_len(),
            redo_depth: self.history.redo_len(),
            active_pane: self.workspace.active_pane,
            target_pane: pane,
            selected_annotation: self.session.selected_annotation,
            selected_result: self.session.selected_result,
            polygon_vertices: crate::actions::polygon_vertex_count(self.session.gesture.as_ref()),
        }
    }

    fn poll_runtime(&mut self, pass: &mut PassContext<'_>, view: &mut LabView) {
        let started = Instant::now();
        let native_completed = self.runtime.poll();
        #[cfg(target_arch = "wasm32")]
        let browser_completed = {
            let (events, failures) = self.browser_worker.as_ref().map_or_else(
                || (Vec::new(), Vec::new()),
                |worker| (worker.drain(), worker.drain_failures()),
            );
            let count = events.len();
            let terminal_failure = !failures.is_empty();
            for event in events {
                self.runtime.accept_event(event);
            }
            for failure in failures {
                self.status = failure.clone();
                self.runtime.record_browser_worker_unavailable(failure);
            }
            if terminal_failure {
                self.browser_worker = None;
            }
            count
        };
        #[cfg(not(target_arch = "wasm32"))]
        let browser_completed = 0;
        if native_completed + browser_completed > 0 {
            // `completed` records the pass's repaint reasons once.
            pass.request_repaint(WORKER_COMPLETION);
        }
        let context = pass.context().clone();
        while let Some(event) = self
            .pending_renderer_upload
            .take()
            .or_else(|| self.runtime.take_decoded_for_renderer())
        {
            match event {
                DecodeEvent::Completed {
                    key,
                    token,
                    scalar_u16_le,
                    ..
                } if key.source == SourceId(2) => {
                    match view
                        .thumbnail_cache
                        .insert(&context, key, token, &scalar_u16_le)
                    {
                        Ok(evicted) => {
                            for (evicted_key, evicted_token) in evicted {
                                self.runtime.mark_evicted(evicted_key, evicted_token);
                            }
                            self.runtime.mark_resident(key, token);
                        }
                        Err(error) => {
                            self.status = error.clone();
                            self.runtime.mark_handoff_failed(key, token, error);
                        }
                    }
                }
                event => match self.render_bridge.push(event) {
                    UploadAdmission::Accepted => {}
                    UploadAdmission::Rejected { event, .. } => {
                        self.pending_renderer_upload = Some(event);
                        break;
                    }
                },
            }
        }
        for acknowledgement in self.render_bridge.take_evicted() {
            self.runtime
                .mark_evicted(acknowledgement.key, acknowledgement.token);
        }
        for acknowledgement in self.render_bridge.take_resident() {
            self.runtime
                .mark_resident(acknowledgement.key, acknowledgement.token);
        }
        if view.thumbnail_cache.len() > 0 || self.diagnostics.render.resident_texture_bytes > 0 {
            self.status = "Workspace ready".into();
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(worker) = &self.browser_worker {
            while let Some(request) = self.runtime.take_external_request() {
                if worker.submit(&request).is_err() {
                    crate::startup_fail("worker_transport", "Worker postMessage failed");
                    self.runtime.record_browser_transport_failure(
                        request,
                        "browser Worker postMessage failed",
                    );
                }
            }
        }
        self.diagnostics.frame.runtime_poll_ms = started.elapsed().as_secs_f64() * 1000.0;
    }

    fn execute(&mut self, command: LabCommand) -> Result<bool, String> {
        let mut target = LabTarget {
            document: &mut self.document,
            session: &mut self.session,
            workspace: &mut self.workspace,
        };
        self.history.execute(command, &mut target)
    }

    fn undo(&mut self) -> Result<bool, String> {
        let mut target = LabTarget {
            document: &mut self.document,
            session: &mut self.session,
            workspace: &mut self.workspace,
        };
        self.history.undo(&mut target)
    }

    fn redo(&mut self) -> Result<bool, String> {
        let mut target = LabTarget {
            document: &mut self.document,
            session: &mut self.session,
            workspace: &mut self.workspace,
        };
        self.history.redo(&mut target)
    }

    fn execute_image_intent(&mut self, intent: ImageIntent) -> Result<(), String> {
        let command = validate_intent(intent, &mut self.document, &self.session)?;
        self.execute(LabCommand::Image(command))?;
        Ok(())
    }

    fn update_diagnostics(&mut self, view: &LabView) {
        self.diagnostics.workspace = WorkspaceMetrics {
            registered_panes: 8,
            visible_panes: {
                let mut panes = Vec::new();
                self.workspace.root.active_panes(&mut panes);
                panes.len()
            },
            active_pane: Some(self.workspace.active_pane),
            dock_nodes: self.workspace.root.node_count(),
            serialised_bytes: self.workspace.serialised_size(),
        };
        self.diagnostics.runtime = self.runtime.metrics.clone();
        self.diagnostics.cameras = self.session.cameras.clone();
        self.diagnostics.render = self.render_bridge.snapshot();
        self.diagnostics.frame.render_prepare_ms = self.diagnostics.render.prepare_ms;
        self.diagnostics.virtualisation = view.virtualisation.clone();
        self.diagnostics.virtualisation.resident_thumbnails = view.thumbnail_cache.len();
        self.diagnostics.virtualisation.thumbnail_cache_bytes = view.thumbnail_cache.used();
    }

    #[cfg(target_arch = "wasm32")]
    fn publish_browser_diagnostics(&self) {
        use wasm_bindgen::JsValue;
        let Some(window) = web_sys::window() else {
            return;
        };
        let serializer = serde_wasm_bindgen::Serializer::json_compatible();
        let snapshot = self
            .diagnostics
            .serialize(&serializer)
            .unwrap_or(JsValue::NULL);
        let _ = js_sys::Reflect::set(
            &window,
            &JsValue::from_str("__STOBIE_DIAGNOSTICS"),
            &snapshot,
        );
        if let Some(document) = window.document() {
            if let Some(canvas) = document.get_element_by_id("stobie-canvas") {
                let _ = canvas.set_attribute("data-stobie-ready", "true");
                let _ = canvas.set_attribute(
                    "data-worker-completions",
                    &self.runtime.metrics.completed.to_string(),
                );
                let _ = canvas.set_attribute("data-pane-count", "8");
                let _ = canvas.set_attribute("data-renderer", "wgpu-scalar");
            }
        }
    }

    /// The application bar: identity, history and layout actions, the View and
    /// Appearance menus, and status lines.
    fn application_bar(
        &self,
        root_ui: &mut egui::Ui,
        view: &mut LabView,
        frame: &mut Frame<'_, LabAction, Intent>,
        ui_geometry: &mut UiGeometry,
    ) {
        let tokens = frame.tokens();
        let font_scale = self.preferences.font_scale;
        let application_bar_id = SemanticUiId::new("application.bar");
        let context = self.action_context(None);
        let mut preferences = self.preferences;
        let menu = egui::Panel::top("application-menu")
            .frame(application_bar_frame(&tokens))
            .exact_size(application_bar_height(&tokens, font_scale))
            .show(root_ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let compact_bar = ui.available_width()
                        < tokens.geometry.minimum_hit_size.0 * 34.0
                        || font_scale > 1.25;
                    measured_inline_label(
                        ui,
                        1,
                        APPLICATION_NAME,
                        stobie_ui_egui::TextRole::ApplicationTitle,
                        tokens.geometry.minimum_hit_size.0 * if compact_bar { 4.0 } else { 6.0 },
                        stobie_ui_egui::TextInteraction::Inert,
                        &tokens,
                        font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui.separator();
                    let bar_action = |ui: &mut egui::Ui, geometry: &mut UiGeometry, action| {
                        bar_action(
                            ui,
                            geometry,
                            &application_bar_id,
                            action,
                            availability(action, context),
                            compact_bar,
                            &tokens,
                            font_scale,
                        )
                    };
                    for action in [
                        LabAction::Undo,
                        LabAction::Redo,
                        LabAction::SaveLayout,
                        LabAction::ResetWorkspace,
                    ] {
                        if bar_action(ui, ui_geometry, action).0 {
                            frame.trigger(action);
                        }
                    }
                    let (_, panels) = bar_action(ui, ui_geometry, LabAction::ViewPanels);
                    if std::mem::take(&mut view.focus_panels) {
                        panels.request_focus();
                    }
                    if let Some(popup) = egui::Popup::menu(&panels).show(|ui| {
                        ui.set_width(tokens.geometry.minimum_hit_size.0 * 4.0);
                        let target = ActionTarget::application(LabAction::ToggleDiagnostics);
                        let enabled = availability(LabAction::ToggleDiagnostics, context);
                        let state = ActionButtonState::Toggle {
                            pressed: !self
                                .workspace
                                .closed_optional_panes
                                .contains(&DIAGNOSTICS_PANE),
                        };
                        let response = action_button(
                            ui,
                            ActionButtonSpec {
                                target,
                                availability: enabled.clone(),
                                state,
                                emphasis: ActionEmphasis::Quiet,
                                compact: false,
                            },
                            &tokens,
                            font_scale,
                            &mut ui_geometry.text_layouts,
                        );
                        ui_geometry.action(
                            application_bar_id.clone(),
                            target,
                            &enabled,
                            state,
                            &response,
                        );
                        if response.clicked() {
                            ui.close();
                        }
                        response.clicked()
                    }) && popup.inner
                    {
                        frame.trigger(LabAction::ToggleDiagnostics);
                    }
                    let (_, appearance) =
                        bar_action(ui, ui_geometry, LabAction::AppearanceSettings);
                    if let Some(popup) = egui::Popup::menu(&appearance).show(|ui| {
                        ui.set_width(tokens.geometry.minimum_hit_size.0 * 7.0);
                        preferences_control(
                            ui,
                            &mut preferences,
                            &application_bar_id,
                            LabAction::AppearanceSettings,
                        )
                    }) {
                        for node in popup.inner.nodes {
                            ui_geometry.record_node(node);
                        }
                    }
                    ui.separator();
                    if ui.available_width() > tokens.geometry.minimum_hit_size.0 * 5.0 {
                        let status_width = (ui.available_width()
                            - tokens.geometry.minimum_hit_size.0 * 4.0)
                            .min(tokens.geometry.minimum_hit_size.0 * 7.0);
                        let status = measured_inline_label(
                            ui,
                            2,
                            &self.status,
                            stobie_ui_egui::TextRole::Status,
                            status_width,
                            stobie_ui_egui::TextInteraction::Selectable,
                            &tokens,
                            font_scale,
                            &mut ui_geometry.text_layouts,
                        );
                        let mut node = UiNode::container(
                            SemanticUiId::new("application.status"),
                            Some(application_bar_id.clone()),
                            UiRole::Status,
                            status.rect.into(),
                        );
                        node.name = self.status.clone();
                        node.text_selectable = true;
                        ui_geometry.record_node(node);
                    }
                    if ui.available_width() > tokens.geometry.minimum_hit_size.0 * 5.0 {
                        let worker_status = format!(
                            "{} decoded thumbnails · {} decoding",
                            view.thumbnail_cache.len(),
                            self.runtime.metrics.in_flight
                        );
                        let worker_width = ui.available_width();
                        let response = measured_inline_label(
                            ui,
                            3,
                            &worker_status,
                            stobie_ui_egui::TextRole::Secondary,
                            worker_width,
                            stobie_ui_egui::TextInteraction::Selectable,
                            &tokens,
                            font_scale,
                            &mut ui_geometry.text_layouts,
                        );
                        let mut node = UiNode::container(
                            SemanticUiId::new("application.worker_status"),
                            Some(application_bar_id.clone()),
                            UiRole::Status,
                            response.rect.into(),
                        );
                        node.name = worker_status;
                        node.text_selectable = true;
                        ui_geometry.record_node(node);
                    }
                });
            });
        if preferences != self.preferences {
            frame.emit(Intent::Preferences(preferences));
        }
        ui_geometry.menu = Some(menu.response.rect.into());
        let mut application_bar = UiNode::container(
            application_bar_id,
            Some(SemanticUiId::root()),
            UiRole::ApplicationBar,
            menu.response.rect.into(),
        );
        application_bar.name = "Application actions".into();
        ui_geometry.record_node(application_bar);
    }
}

/// Present one application-bar action and record its semantics. Returns
/// whether an available action was clicked, and the button's response.
#[allow(clippy::too_many_arguments)]
fn bar_action(
    ui: &mut egui::Ui,
    geometry: &mut UiGeometry,
    parent: &SemanticUiId,
    action: LabAction,
    availability: Availability,
    compact: bool,
    tokens: &stobie_ui_egui::DesignTokens,
    font_scale: f32,
) -> (bool, egui::Response) {
    let target = ActionTarget::application(action);
    let response = action_button(
        ui,
        ActionButtonSpec {
            target,
            availability: availability.clone(),
            state: ActionButtonState::Momentary,
            emphasis: ActionEmphasis::Quiet,
            compact,
        },
        tokens,
        font_scale,
        &mut geometry.text_layouts,
    );
    geometry.action(
        parent.clone(),
        target,
        &availability,
        ActionButtonState::Momentary,
        &response,
    );
    (availability.enabled() && response.clicked(), response)
}

/// Map a host repaint reason to the Lab's diagnostic vocabulary.
fn repaint_reason(reason: HostRepaint) -> RepaintReason {
    match reason {
        HostRepaint::StateChanged | HostRepaint::Rejected => RepaintReason::Command,
        HostRepaint::DockInteraction | HostRepaint::AutomationEnabled => RepaintReason::Interaction,
        HostRepaint::Appearance => RepaintReason::Preferences,
        HostRepaint::Scheduled { .. } => RepaintReason::Scheduled,
        HostRepaint::Application(PENDING_UPLOAD) => RepaintReason::PendingUpload,
        HostRepaint::Application(WORKER_COMPLETION) => RepaintReason::WorkerCompletion,
        HostRepaint::Application(_) => RepaintReason::Interaction,
    }
}

const PENDING_UPLOAD: &str = "pending upload";
const WORKER_COMPLETION: &str = "worker completion";

impl Application for Lab {
    type Action = LabAction;
    type Intent = Intent;
    type UiState = LabView;

    fn identity(&self) -> ApplicationIdentity {
        stobie_ui_egui::application_identity!(APPLICATION_NAME)
    }

    fn availability(&self, target: ActionTarget<LabAction>) -> Availability {
        availability(target.action, self.action_context(target.pane))
    }

    /// Fit and Link depend on the pane's camera; the Diagnostics toggle on
    /// whether the pane is open.
    fn meaning(&self, target: ActionTarget<LabAction>) -> String {
        match target.action {
            LabAction::FitView | LabAction::LinkViews => serde_json::to_string(
                &self
                    .session
                    .cameras
                    .iter()
                    .find(|state| Some(state.pane) == target.pane),
            )
            .unwrap_or_default(),
            LabAction::ToggleDiagnostics => self
                .workspace
                .closed_optional_panes
                .contains(&DIAGNOSTICS_PANE)
                .to_string(),
            _ => "stable".into(),
        }
    }

    fn preferences(&self) -> UiPreferences {
        self.preferences
    }

    fn active_pane(&self) -> Option<PaneId> {
        Some(self.workspace.active_pane)
    }

    fn action_panes(&self, action: LabAction) -> Vec<PaneId> {
        if action.specification().scope == stobie_ui_egui::ActionScope::Application {
            Vec::new()
        } else {
            (1..=8).map(PaneId).collect()
        }
    }

    fn prepare(&mut self, pass: &mut PassContext<'_>, view: &mut LabView) {
        self.frame_started = Instant::now();
        self.diagnostics.frame.frame_number += 1;
        self.diagnostics.frame.repaint_reason = RepaintReason::None;
        let (wheel_events, cancel_camera_drag) = pass.context().input(|input| {
            let wheel = input
                .events
                .iter()
                .filter(|event| matches!(event, egui::Event::MouseWheel { .. }))
                .count() as u64;
            (wheel, should_cancel_camera_drag(input))
        });
        self.diagnostics.frame.physical_wheel_events += wheel_events;
        if cancel_camera_drag {
            view.ui_behaviour.cancel_camera_drags();
        }
        view.ui_behaviour.begin_frame();
        self.poll_runtime(pass, view);
    }

    fn ui(
        &self,
        root_ui: &mut egui::Ui,
        view: &mut LabView,
        frame: &mut Frame<'_, LabAction, Intent>,
    ) {
        let _span = info_span!(
            "frame_processing",
            frame = self.diagnostics.frame.frame_number
        )
        .entered();
        let ui_started = Instant::now();
        let ctx = root_ui.ctx().clone();
        let tokens = frame.tokens();
        let mut ui_geometry = UiGeometry::new(root_ui.max_rect(), ctx.pixels_per_point());
        self.application_bar(root_ui, view, frame, &mut ui_geometry);
        let mut outputs = FrameOutput::with_ui_geometry(ui_geometry);
        let frame_number = self.diagnostics.frame.frame_number;
        let generation = self.runtime.generation();
        // Panes edit a pass-local copy of the session's gesture preview, so the
        // preview renders in this frame; the session receives it as an intent.
        let mut gesture = self.session.gesture.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root_ui, |ui| {
                stage_renderer_maintenance(ui, ui.max_rect(), frame_number, generation);
                let mut surface = PaneSurface::new(
                    PaneReadModel {
                        document: &self.document,
                        cameras: &self.session.cameras,
                        active_tools: self.session.active_tools.clone(),
                        selected_result: self.session.selected_result,
                        selected_annotation: self.session.selected_annotation,
                        display: self.display.clone(),
                        diagnostics: &self.diagnostics,
                        generation,
                        frame_number,
                        active_pane: self.workspace.active_pane,
                        tokens,
                        font_scale: self.preferences.font_scale,
                    },
                    PaneFeatureState {
                        annotation_ui: AnnotationUiState::new(&mut gesture),
                        ui_behaviour: &mut view.ui_behaviour,
                        virtualisation: &mut view.virtualisation,
                        thumbnail_cache: &mut view.thumbnail_cache,
                        outputs: &mut outputs,
                    },
                );
                frame.dock(ui, &self.workspace, &mut surface);
            });
        view.ui_behaviour
            .finish_camera_gestures(Instant::now(), &mut outputs);
        outputs.finalise_camera_previews(
            root_ui,
            &view.ui_behaviour,
            &self.session.cameras,
            generation,
        );
        if let Err(error) = submit_render_plan(&outputs.render_plan, &outputs.render_targets) {
            tracing::error!(%error, "render plan submission rejected");
            frame.emit(Intent::Status(format!("Render plan rejected: {error}")));
            outputs.render_plan.images.clear();
        }
        view.render_cameras = outputs
            .render_plan
            .images
            .iter()
            .map(|request| CameraState {
                pane: request.pane,
                camera: request.camera,
                link: self
                    .session
                    .cameras
                    .iter()
                    .find(|state| state.pane == request.pane)
                    .and_then(|state| state.link),
            })
            .collect();
        view.visible_tile_keys = outputs
            .demands
            .iter()
            .filter(|demand| demand.priority == DemandPriority::Visible)
            .map(|demand| demand.key)
            .collect();
        view.visible_tile_keys.sort();
        view.visible_tile_keys.dedup();
        // The panes clip and virtualise what they record; publish it as presented.
        outputs.ui_geometry.observe(root_ui);
        if gesture != self.session.gesture {
            frame.emit(Intent::Gesture(gesture));
        }
        for intent in outputs.pane_intents.drain(..) {
            frame.emit(Intent::Pane(intent));
        }
        for command in outputs.commands.drain(..) {
            frame.emit(Intent::Command(command));
        }
        for intent in outputs.intents.drain(..) {
            frame.emit(Intent::Image(intent));
        }
        frame.emit(Intent::Demands(std::mem::take(&mut outputs.demands)));
        for target in outputs.triggers.drain(..) {
            frame.trigger(target);
        }
        view.interaction_active = outputs.interaction_active;
        if outputs.interaction_active {
            frame.request_repaint("interaction");
        }
        if let Some(delay) = outputs.repaint_after {
            frame.request_repaint_after("camera gesture idle", delay);
        }
        if self.render_bridge.snapshot().pending_upload_bytes > 0
            || self.pending_renderer_upload.is_some()
        {
            frame.request_repaint(PENDING_UPLOAD);
        }
        view.geometry = outputs.ui_geometry;
        view.ui_ms = ui_started.elapsed().as_secs_f64() * 1000.0;
    }

    fn apply(&mut self, intent: Intent, _view: &mut LabView) -> Result<bool, String> {
        let _span = info_span!("command_dispatch").entered();
        let reason = match intent {
            Intent::Gesture(gesture) => {
                // This pass already presented the preview.
                self.session.gesture = gesture;
                return Ok(false);
            }
            Intent::Pane(intent) => {
                apply_pane_intent(intent, &self.document, &mut self.session, &mut self.display)?;
                RepaintReason::Interaction
            }
            Intent::Command(command) => {
                self.execute(LabCommand::Image(command))?;
                RepaintReason::Command
            }
            Intent::Image(intent) => {
                self.execute_image_intent(intent)?;
                RepaintReason::Command
            }
            Intent::Layout(change @ WorkspaceChange::Resize(_)) => {
                // A completed resize is one undo entry beside image commands.
                if let Some(command) = WorkspaceCommand::from_change(change, &self.workspace)? {
                    self.execute(LabCommand::Layout(command))?;
                }
                RepaintReason::Command
            }
            Intent::Layout(change) => {
                if !change.apply(&mut self.workspace)? {
                    return Ok(false);
                }
                RepaintReason::Command
            }
            Intent::Demands(demands) => {
                let started = Instant::now();
                self.runtime.reconcile(demands);
                self.diagnostics.frame.demand_ms = started.elapsed().as_secs_f64() * 1000.0;
                return Ok(false);
            }
            Intent::Preferences(preferences) => {
                // The host reapplies the design system in the next pass and
                // records that repaint as `Appearance`, counted then.
                self.preferences = preferences.validated();
                self.status = "Appearance preferences saved".into();
                self.save_requested = true;
                return Ok(true);
            }
            Intent::Status(status) => {
                if status == self.status {
                    return Ok(false);
                }
                self.status = status;
                RepaintReason::Command
            }
        };
        self.record_repaint(reason);
        Ok(true)
    }

    fn perform(
        &mut self,
        target: ActionTarget<LabAction>,
        view: &mut LabView,
    ) -> Result<bool, String> {
        let pane = target.pane;
        let image_pane = || pane.ok_or_else(|| "An image pane is required".to_owned());
        let changed = match target.action {
            LabAction::Undo => self.undo()?,
            LabAction::Redo => self.redo()?,
            LabAction::SaveLayout => {
                self.save_requested = true;
                self.status = "Workspace saved".into();
                true
            }
            LabAction::ResetWorkspace => {
                self.workspace = analytical_workspace();
                self.session = default_session();
                self.status = "Default workspace restored".into();
                true
            }
            LabAction::ToggleDiagnostics => {
                let toggled = toggle_diagnostics(&mut self.workspace);
                if toggled {
                    self.status = if self
                        .workspace
                        .closed_optional_panes
                        .contains(&DIAGNOSTICS_PANE)
                    {
                        "Diagnostics closed"
                    } else {
                        "Diagnostics reopened"
                    }
                    .into();
                    view.focus_panels = true;
                }
                toggled
            }
            LabAction::FitView => {
                let pane = image_pane()?;
                let rect = self
                    .last_ui_snapshot
                    .node(&SemanticUiId::viewport(pane))
                    .map(|node| node.rect)
                    .filter(|rect| rect.is_positive())
                    .ok_or("The image viewport is unavailable")?;
                self.execute_image_intent(crate::actions::fit_view_intent(pane, rect))?;
                true
            }
            LabAction::LinkViews => {
                let pane = image_pane()?;
                let linked = self
                    .session
                    .cameras
                    .iter()
                    .find(|state| state.pane == pane)
                    .ok_or("The camera is unavailable")?
                    .link
                    .is_some();
                self.execute_image_intent(ImageIntent::SetCameraLink {
                    pane,
                    link: (!linked).then_some(LinkGroupId(1)),
                })?;
                true
            }
            LabAction::NavigateTool | LabAction::PolygonTool | LabAction::EditVerticesTool => {
                let tool = match target.action {
                    LabAction::NavigateTool => ActiveTool::Navigate,
                    LabAction::PolygonTool => ActiveTool::Polygon,
                    _ => ActiveTool::EditVertex,
                };
                let pane = image_pane()?;
                let before = self.session.active_tools.get(&pane).copied();
                apply_pane_intent(
                    PaneIntent::SetActiveTool { pane, tool },
                    &self.document,
                    &mut self.session,
                    &mut self.display,
                )?;
                before != Some(tool)
            }
            LabAction::CommitPolygon => match self.session.gesture.take() {
                Some(GesturePreview::Polygon { layer, vertices }) => {
                    self.execute_image_intent(ImageIntent::CommitPolygon { layer, vertices })?;
                    true
                }
                other => {
                    self.session.gesture = other;
                    return Err("A polygon needs at least three vertices".into());
                }
            },
            LabAction::DeleteAnnotation => {
                let annotation = self
                    .session
                    .selected_annotation
                    .ok_or("No annotation is selected")?;
                self.execute_image_intent(ImageIntent::DeleteAnnotation { annotation })?;
                true
            }
            LabAction::RecenterPrimary => {
                let result = self
                    .session
                    .selected_result
                    .ok_or("No result is selected")?;
                self.execute_image_intent(ImageIntent::Recentre {
                    pane: PaneId(1),
                    centre: result_at(result.0).position,
                })?;
                true
            }
            // Menu anchors and the clipboard copy act in the UI pass.
            LabAction::ViewPanels
            | LabAction::AppearanceSettings
            | LabAction::DisplaySettings
            | LabAction::CopyDiagnostics => false,
        };
        if changed {
            self.record_repaint(RepaintReason::Command);
        }
        Ok(changed)
    }

    fn report_error(&mut self, message: String) {
        self.status = message;
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("undo_entries", self.history.undo_len());
        observation.fact("redo_entries", self.history.redo_len());
        observation.fact(
            "diagnostics_open",
            !self
                .workspace
                .closed_optional_panes
                .contains(&DIAGNOSTICS_PANE),
        );
        observation.fact("status", self.status.clone());
        for camera in &self.session.cameras {
            let pane = camera.pane.0;
            let state = camera.camera;
            observation.fact(
                format!("camera.{pane}.scale"),
                state.pixels_per_screen_point.to_string(),
            );
            observation.fact(
                format!("camera.{pane}.centre_x"),
                state.centre.x.to_string(),
            );
            observation.fact(
                format!("camera.{pane}.centre_y"),
                state.centre.y.to_string(),
            );
            observation.fact(format!("camera.{pane}.linked"), camera.link.is_some());
        }
        for (id, total, role) in [
            ("results", 1_000_000, UiRole::ResultRow),
            ("thumbnails", 100_000, UiRole::ThumbnailCell),
        ] {
            let observed = observation.snapshot().by_role(role).count();
            observation.collection(CollectionObservation {
                id: id.into(),
                total: Some(total),
                observed,
                offset: None,
                virtualised: true,
                complete: false,
            });
        }
    }

    fn completed(&mut self, pass: &CompletedPass<'_>, view: &LabView) {
        for reason in pass.repaint {
            self.record_repaint(repaint_reason(*reason));
        }
        self.diagnostics.frame.interaction_active =
            view.interaction_active || pass.repaint.contains(&HostRepaint::DockInteraction);
        self.last_ui_snapshot = pass.snapshot.clone();
        self.last_ui_geometry = view.geometry.with_observation(pass.snapshot);
        self.last_render_cameras = view.render_cameras.clone();
        self.last_visible_tile_keys = view.visible_tile_keys.clone();
        self.diagnostics.frame.ui_ms = view.ui_ms;
        self.update_diagnostics(view);
        #[cfg(not(target_arch = "wasm32"))]
        if pass
            .context
            .input(|input| input.key_pressed(egui::Key::F12))
            && let Err(error) = self.write_native_test_snapshot(view)
        {
            tracing::error!(%error, "native physical-test snapshot failed");
        }
        self.diagnostics.frame.cpu_frame_ms = self.frame_started.elapsed().as_secs_f64() * 1000.0;
        self.diagnostics
            .frame
            .cpu_frame_history_ms
            .push_back(self.diagnostics.frame.cpu_frame_ms);
        while self.diagnostics.frame.cpu_frame_history_ms.len() > 240 {
            self.diagnostics.frame.cpu_frame_history_ms.pop_front();
        }
        #[cfg(target_arch = "wasm32")]
        self.publish_browser_diagnostics();
        #[cfg(target_arch = "wasm32")]
        crate::startup_frame(
            self.diagnostics.render.draw_calls > 0
                && self.diagnostics.render.resident_texture_bytes > 0,
        );
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.persisted());
    }

    fn take_save_request(&mut self) -> bool {
        std::mem::take(&mut self.save_requested)
    }
}

fn stable_workspace_hash(workspace: &Workspace) -> u64 {
    serde_json::to_vec(workspace)
        .unwrap_or_default()
        .into_iter()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

fn default_display() -> BTreeMap<PaneId, DisplaySettings> {
    (1..=4)
        .map(|pane| {
            (
                PaneId(pane),
                DisplaySettings {
                    map: if pane == 4 {
                        DisplayMap::Threshold
                    } else if pane == 3 {
                        DisplayMap::Greyscale
                    } else {
                        DisplayMap::Viridis
                    },
                    ..Default::default()
                },
            )
        })
        .collect()
}

fn apply_pane_intent(
    intent: PaneIntent,
    document: &Document,
    session: &mut Session,
    display: &mut BTreeMap<PaneId, DisplaySettings>,
) -> Result<(), String> {
    match intent {
        PaneIntent::SetActiveTool { pane, tool } if (1..=4).contains(&pane.0) => {
            session.active_tools.insert(pane, tool);
        }
        PaneIntent::SelectAnnotation(annotation)
            if annotation.is_none()
                || annotation.is_some_and(|annotation| {
                    document
                        .annotations
                        .iter()
                        .any(|polygon| polygon.id == annotation)
                }) =>
        {
            session.selected_annotation = annotation;
        }
        PaneIntent::SetDisplay { pane, settings }
            if (1..=4).contains(&pane.0)
                && settings.window_low.is_finite()
                && settings.window_high.is_finite()
                && (0.0..settings.window_high).contains(&settings.window_low)
                && settings.window_high <= 1.0 =>
        {
            display.insert(pane, settings);
        }
        _ => return Err("Rejected invalid pane presentation intent".into()),
    }
    Ok(())
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod app_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MemoryStorage(BTreeMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }
        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn former_saved_workspace_survives_load_save_and_restart() {
        let mut storage = MemoryStorage::default();
        let mut state = persisted(analytical_workspace());
        state.preferences.font_scale = 1.25;
        eframe::set_value(&mut storage, LEGACY_STORAGE_KEY, &state);
        let legacy = storage.0[LEGACY_STORAGE_KEY].clone();
        let restored = load_persisted(&storage).unwrap();
        assert_eq!(restored.preferences.font_scale, 1.25);
        eframe::set_value(&mut storage, STORAGE_KEY, &restored);
        assert_eq!(
            load_persisted(&storage).unwrap().preferences.font_scale,
            1.25
        );
        assert_eq!(storage.0[LEGACY_STORAGE_KEY], legacy);

        state.preferences.font_scale = 1.5;
        eframe::set_value(&mut storage, STORAGE_KEY, &state);
        assert_eq!(
            load_persisted(&storage).unwrap().preferences.font_scale,
            1.5
        );
        storage.0.insert(STORAGE_KEY.into(), "malformed".into());
        assert!(load_persisted(&storage).is_none());
        state.schema_version += 1;
        eframe::set_value(&mut storage, STORAGE_KEY, &state);
        assert!(load_persisted(&storage).is_none());
        storage.0.remove(STORAGE_KEY);
        storage
            .0
            .insert(LEGACY_STORAGE_KEY.into(), "malformed".into());
        assert!(load_persisted(&storage).is_none());
    }

    fn rich_persisted() -> PersistedState {
        let mut workspace = analytical_workspace();
        workspace.root.set_split_fraction(DockNodeId(1), 0.6);
        assert!(toggle_diagnostics(&mut workspace));
        let mut state = persisted(workspace);
        state.preferences.font_scale = 1.25;
        state.document.annotations.push(Polygon {
            id: AnnotationId(0),
            layer: LayerId(1),
            vertices: vec![
                WorldPoint::new(10.0, 20.0),
                WorldPoint::new(30.5, 20.0),
                WorldPoint::new(10.0, 40.25),
            ],
        });
        state.document.next_annotation_id = 1;
        state.session.selected_result = Some(ResultId(734_219));
        state.session.selected_annotation = Some(AnnotationId(0));
        state.session.cameras[2].link = Some(LinkGroupId(1));
        state.session.cameras[3].camera = Camera {
            centre: ImagePoint::new(8_192.0, 4_096.5),
            pixels_per_screen_point: 3.0,
        };
        state
            .session
            .active_tools
            .insert(PaneId(2), ActiveTool::Polygon);
        state.display.insert(
            PaneId(4),
            DisplaySettingsDto {
                low: 0.1,
                high: 0.9,
                map: 2,
            },
        );
        state
    }

    /// The saved state as stored before the image domain left core. Moving
    /// types between crates must not change this format.
    const SAVED_STATE_V2: &str = include_str!("../tests/fixtures/saved-state-v2.ron");

    #[test]
    fn saved_state_fixture_loads_and_reencodes_unchanged() {
        let fixture = SAVED_STATE_V2.trim_end();
        let mut storage = MemoryStorage::default();
        storage.0.insert(STORAGE_KEY.into(), fixture.into());
        let restored = load_persisted(&storage).expect("the fixture is a valid saved state");
        let expected = rich_persisted();
        assert_eq!(restored.workspace, expected.workspace);
        assert_eq!(restored.document, expected.document);
        assert_eq!(restored.session, expected.session);
        assert_eq!(restored.preferences, expected.preferences);
        eframe::set_value(&mut storage, STORAGE_KEY, &restored);
        assert_eq!(storage.0[STORAGE_KEY], fixture);
        eframe::set_value(&mut storage, STORAGE_KEY, &expected);
        assert_eq!(storage.0[STORAGE_KEY], fixture);
    }

    fn persisted(workspace: Workspace) -> PersistedState {
        PersistedState {
            schema_version: LAYOUT_SCHEMA_VERSION,
            preferences: UiPreferences::default(),
            workspace,
            document: Document::default(),
            session: default_session(),
            display: default_display()
                .into_iter()
                .map(|(pane, settings)| (pane, settings.into()))
                .collect(),
        }
    }

    #[test]
    fn restoration_rejects_a_structurally_valid_unknown_pane() {
        let mut workspace = analytical_workspace();
        workspace.root = DockNode::Tabs {
            id: DockNodeId(1),
            tabs: vec![PaneId(99)],
            active: 0,
        };
        workspace.active_pane = PaneId(99);
        workspace.next_node_id = 2;
        assert!(workspace.validate().is_ok());
        assert!(!persisted_state_is_valid(&persisted(workspace)));
    }

    #[test]
    fn restoration_rejects_missing_registered_panes() {
        let mut workspace = analytical_workspace();
        workspace.root = DockNode::Tabs {
            id: DockNodeId(1),
            tabs: vec![PaneId(1)],
            active: 0,
        };
        workspace.active_pane = PaneId(1);
        workspace.next_node_id = 2;
        assert!(workspace.validate().is_ok());
        assert!(!persisted_state_is_valid(&persisted(workspace)));
    }

    #[test]
    fn restoration_rejects_dangling_selection_and_invalid_display() {
        let mut state = persisted(analytical_workspace());
        state.session.selected_annotation = Some(AnnotationId(42));
        assert!(!persisted_state_is_valid(&state));
        state.session.selected_annotation = None;
        state.display.get_mut(&PaneId(1)).unwrap().high = f32::NAN;
        assert!(!persisted_state_is_valid(&state));
    }

    #[test]
    fn persisted_state_without_preferences_migrates_to_safe_defaults() {
        let mut value = serde_json::to_value(persisted(analytical_workspace())).unwrap();
        value.as_object_mut().unwrap().remove("preferences");
        let restored: PersistedState = serde_json::from_value(value).unwrap();
        assert_eq!(restored.preferences, UiPreferences::default());
        assert!(persisted_state_is_valid(&restored));
    }

    #[test]
    fn closed_diagnostics_restores_without_losing_analytical_state() {
        let mut state = persisted(analytical_workspace());
        state.document.annotations.push(Polygon {
            id: AnnotationId(7),
            layer: LayerId(1),
            vertices: vec![
                WorldPoint::new(0.0, 0.0),
                WorldPoint::new(1.0, 0.0),
                WorldPoint::new(0.0, 1.0),
            ],
        });
        state.session.selected_annotation = Some(AnnotationId(7));
        state.session.selected_result = Some(ResultId(42));
        state.session.cameras[0].camera.centre = ImagePoint::new(101.0, 202.0);
        let original_document = state.document.clone();
        let original_session = state.session.clone();
        let original_display = serde_json::to_value(&state.display).unwrap();

        assert!(persisted_state_is_valid(&state));
        assert!(toggle_diagnostics(&mut state.workspace));
        assert!(persisted_state_is_valid(&state));
        let mut restored: PersistedState =
            serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
        assert!(persisted_state_is_valid(&restored));
        assert!(
            restored
                .workspace
                .closed_optional_panes
                .contains(&DIAGNOSTICS_PANE)
        );
        assert_eq!(restored.document, original_document);
        assert_eq!(restored.session, original_session);
        assert_eq!(
            serde_json::to_value(&restored.display).unwrap(),
            original_display
        );

        assert!(toggle_diagnostics(&mut restored.workspace));
        assert!(persisted_state_is_valid(&restored));
        assert!(
            !restored
                .workspace
                .closed_optional_panes
                .contains(&DIAGNOSTICS_PANE)
        );
    }

    #[test]
    fn restoration_rejects_unknown_closed_panes() {
        let mut state = persisted(analytical_workspace());
        state.workspace.closed_optional_panes.insert(PaneId(99));
        assert!(!persisted_state_is_valid(&state));
    }

    #[test]
    fn pane_intents_reduce_authoritative_tool_selection_and_display_state() {
        let mut document = Document::default();
        document.annotations.push(Polygon {
            id: AnnotationId(7),
            layer: LayerId(1),
            vertices: vec![
                WorldPoint::new(0.0, 0.0),
                WorldPoint::new(1.0, 0.0),
                WorldPoint::new(0.0, 1.0),
            ],
        });
        let mut session = default_session();
        let mut display = default_display();
        let settings = DisplaySettings {
            window_low: 0.2,
            window_high: 0.7,
            map: DisplayMap::Threshold,
        };

        apply_pane_intent(
            PaneIntent::SetActiveTool {
                pane: PaneId(1),
                tool: ActiveTool::EditVertex,
            },
            &document,
            &mut session,
            &mut display,
        )
        .unwrap();
        apply_pane_intent(
            PaneIntent::SelectAnnotation(Some(AnnotationId(7))),
            &document,
            &mut session,
            &mut display,
        )
        .unwrap();
        apply_pane_intent(
            PaneIntent::SetDisplay {
                pane: PaneId(1),
                settings,
            },
            &document,
            &mut session,
            &mut display,
        )
        .unwrap();

        assert_eq!(
            session.active_tools.get(&PaneId(1)),
            Some(&ActiveTool::EditVertex)
        );
        assert_eq!(session.selected_annotation, Some(AnnotationId(7)));
        assert_eq!(display.get(&PaneId(1)), Some(&settings));
    }

    #[test]
    fn invalid_pane_intents_do_not_mutate_authoritative_state() {
        let document = Document::default();
        let mut session = default_session();
        let original_session = session.clone();
        let mut display = default_display();
        let original_display = display.clone();

        for intent in [
            PaneIntent::SetActiveTool {
                pane: PaneId(99),
                tool: ActiveTool::Polygon,
            },
            PaneIntent::SelectAnnotation(Some(AnnotationId(99))),
            PaneIntent::SetDisplay {
                pane: PaneId(1),
                settings: DisplaySettings {
                    window_low: 0.9,
                    window_high: 0.1,
                    map: DisplayMap::Viridis,
                },
            },
        ] {
            assert!(apply_pane_intent(intent, &document, &mut session, &mut display).is_err());
        }
        assert_eq!(session.active_tools, original_session.active_tools);
        assert_eq!(
            session.selected_annotation,
            original_session.selected_annotation
        );
        assert_eq!(display, original_display);
    }
}
