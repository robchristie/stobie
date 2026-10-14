//! Renderer and tile-runtime metrics for diagnostics panes.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RenderMetrics {
    pub gpu_viewports: usize,
    pub render_jobs: usize,
    pub paint_callbacks: usize,
    pub draw_calls: usize,
    pub returned_command_buffers: usize,
    pub actual_render_passes: Option<usize>,
    pub uploaded_bytes: usize,
    pub pending_upload_bytes: usize,
    pub resident_texture_bytes: usize,
    pub cache_evictions: u64,
    pub prepare_ms: f64,
    pub capability_profile: String,
    pub gpu_timestamp_ms: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LatencySummary {
    pub samples: u64,
    pub min: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
    #[serde(skip)]
    recent_samples: VecDeque<f64>,
}

impl LatencySummary {
    /// Retain a bounded recent reservoir and calculate real nearest-rank percentiles.
    pub fn record(&mut self, value: f64) {
        const CAPACITY: usize = 128;
        if self.recent_samples.len() == CAPACITY {
            self.recent_samples.pop_front();
        }
        self.recent_samples.push_back(value);
        self.samples = self.recent_samples.len() as u64;
        let mut values: Vec<_> = self.recent_samples.iter().copied().collect();
        values.sort_by(f64::total_cmp);
        self.min = values[0];
        self.max = *values.last().unwrap();
        self.p50 = values[(values.len() - 1) / 2];
        self.p95 = values[((values.len() - 1) * 95).div_ceil(100)];
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerHealth {
    #[default]
    Starting,
    Running,
    Unavailable,
    Stopped,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RuntimeMetrics {
    pub total_demands: usize,
    pub visible_demands: usize,
    pub prefetch_demands: usize,
    pub duplicate_demands_removed: usize,
    pub stale_demands_rejected: u64,
    pub invalid_demands_rejected: u64,
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
    pub desired: usize,
    pub decoded: usize,
    pub decoded_bytes: usize,
    pub scheduler_capacity: usize,
    pub external_queue_capacity: usize,
    pub browser_credit_capacity: usize,
    pub browser_credits_in_use: usize,
    pub native_queue_depth: usize,
    pub scheduler_high_water: usize,
    pub external_queue_high_water: usize,
    pub native_queue_high_water: usize,
    pub deferred_dispatches: u64,
    pub deferred_completions: u64,
    pub completion_unknown: u64,
    pub completion_obsolete: u64,
    pub completion_superseded: u64,
    pub completion_duplicate: u64,
    pub residency_rejected: u64,
    pub worker_health: WorkerHealth,
    pub worker_failures: u64,
    pub last_worker_error: String,
    pub preparation_latency_ms: LatencySummary,
    pub decode_latency_ms: LatencySummary,
    pub end_to_end_latency_ms: LatencySummary,
}
