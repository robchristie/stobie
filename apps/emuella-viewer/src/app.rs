use crate::{
    Executor,
    engine::{Event, Job, WorkerMetrics, pacing_now_ms, representation},
};
use emuella_viewer_source::Manifest;
use polyorama_core::{
    DemandPriority, ImageRegion, PaneId, RegionConsumerId, RegionDemand, RegionKey, SourceStage,
    layout_virtual_grid,
};
use polyorama_render_wgpu::{
    PixelRect, RegionalDisplaySettings, RegionalDraw, RegionalGpuLimits, RegionalRenderer,
};
use polyorama_runtime::{RegionalRuntime, RegionalRuntimeLimits};
use polyorama_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionScope, ActionSpec,
    ActionTarget, Availability, UiPreferences, action_button, apply_design_system,
};
use serde::{Deserialize, Serialize};
use web_time::{Duration, Instant};

const LOGICAL_DETECTIONS: usize = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewerAction {
    Fit,
    ZoomIn,
    ZoomOut,
    PanLeft,
    PanRight,
    NextImage,
    CompareImage,
    Bookmark,
    Recall,
    Retry,
    Diagnostics,
}
impl ActionKey for ViewerAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Fit => "fit",
            Self::ZoomIn => "zoom_in",
            Self::ZoomOut => "zoom_out",
            Self::PanLeft => "pan_left",
            Self::PanRight => "pan_right",
            Self::NextImage => "next_image",
            Self::CompareImage => "compare_image",
            Self::Bookmark => "bookmark",
            Self::Recall => "recall",
            Self::Retry => "retry",
            Self::Diagnostics => "diagnostics",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        let label = match self {
            Self::Fit => "Fit image",
            Self::ZoomIn => "Zoom in",
            Self::ZoomOut => "Zoom out",
            Self::PanLeft => "Pan left",
            Self::PanRight => "Pan right",
            Self::NextImage => "Next image",
            Self::CompareImage => "Compare image",
            Self::Bookmark => "Bookmark",
            Self::Recall => "Recall",
            Self::Retry => "Retry",
            Self::Diagnostics => "Diagnostics",
        };
        ActionSpec {
            id: self,
            label,
            compact_label: None,
            description: label,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Intent {
    Action {
        action: ViewerAction,
    },
    SelectImage {
        index: usize,
    },
    Pan {
        dx: f32,
        dy: f32,
    },
    Zoom {
        factor: f32,
    },
    Gallery {
        row: usize,
    },
    OpenDetection {
        index: usize,
    },
    Stretch {
        low: f32,
        high: f32,
        gamma: f32,
    },
    /// Calibration-only reset; accepted only after every worker has stopped.
    ClearDisplayCache,
}
#[derive(Clone, Copy)]
struct Camera {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}
#[derive(Clone)]
struct Bookmark {
    image: usize,
    camera: Camera,
    detail: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkEvent {
    pub kind: String,
    pub at_ms: f64,
    pub key: RegionKey,
    pub token: polyorama_runtime::RequestToken,
    pub accounted_decoded_bytes: usize,
    #[serde(default)]
    pub pacing: Option<PacingSample>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PacingSample {
    pub dispatch_ms: f64,
    pub worker_started_ms: f64,
    pub worker_finished_ms: f64,
    pub published_ms: f64,
    pub received_ms: f64,
    pub ui_drained_ms: f64,
    #[serde(default)]
    pub wakeup_requested_ms: Option<f64>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PacingTotals {
    #[serde(default)]
    pub cross_realm_intervals_unavailable: bool,
    #[serde(default)]
    pub cross_realm_inversions: u64,
    pub requests: u64,
    pub invalid_samples: u64,
    pub dispatch_to_worker_ms: f64,
    pub worker_execution_ms: f64,
    pub finish_to_publish_ms: f64,
    pub publish_to_receive_ms: f64,
    pub receive_to_ui_ms: f64,
    pub finish_to_ui_max_ms: f64,
    pub same_frame_next_dispatches: u64,
    pub ui_to_next_dispatch_ms: f64,
}
impl PacingTotals {
    fn record(&mut self, s: &PacingSample) {
        self.record_realms(s, cfg!(target_arch = "wasm32"));
    }
    fn record_realms(&mut self, s: &PacingSample, browser: bool) {
        if browser {
            self.cross_realm_intervals_unavailable = true;
            let worker = [s.worker_started_ms, s.worker_finished_ms, s.published_ms];
            let ui = [s.dispatch_ms, s.received_ms, s.ui_drained_ms];
            if worker.iter().chain(ui.iter()).any(|v| !v.is_finite())
                || worker.windows(2).chain(ui.windows(2)).any(|w| w[1] < w[0])
            {
                self.invalid_samples += 1;
                return;
            }
            if s.worker_started_ms < s.dispatch_ms || s.received_ms < s.published_ms {
                self.cross_realm_inversions += 1;
            }
            self.requests += 1;
            self.worker_execution_ms += s.worker_finished_ms - s.worker_started_ms;
            self.finish_to_publish_ms += s.published_ms - s.worker_finished_ms;
            self.receive_to_ui_ms += s.ui_drained_ms - s.received_ms;
            return;
        }
        let stamps = [
            s.dispatch_ms,
            s.worker_started_ms,
            s.worker_finished_ms,
            s.published_ms,
            s.received_ms,
            s.ui_drained_ms,
        ];
        // Keep clock inversions visible instead of silently clipping them.
        if stamps.iter().any(|v| !v.is_finite()) || stamps.windows(2).any(|w| w[1] < w[0]) {
            self.invalid_samples += 1;
            return;
        }
        self.requests += 1;
        self.dispatch_to_worker_ms += s.worker_started_ms - s.dispatch_ms;
        self.worker_execution_ms += s.worker_finished_ms - s.worker_started_ms;
        self.finish_to_publish_ms += s.published_ms - s.worker_finished_ms;
        self.publish_to_receive_ms += s.received_ms - s.published_ms;
        self.receive_to_ui_ms += s.ui_drained_ms - s.received_ms;
        self.finish_to_ui_max_ms = self
            .finish_to_ui_max_ms
            .max(s.ui_drained_ms - s.worker_finished_ms);
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnosticBoundary {
    pub kind: String,
    pub clock_ms: f64,
    pub frame: u64,
    pub phase: String,
    pub token: Option<polyorama_runtime::RequestToken>,
    pub worker_reserved_bytes: usize,
    pub decoded_bytes: usize,
    pub upload_bytes: usize,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub instrument: Option<String>,
    #[serde(default)]
    pub diagnostic_options: crate::DiagnosticOptions,
    #[serde(default)]
    pub diagnostic_trace: Vec<DiagnosticBoundary>,
    #[serde(default)]
    pub diagnostic_trace_dropped: u64,
    #[serde(default)]
    pub phase_data_ready_ms: Option<f64>,
    #[serde(default)]
    pub diagnostic_cycles_completed: usize,
    #[serde(default)]
    pub display_reset_released_items: usize,
    #[serde(default)]
    pub display_reset_released_logical_bytes: usize,
    #[serde(default)]
    pub pacing: PacingTotals,
    pub loaded: bool,
    pub events_dropped: u64,
    pub ready_demands: usize,
    pub primary_desired: usize,
    pub primary_ready: usize,
    pub runtime_epoch: u64,
    pub phase_label: String,
    pub phase_started_ms: f64,
    pub phase_started_frame: u64,
    pub phase_settled_ms: Option<f64>,
    pub phase_first_useful_ms: Option<f64>,
    pub process_peak_rss_bytes: Option<u64>,
    pub events: Vec<WorkEvent>,
    pub gpu_adapter: String,
    pub image: usize,
    pub image_count: usize,
    pub comparison_image: Option<usize>,
    pub bookmarks: usize,
    pub generation: u64,
    pub logical_detections: usize,
    pub materialised_detections: usize,
    pub desired: usize,
    pub in_flight: usize,
    pub peak_in_flight: usize,
    pub decoded_accounted_bytes: usize,
    pub decoded_peak_bytes: usize,
    pub completed: u64,
    pub cancelled: u64,
    pub stale: u64,
    pub gpu_bytes: usize,
    pub gpu_peak_bytes: usize,
    pub gpu_items: usize,
    pub gpu_uploads: u64,
    pub gpu_evictions: u64,
    pub rendered_regions: usize,
    pub frame: u64,
    pub elapsed_ms: f64,
    pub first_useful_ms: Option<f64>,
    pub worker: WorkerMetrics,
    pub errors: Vec<String>,
    pub script_step: usize,
    pub script_complete: bool,
}
struct View {
    clip: egui::Rect,
    pane: PaneId,
    rect: egui::Rect,
    draws: Vec<RegionalDraw>,
}

#[derive(Clone, Debug, Deserialize)]
struct ScriptStep {
    label: String,
    intent: Intent,
    #[serde(default)]
    diagnostic_cycle: Option<usize>,
    #[serde(default)]
    authored_completion: bool,
}

pub struct ViewerApp {
    server: String,
    compressed_limit: usize,
    decoded_limit: usize,
    gpu_limit: usize,
    clear_display_cache: bool,
    context: egui::Context,
    executor: Executor,
    runtime: RegionalRuntime,
    catalogue: Vec<Manifest>,
    image: usize,
    camera: Camera,
    detail: Option<usize>,
    comparison: Option<usize>,
    bookmark_cursor: usize,
    bookmarks: Vec<Bookmark>,
    generation: u64,
    gamma: f32,
    low: f32,
    high: f32,
    diagnostics: bool,
    scroll_to: Option<usize>,
    snapshot: Snapshot,
    script_stages: Vec<Snapshot>,
    started: Instant,
    script: bool,
    script_workload: Vec<ScriptStep>,
    script_step: usize,
    step_started: Instant,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    script_output: Option<String>,
    last_demands: Vec<RegionDemand>,
    pacing_dispatch: Option<(polyorama_runtime::RequestToken, f64)>,
    pacing_receipt: Option<f64>,
    diagnostic: crate::DiagnosticOptions,
    authored_workload_admitted: bool,
    cycle: Option<(usize, std::collections::BTreeSet<RegionKey>)>,
    #[cfg(not(target_arch = "wasm32"))]
    memory: Option<crate::memory::MemoryMarkers>,
}
impl ViewerApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        server: String,
        compressed: usize,
        decoded: usize,
        gpu: usize,
        script: bool,
        script_output: Option<String>,
    ) -> Self {
        Self::new_diagnostic(
            cc,
            server,
            (compressed, decoded, gpu),
            script,
            script_output,
            crate::DiagnosticOptions::default(),
        )
    }
    pub fn new_diagnostic(
        cc: &eframe::CreationContext<'_>,
        server: String,
        limits: (usize, usize, usize),
        script: bool,
        script_output: Option<String>,
        diagnostic: crate::DiagnosticOptions,
    ) -> Self {
        let (compressed, decoded, gpu) = limits;
        apply_design_system(&cc.egui_ctx, UiPreferences::default());
        let state = cc.wgpu_render_state.as_ref().expect("viewer requires WGPU");
        state
            .renderer
            .write()
            .callback_resources
            .insert(RegionalRenderer::new(
                &state.device,
                state.target_format,
                RegionalGpuLimits {
                    texture_bytes: gpu,
                    texture_items: 512,
                    upload_scratch_bytes: 4 << 20,
                    draws: 1024,
                },
            ));
        Self {
            server: server.clone(),
            compressed_limit: compressed,
            decoded_limit: decoded,
            gpu_limit: gpu,
            clear_display_cache: false,
            context: cc.egui_ctx.clone(),
            executor: if diagnostic.enabled {
                Executor::new_diagnostic(server, compressed, cc.egui_ctx.clone(), diagnostic)
            } else {
                Executor::new(server, compressed, cc.egui_ctx.clone())
            },
            runtime: RegionalRuntime::new(RegionalRuntimeLimits {
                max_demands: 1024,
                max_in_flight: 1,
                decoded_bytes: decoded,
            }),
            catalogue: Vec::new(),
            image: 0,
            camera: Camera {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            detail: None,
            comparison: None,
            bookmark_cursor: 0,
            bookmarks: Vec::new(),
            generation: 1,
            gamma: 1.,
            low: 0.,
            high: 65535.,
            diagnostics: false,
            scroll_to: None,
            snapshot: Snapshot {
                instrument: diagnostic
                    .enabled
                    .then(|| "native-completion-memory-v1".into()),
                diagnostic_options: diagnostic,
                phase_label: "empty-client-overview".into(),
                gpu_adapter: format!("{:?}", state.adapter.get_info()),
                ..Default::default()
            },
            script_stages: Vec::new(),
            started: Instant::now(),
            script,
            script_workload: serde_json::from_str(include_str!("../qualification-workload.json"))
                .unwrap(),
            script_step: 0,
            step_started: Instant::now(),
            script_output,
            last_demands: Vec::new(),
            pacing_dispatch: None,
            pacing_receipt: None,
            diagnostic,
            authored_workload_admitted: false,
            cycle: None,
            #[cfg(not(target_arch = "wasm32"))]
            memory: None,
        }
    }
    fn error(&mut self, error: String) {
        if self.snapshot.errors.last() != Some(&error) {
            self.snapshot.errors.push(error);
            if self.snapshot.errors.len() > 16 {
                self.snapshot.errors.remove(0);
            }
        }
    }
    fn fit(&mut self) {
        if let Some(m) = self.catalogue.get(self.image) {
            let p = &m.identity.profile;
            self.camera = Camera {
                x: 0.,
                y: 0.,
                width: p.width as f32,
                height: p.height as f32,
            };
            self.low = 0.;
            self.high = ((1u32 << p.bits_per_sample) - 1) as f32;
        }
    }
    pub fn intent(&mut self, intent: Intent) {
        if self.catalogue.is_empty() {
            if matches!(
                intent,
                Intent::Action {
                    action: ViewerAction::Retry
                }
            ) {
                self.executor = Executor::new_diagnostic(
                    self.server.clone(),
                    self.compressed_limit,
                    self.context.clone(),
                    self.diagnostic,
                );
                self.snapshot.errors.clear();
                self.context.request_repaint();
            }
            return;
        }
        match intent {
            Intent::ClearDisplayCache => {
                if self.runtime.metrics().in_flight != 0 {
                    self.error("Display cache reset requires a settled executor".into());
                    return;
                }
                self.clear_display_cache = true;
            }
            Intent::Action { action } => match action {
                ViewerAction::Fit => self.fit(),
                ViewerAction::ZoomIn => self.zoom(0.5),
                ViewerAction::ZoomOut => self.zoom(2.),
                ViewerAction::PanLeft => self.pan(-0.3, 0.),
                ViewerAction::PanRight => self.pan(0.3, 0.),
                ViewerAction::NextImage => {
                    self.image = (self.image + 1) % self.catalogue.len();
                    self.detail = None;
                    self.fit();
                }
                ViewerAction::CompareImage => {
                    self.comparison = Some((self.image + 1) % self.catalogue.len());
                    self.detail = None;
                }
                ViewerAction::Bookmark => {
                    if self.bookmarks.len() == 16 {
                        self.bookmarks.remove(0);
                    }
                    self.bookmarks.push(Bookmark {
                        image: self.image,
                        camera: self.camera,
                        detail: self.detail,
                    });
                }
                ViewerAction::Recall => {
                    if let Some(b) = self
                        .bookmarks
                        .get(self.bookmark_cursor % self.bookmarks.len().max(1))
                        .cloned()
                    {
                        self.image = b.image;
                        self.camera = b.camera;
                        self.detail = b.detail;
                        self.bookmark_cursor = (self.bookmark_cursor + 1) % self.bookmarks.len();
                        self.high = ((1u32
                            << self.catalogue[self.image].identity.profile.bits_per_sample)
                            - 1) as f32;
                    }
                }
                ViewerAction::Retry => {
                    for d in &self.last_demands {
                        self.runtime.retry(&d.key);
                    }
                    self.snapshot.errors.clear();
                }
                ViewerAction::Diagnostics => self.diagnostics = !self.diagnostics,
            },
            Intent::SelectImage { index } => {
                if index < self.catalogue.len() {
                    self.image = index;
                    self.detail = None;
                    self.fit();
                }
            }
            Intent::Pan { dx, dy } => self.pan(dx, dy),
            Intent::Zoom { factor } => {
                if factor.is_finite() && factor > 0. {
                    self.zoom(factor);
                }
            }
            Intent::Gallery { row } => self.scroll_to = Some(row.min(LOGICAL_DETECTIONS / 2 - 1)),
            Intent::OpenDetection { index } => {
                self.comparison = None;
                self.detail = Some(index.min(LOGICAL_DETECTIONS - 1))
            }
            Intent::Stretch { low, high, gamma } => {
                if low.is_finite()
                    && high.is_finite()
                    && low < high
                    && gamma.is_finite()
                    && gamma > 0.
                {
                    self.low = low;
                    self.high = high;
                    self.gamma = gamma;
                }
            }
        }
        self.generation += 1;
        self.context.request_repaint();
    }
    fn zoom(&mut self, factor: f32) {
        let p = &self.catalogue[self.image].identity.profile;
        let width = (self.camera.width * factor).clamp(32_f32.min(p.width as f32), p.width as f32);
        let height =
            (self.camera.height * factor).clamp(32_f32.min(p.height as f32), p.height as f32);
        self.camera.x += (self.camera.width - width) * 0.5;
        self.camera.y += (self.camera.height - height) * 0.5;
        self.camera.width = width;
        self.camera.height = height;
        self.pan(0., 0.);
    }
    fn pan(&mut self, dx: f32, dy: f32) {
        if !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let p = &self.catalogue[self.image].identity.profile;
        self.camera.x = (self.camera.x + dx * self.camera.width)
            .clamp(0., (p.width as f32 - self.camera.width).max(0.));
        self.camera.y = (self.camera.y + dy * self.camera.height)
            .clamp(0., (p.height as f32 - self.camera.height).max(0.));
    }
    /// Install an explicit bounded qualification workload before its first action.
    pub fn set_script_workload(&mut self, json: &str) -> Result<(), String> {
        if self.script_step != 0 || self.snapshot.script_complete {
            return Err("workload already started".into());
        }
        if json.len() > 64 * 1024 {
            return Err("workload exceeds 64 KiB".into());
        }
        let steps: Vec<ScriptStep> = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let labels: std::collections::BTreeSet<_> = steps.iter().map(|s| &s.label).collect();
        if steps.is_empty()
            || steps.len() > 64
            || labels.len() != steps.len()
            || steps
                .iter()
                .any(|s| s.label.is_empty() || s.label == "empty-client-overview")
        {
            return Err("workload requires 1–64 uniquely labelled actions".into());
        }
        validate_diagnostic_steps(&steps, self.diagnostic)?;
        self.authored_workload_admitted = self.diagnostic.authored_immediate;
        self.script_workload = steps;
        Ok(())
    }
    pub fn script_stages(&self) -> &[Snapshot] {
        &self.script_stages
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn attach_memory_markers(&mut self, memory: Option<crate::memory::MemoryMarkers>) {
        self.memory = memory;
        self.boundary("phase-start", None);
        self.memory_boundary("phase-start");
    }
    fn memory_boundary(&mut self, kind: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        if self.memory.is_some() {
            let lifetimes = serde_json::json!({
                "runtime_epoch":self.snapshot.runtime_epoch,
                "runtime":self.runtime.allocation_diagnostics(),
                "display_reset_released_items":self.snapshot.display_reset_released_items,
                "display_reset_released_logical_bytes":self.snapshot.display_reset_released_logical_bytes,
                "event_ring_slots":self.snapshot.events.capacity(),
                "trace_ring_slots":self.snapshot.diagnostic_trace.capacity(),
                "stage_slots":self.script_stages.capacity(),
                "retained_stage_count":self.script_stages.len(),
                "boundary":"current owner counts/capacities, not allocator totals; runtime counters reset at epoch; display release totals cumulative; retained stages contain bounded evidence copies"
            });
            if let Err(error) =
                self.memory
                    .as_mut()
                    .unwrap()
                    .emit(kind, &self.snapshot.phase_label, lifetimes)
            {
                self.error(format!("Memory diagnostics: {error}"));
                self.memory = None;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = kind;
    }
    fn boundary(&mut self, kind: &str, token: Option<polyorama_runtime::RequestToken>) {
        if !self.diagnostic.enabled {
            return;
        }
        if self.snapshot.diagnostic_trace.len() == 256 {
            self.snapshot.diagnostic_trace.remove(0);
            self.snapshot.diagnostic_trace_dropped += 1;
        }
        let m = self.runtime.metrics();
        self.snapshot.diagnostic_trace.push(DiagnosticBoundary {
            kind: kind.into(),
            clock_ms: pacing_now_ms(),
            frame: self.snapshot.frame,
            phase: self.snapshot.phase_label.clone(),
            token,
            worker_reserved_bytes: m.worker_reserved_bytes,
            decoded_bytes: m.decoded_bytes,
            upload_bytes: m.upload_bytes,
        });
    }
    fn work_event(&mut self, kind: &str, request: &polyorama_runtime::RegionalRequest) {
        self.resource_event(kind, &request.key, request.token);
    }
    fn resource_event(
        &mut self,
        kind: &str,
        key: &RegionKey,
        token: polyorama_runtime::RequestToken,
    ) {
        if self.snapshot.events.len() == 128 {
            self.snapshot.events.remove(0);
            self.snapshot.events_dropped += 1;
        }
        self.snapshot.events.push(WorkEvent {
            kind: kind.into(),
            at_ms: self.started.elapsed().as_secs_f64() * 1000.,
            key: key.clone(),
            token,
            accounted_decoded_bytes: self.runtime.metrics().accounted_decoded_bytes(),
            pacing: None,
        });
    }
    fn receive(&mut self) {
        self.pacing_receipt = None;
        for mut event in self.executor.drain() {
            let drained_ms = pacing_now_ms();
            let timing = event.metrics_mut().and_then(|m| m.timing.clone());
            let request = match &event {
                Event::Completed { request, .. } | Event::Cancelled { request, .. } => {
                    Some(request)
                }
                Event::Failed { request, .. } => request.as_ref(),
                Event::Catalogue(_) => None,
            };
            let sample = request.and_then(|request| {
                let (token, dispatch_ms) = self.pacing_dispatch?;
                let t = timing?;
                if token != request.token {
                    return None;
                }
                self.pacing_dispatch = None;
                Some(PacingSample {
                    dispatch_ms,
                    worker_started_ms: t.started_ms,
                    worker_finished_ms: t.finished_ms,
                    published_ms: t.published_ms,
                    received_ms: t.received_ms?,
                    ui_drained_ms: drained_ms,
                    wakeup_requested_ms: t.wakeup_requested_ms,
                })
            });
            if let Some(sample) = &sample {
                self.snapshot.pacing.record(sample);
                self.pacing_receipt = Some(drained_ms);
            }
            match event {
                Event::Catalogue(c) => {
                    self.catalogue = c;
                    self.fit();
                }
                Event::Completed {
                    request,
                    pixels,
                    metrics,
                } => {
                    self.boundary("ui-drain", Some(request.token));
                    let outcome = self.runtime.complete_frame(&request, pixels);
                    self.work_event(&format!("completion_{outcome:?}"), &request);
                    self.snapshot.worker = metrics;
                }
                Event::Cancelled { request, metrics } => {
                    self.work_event("cancel_acknowledged", &request);
                    self.runtime.acknowledge_cancelled(&request);
                    self.snapshot.worker = metrics;
                }
                Event::Failed {
                    request,
                    error,
                    metrics,
                } => {
                    if let Some(request) = request {
                        self.work_event("failed", &request);
                        self.runtime.fail(&request);
                    }
                    self.error(error);
                    self.snapshot.worker = metrics;
                }
            }
            if let Some(sample) = sample
                && let Some(event) = self.snapshot.events.last_mut()
            {
                event.pacing = Some(sample);
            }
        }
    }
    fn script(&mut self) {
        if !self.script || self.catalogue.is_empty() || self.snapshot.script_complete {
            return;
        }
        if self.snapshot.phase_settled_ms.is_none() {
            if self.step_started.elapsed() < Duration::from_secs(60) {
                self.context
                    .request_repaint_after(Duration::from_millis(20));
                return;
            }
            self.error(format!(
                "Qualification phase {} exceeded 60 seconds",
                self.snapshot.phase_label
            ));
            self.script_stages.push(self.snapshot.clone());
            self.snapshot.script_complete = true;
            return;
        }
        self.script_stages.push(self.snapshot.clone());
        let Some(step) = self.script_workload.get(self.script_step).cloned() else {
            self.snapshot.script_complete = true;
            return;
        };
        self.cycle = step.diagnostic_cycle.map(|number| {
            (
                number,
                self.last_demands.iter().map(|d| d.key.clone()).collect(),
            )
        });
        self.snapshot.phase_label = step.label;
        self.snapshot.phase_started_ms = self.started.elapsed().as_secs_f64() * 1000.;
        self.snapshot.phase_started_frame = self.snapshot.frame;
        self.snapshot.phase_settled_ms = None;
        self.snapshot.phase_first_useful_ms = None;
        self.snapshot.phase_data_ready_ms = None;
        self.boundary("phase-start", None);
        self.memory_boundary("phase-start");
        let intent = step.intent;
        self.intent(intent);
        self.script_step += 1;
        self.snapshot.script_step = self.script_step;
        self.step_started = Instant::now();
    }
}
impl eframe::App for ViewerApp {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn ui(&mut self, root_ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();
        if self.diagnostic.authored_immediate && !self.authored_workload_admitted {
            self.error("Authored diagnostic workload must be admitted before dispatch".into());
            return;
        }
        self.receive();
        self.script();
        let state = frame.wgpu_render_state().expect("WGPU");
        if self.clear_display_cache {
            let released = state
                .renderer
                .read()
                .callback_resources
                .get::<RegionalRenderer>()
                .unwrap()
                .metrics();
            let released_items = released.texture_items;
            let released_bytes = released.texture_bytes;
            self.runtime = RegionalRuntime::new(RegionalRuntimeLimits {
                max_demands: 1024,
                max_in_flight: 1,
                decoded_bytes: self.decoded_limit,
            });
            state
                .renderer
                .write()
                .callback_resources
                .insert(RegionalRenderer::new(
                    &state.device,
                    state.target_format,
                    RegionalGpuLimits {
                        texture_bytes: self.gpu_limit,
                        texture_items: 512,
                        upload_scratch_bytes: 4 << 20,
                        draws: 1024,
                    },
                ));
            self.snapshot.runtime_epoch += 1;
            self.clear_display_cache = false;
            if self.diagnostic.enabled {
                self.snapshot.display_reset_released_items += released_items;
                self.snapshot.display_reset_released_logical_bytes += released_bytes;
            }
            if self.cycle.is_some() {
                if released_items == 0 || released_bytes == 0 {
                    self.error("Diagnostic cycle did not release resident textures".into());
                    self.cycle = None;
                } else {
                    self.boundary("cycle-evicted", None);
                    self.memory_boundary("cycle-evicted");
                }
            }
        }
        let mut intents = Vec::new();
        let mut demands = Vec::new();
        let mut views = Vec::new();
        egui::Panel::top("viewer-toolbar").show(root_ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("Emuella");
                for action in [
                    ViewerAction::Fit,
                    ViewerAction::ZoomIn,
                    ViewerAction::ZoomOut,
                    ViewerAction::PanLeft,
                    ViewerAction::PanRight,
                    ViewerAction::NextImage,
                    ViewerAction::CompareImage,
                    ViewerAction::Bookmark,
                    ViewerAction::Recall,
                    ViewerAction::Retry,
                    ViewerAction::Diagnostics,
                ] {
                    if button(ui, action) {
                        intents.push(Intent::Action { action });
                    }
                }
            });
            ui.horizontal(|ui| {
                if let Some(m) = self.catalogue.get(self.image) {
                    ui.label(format!(
                        "{} · {} × {} · {}-bit · {}",
                        m.target,
                        m.identity.profile.width,
                        m.identity.profile.height,
                        m.identity.profile.bits_per_sample,
                        if m.identity.profile.components == 3 {
                            "RGB"
                        } else {
                            "PAN"
                        }
                    ));
                } else {
                    ui.label("Connecting to image catalogue…");
                }
            });
        });
        if let Some(manifest) = self.catalogue.get(self.image) {
            let display = RegionalDisplaySettings {
                low: [self.low; 3],
                high: [self.high; 3],
                gamma: self.gamma,
            };
            egui::Panel::right("viewer-gallery")
                .default_size(300.)
                .min_size(260.)
                .show(root_ui, |ui| {
                    ui.heading("Detections");
                    ui.label("10,000 clustered and scattered regions");
                    let mut scroll = egui::ScrollArea::vertical().id_salt("detection-grid");
                    if let Some(row) = self.scroll_to.take() {
                        scroll = scroll.vertical_scroll_offset(row as f32 * 116.);
                    }
                    let mut count = 0;
                    scroll.show_rows(ui, 112., LOGICAL_DETECTIONS.div_ceil(2), |ui, rows| {
                        let grid = layout_virtual_grid(LOGICAL_DETECTIONS, 2, rows.clone(), 2);
                        count = grid.materialised_items.len();
                        for i in grid.materialised_items.clone() {
                            let region = detection(manifest, i);
                            let reduction = 2.min(manifest.identity.profile.decomposition_levels);
                            demands.push(demand(
                                manifest,
                                region,
                                reduction,
                                100 + i as u64,
                                if grid.visible_items.contains(&i) {
                                    DemandPriority::Visible
                                } else {
                                    DemandPriority::Prefetch
                                },
                            ));
                        }
                        for row in rows {
                            ui.horizontal(|ui| {
                                for i in row * 2..(row * 2 + 2).min(LOGICAL_DETECTIONS) {
                                    ui.push_id(("detection", manifest.tid.as_str(), i), |ui| {
                                        ui.vertical(|ui| {
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(128., 84.),
                                                egui::Sense::click(),
                                            );
                                            let region = detection(manifest, i);
                                            let reduction = 2.min(
                                                manifest.identity.profile.decomposition_levels,
                                            );
                                            let key = demand(
                                                manifest,
                                                region,
                                                reduction,
                                                100 + i as u64,
                                                DemandPriority::Visible,
                                            )
                                            .key;
                                            views.push(View {
                                                pane: PaneId(100 + i as u32),
                                                rect: fit_rect(rect, region.width, region.height),
                                                clip: ui.clip_rect(),
                                                draws: vec![RegionalDraw {
                                                    key,
                                                    rect_ndc: [-1., 1., 1., -1.],
                                                    uv: [0., 0., 1., 1.],
                                                    display,
                                                }],
                                            });
                                            if response.clicked() {
                                                intents.push(Intent::OpenDetection { index: i });
                                            }
                                            ui.label(format!("Detection {:05}", i + 1));
                                        });
                                    });
                                }
                            });
                        }
                    });
                    self.snapshot.materialised_detections = count;
                });
            egui::Panel::bottom("viewer-status").show(root_ui,|ui| {
                ui.horizontal(|ui| {ui.label("Display gamma");let mut gamma=self.gamma;if ui.add(egui::Slider::new(&mut gamma,0.3..=3.)).changed(){intents.push(Intent::Stretch{low:self.low,high:self.high,gamma});}ui.label(format!("{} regions ready",self.snapshot.rendered_regions));});
                if self.diagnostics {ui.label(format!("JPP received {} B · compressed {} B · decoded {} B · GPU {} B · cache hits {} · cancelled {} · stale {}",self.snapshot.worker.received_jpp_bytes,self.snapshot.worker.compressed_bytes,self.snapshot.decoded_accounted_bytes,self.snapshot.gpu_bytes,self.snapshot.worker.cache_hits,self.snapshot.cancelled,self.snapshot.stale));}
                if let Some(error)=self.snapshot.errors.last(){ui.label(error);}
            });
            egui::CentralPanel::default().show(root_ui, |ui| {
                ui.columns(2, |columns| {
                    columns[0].heading("Primary image");
                    let available = columns[0].available_size();
                    let (rect, response) =
                        columns[0].allocate_exact_size(available, egui::Sense::click_and_drag());
                    let cam = self.camera;
                    let region = camera_region(cam, manifest);
                    add_view(
                        manifest,
                        region,
                        rect,
                        PaneId(1),
                        display,
                        None,
                        &mut demands,
                        &mut views,
                    );
                    if response.dragged() {
                        let delta = ctx.input(|i| i.pointer.delta());
                        intents.push(Intent::Pan {
                            dx: -delta.x / rect.width(),
                            dy: -delta.y / rect.height(),
                        });
                    }
                    if response.hovered() {
                        let scroll = ctx.input(|i| i.smooth_scroll_delta.y);
                        if scroll.abs() > 0.01 {
                            intents.push(Intent::Zoom {
                                factor: (-scroll * 0.002).exp(),
                            });
                        }
                    }
                    let secondary = self
                        .comparison
                        .and_then(|i| self.catalogue.get(i))
                        .unwrap_or(manifest);
                    let secondary_display = if self.comparison.is_some() {
                        RegionalDisplaySettings {
                            low: [0.; 3],
                            high: [((1u32 << secondary.identity.profile.bits_per_sample) - 1)
                                as f32; 3],
                            ..display
                        }
                    } else {
                        display
                    };
                    columns[1].heading(if self.comparison.is_some() {
                        format!("Comparison · {}", secondary.target)
                    } else if self.detail.is_some() {
                        "Detection detail".into()
                    } else {
                        "Linked overview".into()
                    });
                    let (rect, _) = columns[1]
                        .allocate_exact_size(columns[1].available_size(), egui::Sense::hover());
                    let region = if let Some(index) = self.detail {
                        let d = detection(manifest, index);
                        let p = &manifest.identity.profile;
                        let width = 512.min(p.width);
                        let height = 512.min(p.height);
                        ImageRegion {
                            x: d.x.saturating_sub(128).min(p.width - width),
                            y: d.y.saturating_sub(128).min(p.height - height),
                            width,
                            height,
                        }
                    } else {
                        let p = &secondary.identity.profile;
                        ImageRegion {
                            x: 0,
                            y: 0,
                            width: p.width,
                            height: p.height,
                        }
                    };
                    add_view(
                        secondary,
                        region,
                        rect,
                        PaneId(2),
                        secondary_display,
                        self.detail.map(|_| 0),
                        &mut demands,
                        &mut views,
                    );
                });
            });
        } else {
            egui::CentralPanel::default().show(root_ui, |ui| {
                ui.label("Preparing the shared image source…");
                if let Some(error) = self.snapshot.errors.last() {
                    ui.label(error);
                }
            });
        }
        // The complete desired state across panes and materialised thumbnails is reconciled once.
        match self.runtime.reconcile(self.generation, demands.clone()) {
            Ok(cancelled) => {
                for request in cancelled {
                    self.work_event("cancel_requested", &request);
                    self.executor.cancel(&request);
                }
            }
            Err(e) => self.error(format!("Demand reconciliation: {e:?}")),
        }
        self.last_demands = demands;
        {
            let mut backend = state.renderer.write();
            let renderer = backend
                .callback_resources
                .get_mut::<RegionalRenderer>()
                .unwrap();
            renderer.begin_frame();
            while let Some(upload) = self.runtime.take_decoded_frame() {
                let key = upload.key.clone();
                let token = upload.token;
                if !self.runtime.is_upload_current(&key, token) {
                    drop(upload);
                    self.runtime.finish_upload(&key, token, false);
                    continue;
                }
                self.boundary("upload-start", Some(token));
                match renderer.upload_frame(&state.device, &state.queue, upload) {
                    Ok(admission) => {
                        for evicted in admission.evicted {
                            self.resource_event("gpu_evicted", &evicted.key, evicted.token);
                            self.runtime.evict_resident(&evicted.key, evicted.token);
                        }
                        self.runtime.finish_upload(&key, token, true);
                        self.boundary("upload-finished", Some(token));
                    }
                    Err((upload, error)) => {
                        drop(upload);
                        self.runtime.finish_upload(&key, token, false);
                        self.error(format!("GPU admission: {error:?}"));
                    }
                }
            }
        }
        for request in self.runtime.dispatch() {
            let dispatch_ms = pacing_now_ms();
            if let Some(received_ms) = self.pacing_receipt.take() {
                self.snapshot.pacing.same_frame_next_dispatches += 1;
                self.snapshot.pacing.ui_to_next_dispatch_ms += dispatch_ms - received_ms;
            }
            self.pacing_dispatch = Some((request.token, dispatch_ms));
            self.work_event("dispatched", &request);
            self.boundary("dispatch", Some(request.token));
            if let Some(manifest) = self
                .catalogue
                .iter()
                .find(|m| representation(m) == request.key.representation)
                && let Err(e) = self.executor.submit(Job {
                    request: request.clone(),
                    manifest: manifest.clone(),
                })
            {
                self.runtime.fail(&request);
                self.error(e.to_string());
            }
        }
        {
            let mut backend = state.renderer.write();
            let renderer = backend
                .callback_resources
                .get_mut::<RegionalRenderer>()
                .unwrap();
            for view in views {
                if let Err(e) = renderer.prepare(&state.device, view.pane, &view.draws) {
                    self.error(format!("Render preparation: {e:?}"));
                }
                ctx.layer_painter(egui::LayerId::background())
                    .with_clip_rect(view.clip)
                    .add(egui_wgpu::Callback::new_paint_callback(
                        view.rect,
                        RegionCallback(view.pane),
                    ));
            }
            let g = renderer.metrics();
            self.snapshot.gpu_bytes = g.texture_bytes;
            self.snapshot.gpu_peak_bytes = self.snapshot.gpu_peak_bytes.max(g.texture_bytes);
            self.snapshot.gpu_items = g.texture_items;
            self.snapshot.gpu_uploads = g.uploads;
            self.snapshot.gpu_evictions = g.evictions;
            self.snapshot.rendered_regions = g.prepared_draws;
        }
        if self.pacing_receipt.is_some() || self.snapshot.phase_data_ready_ms.is_none() {
            self.boundary("render-submitted", None);
        }
        for intent in intents {
            self.intent(intent);
        }
        let m = self.runtime.metrics();
        self.snapshot.loaded = !self.catalogue.is_empty();
        self.snapshot.image = self.image;
        self.snapshot.image_count = self.catalogue.len();
        self.snapshot.comparison_image = self.comparison;
        self.snapshot.bookmarks = self.bookmarks.len();
        self.snapshot.generation = self.generation;
        self.snapshot.logical_detections = LOGICAL_DETECTIONS;
        self.snapshot.desired = m.desired;
        self.snapshot.ready_demands = self
            .last_demands
            .iter()
            .map(|d| &d.key)
            .collect::<std::collections::BTreeSet<_>>()
            .iter()
            .filter(|k| self.runtime.is_resident(k))
            .count();
        self.snapshot.primary_desired = self
            .last_demands
            .iter()
            .filter(|d| d.consumer == RegionConsumerId(1))
            .count();
        self.snapshot.primary_ready = self
            .last_demands
            .iter()
            .filter(|d| d.consumer == RegionConsumerId(1) && self.runtime.is_resident(&d.key))
            .count();
        self.snapshot.in_flight = m.in_flight;
        self.snapshot.peak_in_flight = self.snapshot.peak_in_flight.max(m.in_flight);
        self.snapshot.decoded_accounted_bytes = m.accounted_decoded_bytes();
        self.snapshot.decoded_peak_bytes = self
            .snapshot
            .decoded_peak_bytes
            .max(m.accounted_decoded_bytes());
        self.snapshot.completed = m.completed;
        self.snapshot.cancelled = m.cancelled;
        self.snapshot.stale = m.stale;
        self.snapshot.elapsed_ms = self.started.elapsed().as_secs_f64() * 1000.;
        self.snapshot.frame += 1;
        #[cfg(target_os = "linux")]
        {
            self.snapshot.process_peak_rss_bytes = std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines().find_map(|line| {
                        line.strip_prefix("VmHWM:")
                            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
                    })
                })
                .map(|v| v * 1024);
        }
        if self.snapshot.primary_ready > 0 && self.snapshot.phase_first_useful_ms.is_none() {
            self.snapshot.phase_first_useful_ms =
                Some(self.snapshot.elapsed_ms - self.snapshot.phase_started_ms);
        }
        if self.snapshot.desired > 0
            && self.snapshot.ready_demands == self.snapshot.desired
            && m.in_flight == 0
            && self.snapshot.phase_data_ready_ms.is_none()
        {
            self.snapshot.phase_data_ready_ms =
                Some(self.snapshot.elapsed_ms - self.snapshot.phase_started_ms);
            self.boundary("data-ready", None);
        }
        // ScrollArea applies requested scroll on a subsequent frame. Observe three
        // complete frames before accepting the new desired state as settled.
        if phase_is_settled(
            self.snapshot.frame,
            self.snapshot.phase_started_frame,
            self.snapshot.desired,
            self.snapshot.ready_demands,
            m.in_flight,
        ) && self.snapshot.phase_settled_ms.is_none()
        {
            self.snapshot.phase_settled_ms =
                Some(self.snapshot.elapsed_ms - self.snapshot.phase_started_ms);
            self.boundary("phase-settled", None);
            self.memory_boundary("phase-settled");
            if let Some((number, keys)) = self.cycle.take() {
                let restored = self.last_demands.iter().map(|d| d.key.clone()).collect();
                if cycle_restored(
                    number,
                    self.snapshot.diagnostic_cycles_completed,
                    &keys,
                    &restored,
                    self.snapshot.gpu_uploads,
                ) {
                    self.snapshot.diagnostic_cycles_completed += 1;
                    self.boundary("cycle-revisited", None);
                    self.memory_boundary("cycle-revisited");
                } else {
                    self.error("Diagnostic revisit did not restore the same demands with actual uploads in cycle order".into());
                }
            }
        }
        if self.snapshot.rendered_regions > 0 && self.snapshot.first_useful_ms.is_none() {
            self.snapshot.first_useful_ms = Some(self.snapshot.elapsed_ms);
        }
        if self.script && !self.snapshot.script_complete {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.snapshot.script_complete
            && let Some(path) = self.script_output.take()
        {
            let stages_path = format!("{path}.stages.json");
            if let Err(error) = std::fs::write(
                stages_path,
                serde_json::to_vec_pretty(&self.script_stages).unwrap(),
            ) {
                self.error(error.to_string());
            }
            match std::fs::write(path, serde_json::to_vec_pretty(&self.snapshot).unwrap()) {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => self.error(e.to_string()),
            }
        }
    }
}
fn phase_is_settled(
    frame: u64,
    started_frame: u64,
    desired: usize,
    ready: usize,
    in_flight: usize,
) -> bool {
    frame >= started_frame + 3 && desired > 0 && ready == desired && in_flight == 0
}

fn validate_diagnostic_steps(
    steps: &[ScriptStep],
    options: crate::DiagnosticOptions,
) -> Result<(), String> {
    let cycles: Vec<_> = steps
        .iter()
        .filter(|s| s.diagnostic_cycle.is_some())
        .collect();
    if !cycles.is_empty()
        && (!options.enabled
            || cycles.len() != 10
            || cycles.iter().enumerate().any(|(i, s)| {
                s.diagnostic_cycle != Some(i + 1)
                    || s.label != format!("cycle-{:02}", i + 1)
                    || !matches!(s.intent, Intent::ClearDisplayCache)
            }))
    {
        return Err("diagnostic workload requires exactly ten ordered display eviction/revisit cycles and --native-diagnostics".into());
    }
    if options.authored_immediate
        && (!options.enabled || steps.iter().any(|s| !s.authored_completion))
    {
        return Err("immediate mode requires an explicitly authored diagnostic workload".into());
    }
    Ok(())
}
fn cycle_restored(
    number: usize,
    completed: usize,
    before: &std::collections::BTreeSet<RegionKey>,
    after: &std::collections::BTreeSet<RegionKey>,
    uploads: u64,
) -> bool {
    number == completed + 1 && !before.is_empty() && before == after && uploads > 0
}

fn button(ui: &mut egui::Ui, action: ViewerAction) -> bool {
    let mut observations = Vec::new();
    action_button(
        ui,
        ActionButtonSpec {
            target: ActionTarget::application(action),
            availability: Availability::Enabled,
            state: ActionButtonState::Momentary,
            emphasis: ActionEmphasis::Quiet,
            compact: false,
        },
        &UiPreferences::default().tokens(true),
        1.,
        &mut observations,
    )
    .clicked()
}
fn camera_region(c: Camera, m: &Manifest) -> ImageRegion {
    let p = &m.identity.profile;
    let x = (c.x as u32).min(p.width - 1);
    let y = (c.y as u32).min(p.height - 1);
    ImageRegion {
        x,
        y,
        width: (c.width.ceil() as u32).max(1).min(p.width - x),
        height: (c.height.ceil() as u32).max(1).min(p.height - y),
    }
}
/// O(1) deterministic metadata; no logical catalogue or independent thumbnail raster is stored.
fn detection(m: &Manifest, index: usize) -> ImageRegion {
    let p = &m.identity.profile;
    let width = 256.min(p.width);
    let height = 256.min(p.height);
    let seed = (index as u64)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let (x, y) = if index.is_multiple_of(2) {
        (
            (p.width - width) / 3 + ((seed >> 32) as u32 % 512),
            (p.height - height) / 3 + (seed as u32 % 512),
        )
    } else {
        (
            (seed >> 32) as u32 % (p.width - width + 1),
            seed as u32 % (p.height - height + 1),
        )
    };
    ImageRegion {
        x: x.min(p.width - width),
        y: y.min(p.height - height),
        width,
        height,
    }
}
fn demand(
    m: &Manifest,
    region: ImageRegion,
    reduction: u8,
    consumer: u64,
    priority: DemandPriority,
) -> RegionDemand {
    let scale = 1usize << reduction;
    let width =
        (region.x as usize + region.width as usize).div_ceil(scale) - region.x as usize / scale;
    let height =
        (region.y as usize + region.height as usize).div_ceil(scale) - region.y as usize / scale;
    RegionDemand {
        consumer: RegionConsumerId(consumer),
        key: RegionKey {
            representation: representation(m),
            region,
            reduction,
            components: (0..m.identity.profile.components).collect(),
            stage: SourceStage(1),
        },
        priority,
        max_decoded_bytes: width
            * height
            * (m.identity.profile.components as usize * 4
                + if m.identity.validity.is_some() { 2 } else { 0 }),
    }
}
#[allow(clippy::too_many_arguments)]
fn add_view(
    m: &Manifest,
    region: ImageRegion,
    rect: egui::Rect,
    pane: PaneId,
    display: RegionalDisplaySettings,
    forced: Option<u8>,
    demands: &mut Vec<RegionDemand>,
    views: &mut Vec<View>,
) {
    let clip = rect;
    let rect = fit_rect(rect, region.width, region.height);
    let max = m.identity.profile.decomposition_levels;
    let reduction = forced.unwrap_or_else(|| {
        (0..=max)
            .find(|&d| {
                region.width.div_ceil(1 << d) <= rect.width().max(1.) as u32
                    && region.height.div_ceil(1 << d) <= rect.height().max(1.) as u32
            })
            .unwrap_or(max)
    });
    let mut draws = Vec::new();
    for d in [max, reduction] {
        if d == reduction && d == max && !draws.is_empty() {
            continue;
        }
        let edge = (512u32 << d).min(m.identity.profile.tile_edge * 8);
        let start_x = region.x / edge * edge;
        let start_y = region.y / edge * edge;
        for y in (start_y..region.y + region.height).step_by(edge as usize) {
            for x in (start_x..region.x + region.width).step_by(edge as usize) {
                let p = &m.identity.profile;
                let chunk = ImageRegion {
                    x,
                    y,
                    width: edge.min(p.width - x),
                    height: edge.min(p.height - y),
                };
                let item = demand(m, chunk, d, u64::from(pane.0), DemandPriority::Visible);
                let l = x.max(region.x);
                let t = y.max(region.y);
                let r = (x + chunk.width).min(region.x + region.width);
                let b = (y + chunk.height).min(region.y + region.height);
                draws.push(RegionalDraw {
                    key: item.key.clone(),
                    rect_ndc: [
                        (l - region.x) as f32 / region.width as f32 * 2. - 1.,
                        1. - (t - region.y) as f32 / region.height as f32 * 2.,
                        (r - region.x) as f32 / region.width as f32 * 2. - 1.,
                        1. - (b - region.y) as f32 / region.height as f32 * 2.,
                    ],
                    uv: [
                        (l - x) as f32 / chunk.width as f32,
                        (t - y) as f32 / chunk.height as f32,
                        (r - x) as f32 / chunk.width as f32,
                        (b - y) as f32 / chunk.height as f32,
                    ],
                    display,
                });
                demands.push(item);
            }
        }
    }
    views.push(View {
        pane,
        rect,
        clip,
        draws,
    });
}
struct RegionCallback(PaneId);
impl egui_wgpu::CallbackTrait for RegionCallback {
    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if let Some(renderer) = resources.get::<RegionalRenderer>() {
            let v = info.viewport_in_pixels();
            let c = info.clip_rect_in_pixels();
            renderer.paint(
                self.0,
                PixelRect {
                    x: v.left_px.max(0) as u32,
                    y: v.top_px.max(0) as u32,
                    width: v.width_px.max(0) as u32,
                    height: v.height_px.max(0) as u32,
                },
                PixelRect {
                    x: c.left_px.max(0) as u32,
                    y: c.top_px.max(0) as u32,
                    width: c.width_px.max(0) as u32,
                    height: c.height_px.max(0) as u32,
                },
                pass,
            );
        }
    }
}

fn fit_rect(rect: egui::Rect, width: u32, height: u32) -> egui::Rect {
    let scale = (rect.width() / width as f32).min(rect.height() / height as f32);
    egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(width as f32 * scale, height as f32 * scale),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        serde_json::from_value(serde_json::json!({
            "target":"public-test", "tid":"00".repeat(32),
            "identity":{"source_sha256":"test", "bands":[0,1,2],
            "profile":{"width":43008,"height":43008,"tile_edge":512,"decomposition_levels":6,"bits_per_sample":16,"components":3,"bits_per_pixel":2.0},
            "codec_revision":"test", "encoding_contract":"test", "spatial_policy_sha256":"test", "payload_sha256":"test", "descriptor_format":"test"},
            "encoded_bytes":1,"main_header_bytes":1,"descriptor_sha256":[]
        })).unwrap()
    }
    #[test]
    fn data_readiness_does_not_shortcut_three_frame_settlement() {
        assert!(!phase_is_settled(12, 10, 30, 30, 0));
        assert!(phase_is_settled(13, 10, 30, 30, 0));
        assert!(!phase_is_settled(13, 10, 30, 29, 0));
        assert!(!phase_is_settled(13, 10, 30, 30, 1));
        assert!(!phase_is_settled(13, 10, 0, 0, 0));
    }
    #[test]
    fn diagnostic_workloads_require_ten_ordered_real_reset_actions() {
        let memory: Vec<ScriptStep> =
            serde_json::from_str(include_str!("../native-memory-workload.json")).unwrap();
        let authored: Vec<ScriptStep> =
            serde_json::from_str(include_str!("../authored-completion-workload.json")).unwrap();
        let normal: Vec<ScriptStep> =
            serde_json::from_str(include_str!("../real-scene-workload.json")).unwrap();
        let options = crate::DiagnosticOptions {
            enabled: true,
            authored_immediate: false,
        };
        assert!(validate_diagnostic_steps(&memory, options).is_ok());
        assert!(validate_diagnostic_steps(&memory, crate::DiagnosticOptions::default()).is_err());
        assert!(validate_diagnostic_steps(&normal, crate::DiagnosticOptions::default()).is_ok());
        assert!(
            validate_diagnostic_steps(
                &memory,
                crate::DiagnosticOptions {
                    authored_immediate: true,
                    ..options
                }
            )
            .is_err()
        );
        assert!(
            validate_diagnostic_steps(
                &authored,
                crate::DiagnosticOptions {
                    authored_immediate: true,
                    ..options
                }
            )
            .is_ok()
        );
        let mut truncated = memory.clone();
        truncated.pop();
        assert!(validate_diagnostic_steps(&truncated, options).is_err());
        let last = truncated.last_mut().unwrap();
        last.diagnostic_cycle = Some(3);
        assert!(validate_diagnostic_steps(&truncated, options).is_err());
        let key = crate::engine::diagnostic_tests::job().request.key;
        let before = std::collections::BTreeSet::from([key]);
        assert!(cycle_restored(1, 0, &before, &before, 1));
        assert!(!cycle_restored(1, 0, &before, &before, 0));
        assert!(!cycle_restored(2, 0, &before, &before, 1));
        assert!(!cycle_restored(1, 0, &before, &Default::default(), 1));
    }
    #[test]
    fn browser_intervals_remain_same_realm_even_with_inverted_delivery() {
        let s = PacingSample {
            dispatch_ms: 10.,
            worker_started_ms: 100.,
            worker_finished_ms: 105.,
            published_ms: 107.,
            received_ms: 12.,
            ui_drained_ms: 20.,
            wakeup_requested_ms: Some(13.),
        };
        let mut totals = PacingTotals::default();
        totals.record_realms(&s, true);
        assert!(totals.cross_realm_intervals_unavailable);
        assert_eq!(totals.cross_realm_inversions, 1);
        assert_eq!(totals.worker_execution_ms, 5.);
        assert_eq!(totals.receive_to_ui_ms, 8.);
        assert_eq!(totals.publish_to_receive_ms, 0.);
        assert_eq!(totals.dispatch_to_worker_ms, 0.);
        assert_eq!(totals.finish_to_ui_max_ms, 0.);
    }

    #[test]
    fn pacing_separates_worker_from_delivery_and_frame_wait() {
        let sample = PacingSample {
            dispatch_ms: 10.,
            worker_started_ms: 11.,
            worker_finished_ms: 14.,
            published_ms: 16.,
            received_ms: 18.,
            ui_drained_ms: 30.,
            wakeup_requested_ms: Some(17.),
        };
        let mut totals = PacingTotals::default();
        totals.record(&sample);
        totals.record(&sample);
        assert_eq!(totals.requests, 2);
        assert_eq!(totals.worker_execution_ms, 6.);
        assert_eq!(totals.finish_to_publish_ms, 4.);
        assert_eq!(totals.publish_to_receive_ms, 4.);
        assert_eq!(totals.receive_to_ui_ms, 24.);
        assert_eq!(totals.finish_to_ui_max_ms, 16.);
        assert_eq!(totals.same_frame_next_dispatches, 0);
        for received_ms in [15., f64::NAN] {
            totals.record(&PacingSample {
                received_ms,
                ..sample.clone()
            });
        }
        assert_eq!(totals.invalid_samples, 2);
        assert_eq!(totals.requests, 2);
        assert_eq!(totals.worker_execution_ms, 6.);
    }

    #[test]
    fn large_overview_covers_every_pixel_in_bounded_source_tile_groups() {
        let m = manifest();
        let mut demands = Vec::new();
        let mut views = Vec::new();
        let region = ImageRegion {
            x: 0,
            y: 0,
            width: 43008,
            height: 43008,
        };
        add_view(
            &m,
            region,
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600., 800.)),
            PaneId(1),
            RegionalDisplaySettings::default(),
            None,
            &mut demands,
            &mut views,
        );
        assert_eq!(demands.len(), 121);
        let area: u64 = demands
            .iter()
            .map(|d| u64::from(d.key.region.width) * u64::from(d.key.region.height))
            .sum();
        assert_eq!(area, 43008u64 * 43008);
        for demand in &demands {
            assert!(demand.key.region.width <= 4096 && demand.key.region.height <= 4096);
            assert!(demand.max_decoded_bytes <= 64 * 64 * 3 * 4);
        }
        assert_eq!(views[0].draws.len(), 121);
        // A fresh large view presents its primary overview before gallery chips.
        let primary_keys: std::collections::BTreeSet<_> =
            demands.iter().map(|d| d.key.clone()).collect();
        for index in 0..26 {
            demands.push(demand(
                &m,
                detection(&m, index),
                2,
                100 + index as u64,
                DemandPriority::Visible,
            ));
        }
        let mut runtime = RegionalRuntime::new(RegionalRuntimeLimits {
            max_demands: 1024,
            max_in_flight: 1,
            decoded_bytes: 16 << 20,
        });
        runtime.reconcile(1, demands).unwrap();
        assert!(primary_keys.contains(&runtime.dispatch()[0].key));
    }
    #[test]
    fn gallery_materialisation_and_detail_keep_parent_identity() {
        let m = manifest();
        let grid = layout_virtual_grid(LOGICAL_DETECTIONS, 2, 1200..1207, 2);
        assert_eq!(grid.materialised_items.len(), 22);
        for i in grid.materialised_items {
            let region = detection(&m, i);
            let thumb = demand(&m, region, 2, 100 + i as u64, DemandPriority::Visible);
            let detail = demand(&m, region, 0, 2, DemandPriority::Visible);
            assert_eq!(thumb.key.representation, detail.key.representation);
            assert_eq!(thumb.key.region, detail.key.region);
            assert!(thumb.max_decoded_bytes < detail.max_decoded_bytes);
            assert!(region.x + region.width <= m.identity.profile.width);
        }
    }
}
