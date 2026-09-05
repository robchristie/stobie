#![allow(clippy::collapsible_if)] // Keep lifecycle actions grouped in the immediate UI flow.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use polyorama_core::*;
use polyorama_render_wgpu::{
    DisplayMap, DisplaySettings, RenderBridge, ScalarRenderer, UploadAdmission,
};
use polyorama_runtime::{DEFAULT_CACHE_BUDGET, DEFAULT_UPLOAD_BUDGET, DecodeEvent, Runtime};
use polyorama_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionTarget, DockBehaviour,
    DockTextContext, SemanticUiId, UiNode, UiPreferences, UiRole, UiSnapshot, action_button,
    application_bar_frame, application_bar_height, apply_design_system, audit_text_layouts,
    consume_action_shortcut, dock_workspace, measured_inline_label, preferences_control,
    stage_renderer_maintenance, submit_render_plan,
};
use serde::{Deserialize, Serialize};
use tracing::info_span;
use web_time::Instant;

use crate::{
    APPLICATION_NAME,
    actions::{ActionContext, LabAction, availability},
    panes::{
        AnnotationUiState, FrameOutput, PaneFeatureState, PaneIntent, PaneReadModel, PaneSurface,
        UiBehaviour, should_cancel_camera_drag,
    },
    thumbnail_cache::ThumbnailCache,
    ui_geometry::UiGeometry,
};

const STORAGE_KEY: &str = "polyorama.vertical-slice.v2";

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
        appearance: polyorama_ui_egui::AppearancePreference,
        contrast: polyorama_ui_egui::ContrastPreference,
        density: polyorama_ui_egui::DensityPreference,
        font_scale: f32,
        motion: polyorama_ui_egui::MotionPreference,
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
        || state.session.validate_image_cameras().is_err()
    {
        return false;
    }
    let expected_panes: BTreeSet<_> = (1..=8).map(PaneId).collect();
    let mut panes = Vec::new();
    state.workspace.root.pane_ids(&mut panes);
    if panes.into_iter().collect::<BTreeSet<_>>() != expected_panes
        || !state.workspace.closed_optional_panes.is_empty()
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

pub struct AnalyticalWorkspaceApp {
    workspace: Workspace,
    document: Document,
    session: Session,
    history: CommandHistory,
    runtime: Runtime,
    render_bridge: RenderBridge,
    dock_behaviour: DockBehaviour,
    ui_behaviour: UiBehaviour,
    display: BTreeMap<PaneId, DisplaySettings>,
    diagnostics: DiagnosticsSnapshot,
    frame_started: Instant,
    thumbnail_cache: ThumbnailCache,
    pending_renderer_upload: Option<DecodeEvent>,
    status: String,
    preferences: UiPreferences,
    #[cfg(target_arch = "wasm32")]
    egui_context: egui::Context,
    last_render_cameras: Vec<CameraState>,
    last_visible_tile_keys: Vec<TileKey>,
    last_ui_geometry: UiGeometry,
    last_ui_snapshot: UiSnapshot,
    #[cfg(target_arch = "wasm32")]
    browser_worker: Option<crate::web_worker::BrowserWorker>,
}

impl AnalyticalWorkspaceApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let persisted = cc
            .storage
            .and_then(|storage| eframe::get_value::<PersistedState>(storage, STORAGE_KEY))
            .filter(persisted_state_is_valid);
        let preferences = persisted
            .as_ref()
            .map_or_else(UiPreferences::default, |saved| {
                saved.preferences.validated()
            });
        apply_design_system(&cc.egui_ctx, preferences);
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
                Workspace::analytical_default(),
                Document::default(),
                Session::default(),
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
        #[cfg(not(target_arch = "wasm32"))]
        let mut runtime = Runtime::default();
        #[cfg(target_arch = "wasm32")]
        let mut runtime = Runtime::default();
        #[cfg(target_arch = "wasm32")]
        let browser_worker = match crate::web_worker::BrowserWorker::new(cc.egui_ctx.clone()) {
            Ok(worker) => Some(worker),
            Err(error) => {
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
            history: CommandHistory::default(),
            runtime,
            render_bridge,
            dock_behaviour: DockBehaviour::default(),
            ui_behaviour: UiBehaviour::default(),
            display,
            diagnostics,
            frame_started: Instant::now(),
            thumbnail_cache: ThumbnailCache::default(),
            pending_renderer_upload: None,
            status: "Initialising progressive data…".into(),
            preferences,
            #[cfg(target_arch = "wasm32")]
            egui_context: cc.egui_ctx.clone(),
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

    #[cfg(target_arch = "wasm32")]
    pub(crate) fn test_action(&mut self, action: TestAction) -> Result<TestSnapshot, String> {
        let command = match action {
            TestAction::SetCamera {
                pane,
                centre_x,
                centre_y,
                pixels_per_screen_point,
            } => validate_intent(
                ImageIntent::SetCamera {
                    pane: PaneId(pane),
                    camera: Camera {
                        centre: ImagePoint::new(centre_x, centre_y),
                        pixels_per_screen_point,
                    },
                },
                &mut self.document,
                &self.session,
            )?,
            TestAction::CommitPolygon { vertices } => validate_intent(
                ImageIntent::CommitPolygon {
                    layer: LayerId(1),
                    vertices: vertices
                        .into_iter()
                        .map(|(x, y)| WorldPoint::new(x, y))
                        .collect(),
                },
                &mut self.document,
                &self.session,
            )?,
            TestAction::Undo => {
                if !self
                    .history
                    .undo(&mut self.document, &mut self.session, &mut self.workspace)
                {
                    return Err("nothing to undo".into());
                }
                let context = self.egui_context.clone();
                self.request_repaint(&context, RepaintReason::Command);
                return Ok(self.test_snapshot());
            }
            TestAction::ResizeSplit { node, fraction } => {
                let node = DockNodeId(node);
                let before = self
                    .workspace
                    .root
                    .split_fraction(node)
                    .ok_or("unknown split node")?;
                Command::ResizeSplit {
                    node,
                    before,
                    after: fraction,
                }
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
                let context = self.egui_context.clone();
                self.request_repaint(&context, RepaintReason::PendingUpload);
                return Ok(self.test_snapshot());
            }
            TestAction::RestoreDefaultWorkspace => {
                self.workspace = Workspace::analytical_default();
                let context = self.egui_context.clone();
                self.request_repaint(&context, RepaintReason::Command);
                return Ok(self.test_snapshot());
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
                let context = self.egui_context.clone();
                self.request_repaint(&context, RepaintReason::WorkerCompletion);
                return Ok(self.test_snapshot());
            }
            TestAction::SetUiPreferences {
                appearance,
                contrast,
                density,
                font_scale,
                motion,
            } => {
                self.preferences = UiPreferences {
                    appearance,
                    contrast,
                    density,
                    font_scale,
                    motion,
                    ..self.preferences
                }
                .validated();
                let context = self.egui_context.clone();
                apply_design_system(&context, self.preferences);
                self.request_repaint(&context, RepaintReason::Preferences);
                return Ok(self.test_snapshot());
            }
        };
        self.history.execute(
            command,
            &mut self.document,
            &mut self.session,
            &mut self.workspace,
        );
        let context = self.egui_context.clone();
        self.request_repaint(&context, RepaintReason::Command);
        Ok(self.test_snapshot())
    }

    pub(crate) fn test_snapshot(&self) -> TestSnapshot {
        let mut visible_panes = Vec::new();
        self.workspace.root.active_panes(&mut visible_panes);
        let render_cameras = self.last_render_cameras.clone();
        let visible_tile_keys = self.last_visible_tile_keys.clone();
        TestSnapshot {
            cameras: self.session.cameras.clone(),
            render_cameras,
            visible_tile_keys,
            annotations: self.document.annotations.clone(),
            annotation_count: self.document.annotations.len(),
            selected_annotation: self.session.selected_annotation,
            undo_depth: self.history.undo_len(),
            workspace_hash: format!("{:016x}", stable_workspace_hash(&self.workspace)),
            thumbnail_resident_keys: self.thumbnail_cache.keys(),
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
    fn write_native_test_snapshot(&self) -> Result<(), String> {
        let path = std::env::var("POLYORAMA_TEST_SNAPSHOT_PATH")
            .map_err(|_| "POLYORAMA_TEST_SNAPSHOT_PATH is not configured".to_owned())?;
        let json = serde_json::to_string_pretty(&self.test_snapshot())
            .map_err(|error| format!("native test snapshot could not be serialised: {error}"))?;
        std::fs::write(&path, json).map_err(|error| {
            format!("native test snapshot could not be written to {path}: {error}")
        })
    }

    fn request_repaint(&mut self, ctx: &egui::Context, reason: RepaintReason) {
        self.diagnostics.frame.repaint_requests += 1;
        self.diagnostics.frame.repaint_reason = reason.clone();
        self.diagnostics.frame.recent_reasons.push_front(reason);
        self.diagnostics.frame.recent_reasons.truncate(8);
        ctx.request_repaint();
    }

    fn request_repaint_after(
        &mut self,
        ctx: &egui::Context,
        duration: std::time::Duration,
        reason: RepaintReason,
    ) {
        self.diagnostics.frame.repaint_requests += 1;
        self.diagnostics.frame.repaint_reason = reason.clone();
        self.diagnostics.frame.recent_reasons.push_front(reason);
        self.diagnostics.frame.recent_reasons.truncate(8);
        ctx.request_repaint_after(duration);
    }

    fn poll_runtime(&mut self, ctx: &egui::Context) {
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
            self.request_repaint(ctx, RepaintReason::WorkerCompletion);
        }
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
                    match self.thumbnail_cache.insert(ctx, key, token, &scalar_u16_le) {
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
        if self.thumbnail_cache.len() > 0 || self.diagnostics.render.resident_texture_bytes > 0 {
            self.status = "Workspace ready".into();
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(worker) = &self.browser_worker {
            while let Some(request) = self.runtime.take_external_request() {
                if worker.submit(&request).is_err() {
                    self.runtime.record_browser_transport_failure(
                        request,
                        "browser Worker postMessage failed",
                    );
                }
            }
        }
        self.diagnostics.frame.runtime_poll_ms = started.elapsed().as_secs_f64() * 1000.0;
    }

    fn apply_outputs(&mut self, ctx: &egui::Context, outputs: FrameOutput) {
        let command_count = outputs.commands.len();
        let intent_count = outputs.intents.len() + outputs.pane_intents.len();
        let _span = info_span!("command_dispatch", command_count, intent_count).entered();
        let mut applied_commands = 0;
        let mut applied_pane_intents = 0;
        for intent in outputs.pane_intents {
            match apply_pane_intent(intent, &self.document, &mut self.session, &mut self.display) {
                Ok(()) => applied_pane_intents += 1,
                Err(error) => self.status = error,
            }
        }
        for command in outputs.commands {
            self.history.execute(
                command,
                &mut self.document,
                &mut self.session,
                &mut self.workspace,
            );
            applied_commands += 1;
        }
        for intent in outputs.intents {
            match validate_intent(intent, &mut self.document, &self.session) {
                Ok(command) => {
                    self.history.execute(
                        command,
                        &mut self.document,
                        &mut self.session,
                        &mut self.workspace,
                    );
                    applied_commands += 1;
                }
                Err(error) => self.status = error,
            }
        }
        let demand_started = Instant::now();
        self.runtime.reconcile(outputs.demands);
        self.diagnostics.frame.demand_ms = demand_started.elapsed().as_secs_f64() * 1000.0;
        if applied_commands > 0 {
            self.request_repaint(ctx, RepaintReason::Command);
        }
        if applied_pane_intents > 0 {
            self.request_repaint(ctx, RepaintReason::Interaction);
        }
        if self.render_bridge.snapshot().pending_upload_bytes > 0
            || self.pending_renderer_upload.is_some()
        {
            self.request_repaint(ctx, RepaintReason::PendingUpload);
        }
        self.diagnostics.frame.interaction_active = outputs.interaction_active;
        if outputs.interaction_active {
            self.request_repaint(ctx, RepaintReason::Interaction);
        }
        if let Some(duration) = outputs.repaint_after {
            self.request_repaint_after(ctx, duration, RepaintReason::Scheduled);
        }
    }

    fn update_diagnostics(&mut self) {
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
        self.diagnostics.virtualisation.resident_thumbnails = self.thumbnail_cache.len();
        self.diagnostics.virtualisation.thumbnail_cache_bytes = self.thumbnail_cache.used();
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
            &JsValue::from_str("__POLYORAMA_DIAGNOSTICS"),
            &snapshot,
        );
        if let Some(document) = window.document() {
            if let Some(canvas) = document.get_element_by_id("polyorama-canvas") {
                let _ = canvas.set_attribute("data-polyorama-ready", "true");
                let _ = canvas.set_attribute(
                    "data-worker-completions",
                    &self.runtime.metrics.completed.to_string(),
                );
                let _ = canvas.set_attribute("data-pane-count", "8");
                let _ = canvas.set_attribute("data-renderer", "wgpu-scalar");
            }
        }
    }
}

impl eframe::App for AnalyticalWorkspaceApp {
    fn ui(&mut self, root_ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        let _span = info_span!(
            "frame_processing",
            frame = self.diagnostics.frame.frame_number
        )
        .entered();
        self.frame_started = Instant::now();
        self.diagnostics.frame.frame_number += 1;
        self.diagnostics.frame.repaint_reason = RepaintReason::None;
        self.diagnostics.frame.physical_wheel_events += root_ui.input(|input| {
            input
                .events
                .iter()
                .filter(|event| matches!(event, egui::Event::MouseWheel { .. }))
                .count() as u64
        });
        let cancel_camera_drag = root_ui.input(should_cancel_camera_drag);
        if cancel_camera_drag {
            self.ui_behaviour.cancel_camera_drags();
        }
        self.ui_behaviour.begin_frame();
        self.poll_runtime(&ctx);
        let system_dark = ctx
            .system_theme()
            .is_none_or(|theme| theme == egui::Theme::Dark);
        let tokens = self.preferences.tokens(system_dark);
        let mut save_now = false;
        let mut preferences_changed = false;
        let mut ui_geometry = UiGeometry::new(root_ui.max_rect(), ctx.pixels_per_point());
        let application_bar_id = SemanticUiId::new("application.bar");
        let action_context = ActionContext {
            undo_depth: self.history.undo_len(),
            redo_depth: self.history.redo_len(),
            active_pane: self.workspace.active_pane,
            ..Default::default()
        };
        let menu = egui::Panel::top("application-menu")
            .frame(application_bar_frame(&tokens))
            .exact_size(application_bar_height(&tokens, self.preferences.font_scale))
            .show(root_ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let compact_bar = ui.available_width()
                        < tokens.geometry.minimum_hit_size.0 * 34.0
                        || self.preferences.font_scale > 1.25;
                    measured_inline_label(
                        ui,
                        1,
                        APPLICATION_NAME,
                        polyorama_ui_egui::TextRole::ApplicationTitle,
                        tokens.geometry.minimum_hit_size.0 * if compact_bar { 4.0 } else { 6.0 },
                        polyorama_ui_egui::TextInteraction::Inert,
                        &tokens,
                        self.preferences.font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui.separator();
                    let undo_availability = availability(LabAction::Undo, action_context);
                    let undo_target = ActionTarget::application(LabAction::Undo);
                    let undo = action_button(
                        ui,
                        ActionButtonSpec {
                            target: undo_target,
                            availability: undo_availability.clone(),
                            state: ActionButtonState::Momentary,
                            emphasis: ActionEmphasis::Quiet,
                            compact: compact_bar,
                        },
                        &tokens,
                        self.preferences.font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui_geometry.action(
                        application_bar_id.clone(),
                        undo_target,
                        &undo_availability,
                        ActionButtonState::Momentary,
                        &undo,
                    );
                    if undo_availability.enabled()
                        && (undo.clicked() || consume_action_shortcut(ui, LabAction::Undo, true))
                        && self.history.undo(
                            &mut self.document,
                            &mut self.session,
                            &mut self.workspace,
                        )
                    {
                        self.request_repaint(&ctx, RepaintReason::Command);
                    }
                    let redo_availability = availability(LabAction::Redo, action_context);
                    let redo_target = ActionTarget::application(LabAction::Redo);
                    let redo = action_button(
                        ui,
                        ActionButtonSpec {
                            target: redo_target,
                            availability: redo_availability.clone(),
                            state: ActionButtonState::Momentary,
                            emphasis: ActionEmphasis::Quiet,
                            compact: compact_bar,
                        },
                        &tokens,
                        self.preferences.font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui_geometry.action(
                        application_bar_id.clone(),
                        redo_target,
                        &redo_availability,
                        ActionButtonState::Momentary,
                        &redo,
                    );
                    if redo_availability.enabled()
                        && (redo.clicked() || consume_action_shortcut(ui, LabAction::Redo, true))
                        && self.history.redo(
                            &mut self.document,
                            &mut self.session,
                            &mut self.workspace,
                        )
                    {
                        self.request_repaint(&ctx, RepaintReason::Command);
                    }
                    let save_availability = availability(LabAction::SaveLayout, action_context);
                    let save_target = ActionTarget::application(LabAction::SaveLayout);
                    let save = action_button(
                        ui,
                        ActionButtonSpec {
                            target: save_target,
                            availability: save_availability.clone(),
                            state: ActionButtonState::Momentary,
                            emphasis: ActionEmphasis::Quiet,
                            compact: compact_bar,
                        },
                        &tokens,
                        self.preferences.font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui_geometry.action(
                        application_bar_id.clone(),
                        save_target,
                        &save_availability,
                        ActionButtonState::Momentary,
                        &save,
                    );
                    if save_availability.enabled()
                        && (save.clicked()
                            || consume_action_shortcut(ui, LabAction::SaveLayout, true))
                    {
                        save_now = true;
                        self.status = "Workspace saved".into();
                    }
                    let reset_availability =
                        availability(LabAction::ResetWorkspace, action_context);
                    let reset_target = ActionTarget::application(LabAction::ResetWorkspace);
                    let reset = action_button(
                        ui,
                        ActionButtonSpec {
                            target: reset_target,
                            availability: reset_availability.clone(),
                            state: ActionButtonState::Momentary,
                            emphasis: ActionEmphasis::Quiet,
                            compact: compact_bar,
                        },
                        &tokens,
                        self.preferences.font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui_geometry.action(
                        application_bar_id.clone(),
                        reset_target,
                        &reset_availability,
                        ActionButtonState::Momentary,
                        &reset,
                    );
                    if reset_availability.enabled() && reset.clicked() {
                        self.workspace = Workspace::analytical_default();
                        self.session = Session::default();
                        self.status = "Default workspace restored".into();
                        self.request_repaint(&ctx, RepaintReason::Command);
                    }
                    let appearance_availability =
                        availability(LabAction::AppearanceSettings, action_context);
                    let appearance_target =
                        ActionTarget::application(LabAction::AppearanceSettings);
                    let appearance = action_button(
                        ui,
                        ActionButtonSpec {
                            target: appearance_target,
                            availability: appearance_availability.clone(),
                            state: ActionButtonState::Momentary,
                            emphasis: ActionEmphasis::Quiet,
                            compact: compact_bar,
                        },
                        &tokens,
                        self.preferences.font_scale,
                        &mut ui_geometry.text_layouts,
                    );
                    ui_geometry.action(
                        application_bar_id.clone(),
                        appearance_target,
                        &appearance_availability,
                        ActionButtonState::Momentary,
                        &appearance,
                    );
                    if let Some(popup) = egui::Popup::menu(&appearance).show(|ui| {
                        ui.set_width(tokens.geometry.minimum_hit_size.0 * 7.0);
                        preferences_control(
                            ui,
                            &mut self.preferences,
                            &application_bar_id,
                            LabAction::AppearanceSettings,
                        )
                    }) {
                        preferences_changed |= popup.inner.changed;
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
                            polyorama_ui_egui::TextRole::Status,
                            status_width,
                            polyorama_ui_egui::TextInteraction::Selectable,
                            &tokens,
                            self.preferences.font_scale,
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
                            self.thumbnail_cache.len(),
                            self.runtime.metrics.in_flight
                        );
                        let worker_width = ui.available_width();
                        let response = measured_inline_label(
                            ui,
                            3,
                            &worker_status,
                            polyorama_ui_egui::TextRole::Secondary,
                            worker_width,
                            polyorama_ui_egui::TextInteraction::Selectable,
                            &tokens,
                            self.preferences.font_scale,
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
        if preferences_changed {
            self.preferences = self.preferences.validated();
            apply_design_system(&ctx, self.preferences);
            self.status = "Appearance preferences saved".into();
            save_now = true;
            self.request_repaint(&ctx, RepaintReason::Preferences);
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
        let ui_started = Instant::now();
        let mut outputs = FrameOutput::with_ui_geometry(ui_geometry);
        let frame_number = self.diagnostics.frame.frame_number;
        let active_pane = self.workspace.active_pane;
        let diagnostics_view = self.diagnostics.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root_ui, |ui| {
                stage_renderer_maintenance(
                    ui,
                    ui.max_rect(),
                    frame_number,
                    self.runtime.generation(),
                );
                let mut surface = PaneSurface::new(
                    PaneReadModel {
                        document: &self.document,
                        cameras: &self.session.cameras,
                        active_tools: self.session.active_tools.clone(),
                        selected_result: self.session.selected_result,
                        selected_annotation: self.session.selected_annotation,
                        display: self.display.clone(),
                        diagnostics: &diagnostics_view,
                        generation: self.runtime.generation(),
                        frame_number,
                        active_pane,
                        tokens,
                        font_scale: self.preferences.font_scale,
                    },
                    PaneFeatureState {
                        annotation_ui: AnnotationUiState::new(&mut self.session.gesture),
                        ui_behaviour: &mut self.ui_behaviour,
                        virtualisation: &mut self.diagnostics.virtualisation,
                        thumbnail_cache: &mut self.thumbnail_cache,
                        outputs: &mut outputs,
                    },
                );
                if let Some(command) = dock_workspace(
                    ui,
                    &mut self.workspace,
                    &mut self.dock_behaviour,
                    &mut surface,
                    DockTextContext {
                        tokens,
                        font_scale: self.preferences.font_scale,
                    },
                ) {
                    surface.push_shell_command(command);
                }
                surface.record_shell_interaction(self.dock_behaviour.interaction_active());
            });
        self.ui_behaviour
            .finish_camera_gestures(Instant::now(), &mut outputs);
        outputs.finalise_camera_previews(
            root_ui,
            &self.ui_behaviour,
            &self.session.cameras,
            self.runtime.generation(),
        );
        if let Err(error) = submit_render_plan(&outputs.render_plan, &outputs.render_targets) {
            tracing::error!(%error, "render plan submission rejected");
            self.status = format!("Render plan rejected: {error}");
            outputs.render_plan.images.clear();
        }
        self.last_render_cameras = outputs
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
        self.last_visible_tile_keys = outputs
            .demands
            .iter()
            .filter(|demand| demand.priority == DemandPriority::Visible)
            .map(|demand| demand.key)
            .collect();
        self.last_visible_tile_keys.sort();
        self.last_visible_tile_keys.dedup();
        self.diagnostics.frame.ui_ms = ui_started.elapsed().as_secs_f64() * 1000.0;
        outputs.ui_geometry.text_audit = audit_text_layouts(&outputs.ui_geometry.text_layouts);
        outputs.ui_geometry.text_audit_coverage = Some(polyorama_ui_egui::text_audit_coverage(
            &ctx,
            &outputs.ui_geometry.text_layouts,
        ));
        self.last_ui_snapshot = outputs.ui_geometry.snapshot(frame_number);
        self.last_ui_geometry = outputs.ui_geometry.clone();
        self.apply_outputs(&ctx, outputs);
        self.update_diagnostics();
        #[cfg(not(target_arch = "wasm32"))]
        if root_ui.input(|input| input.key_pressed(egui::Key::F12))
            && let Err(error) = self.write_native_test_snapshot()
        {
            tracing::error!(%error, "native physical-test snapshot failed");
        }
        if save_now {
            if let Some(storage) = frame.storage_mut() {
                eframe::set_value(storage, STORAGE_KEY, &self.persisted());
                storage.flush();
            }
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
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.persisted());
    }

    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
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
mod tests {
    use super::*;

    fn persisted(workspace: Workspace) -> PersistedState {
        PersistedState {
            schema_version: LAYOUT_SCHEMA_VERSION,
            preferences: UiPreferences::default(),
            workspace,
            document: Document::default(),
            session: Session::default(),
            display: default_display()
                .into_iter()
                .map(|(pane, settings)| (pane, settings.into()))
                .collect(),
        }
    }

    #[test]
    fn restoration_rejects_a_structurally_valid_unknown_pane() {
        let mut workspace = Workspace::analytical_default();
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
        let mut workspace = Workspace::analytical_default();
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
        let mut state = persisted(Workspace::analytical_default());
        state.session.selected_annotation = Some(AnnotationId(42));
        assert!(!persisted_state_is_valid(&state));
        state.session.selected_annotation = None;
        state.display.get_mut(&PaneId(1)).unwrap().high = f32::NAN;
        assert!(!persisted_state_is_valid(&state));
    }

    #[test]
    fn persisted_state_without_preferences_migrates_to_safe_defaults() {
        let mut value = serde_json::to_value(persisted(Workspace::analytical_default())).unwrap();
        value.as_object_mut().unwrap().remove("preferences");
        let restored: PersistedState = serde_json::from_value(value).unwrap();
        assert_eq!(restored.preferences, UiPreferences::default());
        assert!(persisted_state_is_valid(&restored));
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
        let mut session = Session::default();
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
        let mut session = Session::default();
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
