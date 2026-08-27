use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{CameraState, PaneId, RESULT_COUNT, THUMBNAIL_COUNT};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepaintReason {
    #[default]
    None,
    Interaction,
    Command,
    WorkerCompletion,
    PendingUpload,
    Animation,
    Scheduled,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FrameMetrics {
    pub frame_number: u64,
    pub cpu_frame_ms: f64,
    pub cpu_frame_history_ms: VecDeque<f64>,
    pub runtime_poll_ms: f64,
    pub ui_ms: f64,
    pub demand_ms: f64,
    pub render_prepare_ms: f64,
    pub repaint_requests: u64,
    pub repaint_reason: RepaintReason,
    pub recent_reasons: VecDeque<RepaintReason>,
    pub interaction_active: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceMetrics {
    pub registered_panes: usize,
    pub visible_panes: usize,
    pub active_pane: Option<PaneId>,
    pub dock_nodes: usize,
    pub serialised_bytes: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RenderMetrics {
    pub gpu_viewports: usize,
    pub render_jobs: usize,
    pub render_passes: usize,
    pub draw_calls: usize,
    pub command_buffers: usize,
    pub uploaded_bytes: usize,
    pub pending_upload_bytes: usize,
    pub resident_texture_bytes: usize,
    pub cache_evictions: u64,
    pub prepare_ms: f64,
    pub capability_profile: String,
    pub gpu_timestamp_ms: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    pub total_demands: usize,
    pub visible_demands: usize,
    pub prefetch_demands: usize,
    pub duplicate_demands_removed: usize,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub evictions: u64,
    pub queued: usize,
    pub in_flight: usize,
    pub completed: u64,
    pub failed: u64,
    pub stale_discarded: u64,
    pub decode_latency_ms_median: f64,
    pub worker_queue_depth: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VirtualisationMetrics {
    pub result_count: u64,
    pub visible_rows: (usize, usize),
    pub materialised_rows: usize,
    pub row_overscan: usize,
    pub thumbnail_count: u64,
    pub visible_thumbnails: (usize, usize),
    pub materialised_thumbnails: usize,
}

impl Default for VirtualisationMetrics {
    fn default() -> Self {
        Self {
            result_count: RESULT_COUNT,
            visible_rows: (0, 0),
            materialised_rows: 0,
            row_overscan: 8,
            thumbnail_count: THUMBNAIL_COUNT,
            visible_thumbnails: (0, 0),
            materialised_thumbnails: 0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiagnosticsSnapshot {
    pub application_version: String,
    pub dependency_versions: BTreeMap<String, String>,
    pub platform: String,
    pub backend: String,
    pub adapter: String,
    pub frame: FrameMetrics,
    pub workspace: WorkspaceMetrics,
    pub render: RenderMetrics,
    pub runtime: RuntimeMetrics,
    pub cameras: Vec<CameraState>,
    pub virtualisation: VirtualisationMetrics,
    pub tile_cache_budget_bytes: usize,
    pub upload_budget_bytes: usize,
}

impl DiagnosticsSnapshot {
    pub fn json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
