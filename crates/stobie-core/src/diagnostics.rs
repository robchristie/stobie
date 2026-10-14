use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::PaneId;

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
    Preferences,
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
    pub physical_wheel_events: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceMetrics {
    pub registered_panes: usize,
    pub visible_panes: usize,
    pub active_pane: Option<PaneId>,
    pub dock_nodes: usize,
    pub serialised_bytes: usize,
}
