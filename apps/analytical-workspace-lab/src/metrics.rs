//! The Lab's diagnostics snapshot and virtualisation metrics.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use stobie_core::{FrameMetrics, WorkspaceMetrics};
use stobie_imaging::{CameraState, RenderMetrics, RuntimeMetrics};

use crate::demo::{RESULT_COUNT, THUMBNAIL_COUNT};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VirtualisationMetrics {
    pub result_count: u64,
    pub visible_rows: (usize, usize),
    pub materialised_rows: usize,
    pub row_overscan: usize,
    pub thumbnail_count: u64,
    pub visible_thumbnails: (usize, usize),
    pub materialised_thumbnails: usize,
    pub materialised_thumbnail_range: (usize, usize),
    pub thumbnail_columns: usize,
    pub thumbnail_total_rows: usize,
    pub thumbnail_scroll_offset_y: f32,
    pub thumbnail_content_height: f32,
    pub thumbnail_viewport_height: f32,
    pub thumbnail_wheel_input_frames: u64,
    pub thumbnail_wheel_delta_y: f32,
    pub resident_thumbnails: usize,
    pub thumbnail_cache_bytes: usize,
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
            materialised_thumbnail_range: (0, 0),
            thumbnail_columns: 0,
            thumbnail_total_rows: 0,
            thumbnail_scroll_offset_y: 0.0,
            thumbnail_content_height: 0.0,
            thumbnail_viewport_height: 0.0,
            thumbnail_wheel_input_frames: 0,
            thumbnail_wheel_delta_y: 0.0,
            resident_thumbnails: 0,
            thumbnail_cache_bytes: 0,
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
    /// How the host chose the backend, such as `fallback: WebGPU returned no adapter`.
    #[serde(default)]
    pub renderer_selection: String,
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
