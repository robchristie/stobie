#![allow(clippy::collapsible_if)] // Keep lifecycle actions grouped in the immediate UI flow.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use serde::{Deserialize, Serialize};
use tracing::info_span;
use web_time::Instant;
use workspace_core::*;
use workspace_render_wgpu::{DisplayMap, DisplaySettings, RenderBridge, ScalarRenderer};
use workspace_runtime::{DEFAULT_CACHE_BUDGET, DEFAULT_UPLOAD_BUDGET, DecodeEvent, Runtime};
use workspace_ui_egui::{DockBehaviour, dock_workspace};

use crate::{
    APPLICATION_NAME,
    panes::{PaneOutputs, PaneSurface, UiBehaviour},
};

const STORAGE_KEY: &str = "polyorama.vertical-slice.v2";

#[derive(Clone, Serialize, Deserialize)]
struct PersistedState {
    schema_version: u32,
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
    resident_tiles: BTreeSet<TileKey>,
    status: String,
    #[cfg(target_arch = "wasm32")]
    browser_worker: Option<crate::web_worker::BrowserWorker>,
}

impl AnalyticalWorkspaceApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_style(&cc.egui_ctx);
        let persisted = cc
            .storage
            .and_then(|storage| eframe::get_value::<PersistedState>(storage, STORAGE_KEY))
            .filter(|state| {
                state.schema_version == LAYOUT_SCHEMA_VERSION && state.workspace.validate().is_ok()
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
        #[cfg(target_arch = "wasm32")]
        let browser_worker = crate::web_worker::BrowserWorker::new(cc.egui_ctx.clone()).ok();
        #[cfg(not(target_arch = "wasm32"))]
        let mut runtime = Runtime::default();
        #[cfg(target_arch = "wasm32")]
        let runtime = Runtime::default();
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
            resident_tiles: BTreeSet::new(),
            status: "Initialising progressive data…".into(),
            #[cfg(target_arch = "wasm32")]
            browser_worker,
        }
    }

    fn persisted(&self) -> PersistedState {
        let _span = info_span!("layout_serialisation").entered();
        PersistedState {
            schema_version: LAYOUT_SCHEMA_VERSION,
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

    fn request_repaint(&mut self, ctx: &egui::Context, reason: RepaintReason) {
        self.diagnostics.frame.repaint_requests += 1;
        self.diagnostics.frame.repaint_reason = reason.clone();
        self.diagnostics.frame.recent_reasons.push_front(reason);
        self.diagnostics.frame.recent_reasons.truncate(8);
        ctx.request_repaint();
    }

    fn poll_runtime(&mut self, ctx: &egui::Context) {
        let started = Instant::now();
        let native_completed = self.runtime.poll();
        #[cfg(target_arch = "wasm32")]
        let browser_completed = {
            let events = self
                .browser_worker
                .as_ref()
                .map_or_else(Vec::new, |worker| worker.drain());
            let count = events.len();
            for event in events {
                self.runtime.accept_event(event);
            }
            count
        };
        #[cfg(not(target_arch = "wasm32"))]
        let browser_completed = 0;
        if native_completed + browser_completed > 0 {
            self.request_repaint(ctx, RepaintReason::WorkerCompletion);
        }
        while let Some(event) = self.runtime.pop_decoded() {
            match &event {
                DecodeEvent::Completed { key, .. } if key.source == SourceId(2) => {
                    self.resident_tiles.insert(*key);
                    self.runtime.mark_resident(*key);
                }
                _ => self.render_bridge.push(event),
            }
        }
        for key in self.render_bridge.take_resident() {
            self.resident_tiles.insert(key);
            self.runtime.mark_resident(key);
        }
        for key in self.render_bridge.take_evicted() {
            self.resident_tiles.remove(&key);
            self.runtime.mark_evicted(key);
        }
        if !self.resident_tiles.is_empty() {
            self.status = "Workspace ready".into();
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(worker) = &self.browser_worker {
            while let Some(request) = self.runtime.take_external_request() {
                if worker.submit(&request).is_err() {
                    self.runtime.accept_event(DecodeEvent::Failed {
                        key: request.key,
                        generation: request.generation,
                        message: "browser Worker postMessage failed".into(),
                    });
                }
            }
        }
        self.diagnostics.frame.runtime_poll_ms = started.elapsed().as_secs_f64() * 1000.0;
    }

    fn apply_outputs(&mut self, ctx: &egui::Context, outputs: PaneOutputs) {
        let command_count = outputs.commands.len();
        let intent_count = outputs.intents.len();
        let _span = info_span!("command_dispatch", command_count, intent_count).entered();
        let mut applied_commands = 0;
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
        if self.render_bridge.snapshot().pending_upload_bytes > 0 {
            self.request_repaint(ctx, RepaintReason::PendingUpload);
        }
        self.diagnostics.frame.interaction_active = outputs.interaction_active;
        if outputs.interaction_active {
            self.request_repaint(ctx, RepaintReason::Interaction);
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
        self.diagnostics.runtime.evictions = self.diagnostics.render.cache_evictions;
        self.diagnostics.frame.render_prepare_ms = self.diagnostics.render.prepare_ms;
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
        self.poll_runtime(&ctx);
        let mut save_now = false;
        egui::Panel::top("application-menu")
            .exact_size(38.0)
            .show(root_ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.strong(APPLICATION_NAME);
                    ui.separator();
                    if ui.button("Undo").clicked()
                        && self.history.undo(
                            &mut self.document,
                            &mut self.session,
                            &mut self.workspace,
                        )
                    {
                        self.request_repaint(&ctx, RepaintReason::Command);
                    }
                    if ui.button("Redo").clicked()
                        && self.history.redo(
                            &mut self.document,
                            &mut self.session,
                            &mut self.workspace,
                        )
                    {
                        self.request_repaint(&ctx, RepaintReason::Command);
                    }
                    if ui.button("Save layout").clicked() {
                        save_now = true;
                        self.status = "Workspace saved".into();
                    }
                    if ui.button("Reset workspace").clicked() {
                        self.workspace = Workspace::analytical_default();
                        self.session = Session::default();
                        self.status = "Default workspace restored".into();
                        self.request_repaint(&ctx, RepaintReason::Command);
                    }
                    ui.separator();
                    ui.label(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!(
                            "{} resident · {} decoding",
                            self.resident_tiles.len(),
                            self.runtime.metrics.in_flight
                        ));
                    });
                });
            });
        let ui_started = Instant::now();
        let mut outputs = PaneOutputs::default();
        let frame_number = self.diagnostics.frame.frame_number;
        let active_pane = self.workspace.active_pane;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root_ui, |ui| {
                let mut surface = PaneSurface {
                    document: &self.document,
                    session: &mut self.session,
                    ui_behaviour: &mut self.ui_behaviour,
                    display: &mut self.display,
                    diagnostics: &mut self.diagnostics,
                    resident_tiles: &self.resident_tiles,
                    generation: self.runtime.generation(),
                    frame_number,
                    active_pane,
                    outputs: &mut outputs,
                };
                if let Some(command) = dock_workspace(
                    ui,
                    &mut self.workspace,
                    &mut self.dock_behaviour,
                    &mut surface,
                ) {
                    surface.outputs.commands.push(command);
                }
            });
        self.diagnostics.frame.ui_ms = ui_started.elapsed().as_secs_f64() * 1000.0;
        self.apply_outputs(&ctx, outputs);
        self.update_diagnostics();
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

fn configure_style(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(19, 23, 27);
    visuals.extreme_bg_color = egui::Color32::from_rgb(11, 15, 18);
    visuals.selection.bg_fill = egui::Color32::from_rgb(26, 133, 145);
    visuals.selection.stroke.color = egui::Color32::from_rgb(112, 222, 210);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(3);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(9.0, 4.0);
    ctx.set_style_of(egui::Theme::Dark, style);
}
