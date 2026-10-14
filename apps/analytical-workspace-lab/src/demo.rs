//! The Lab's demo state: its layout, image panes and synthetic results.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use stobie_core::{DockNode, DockNodeId, LAYOUT_SCHEMA_VERSION, PaneId, SplitAxis, Workspace};
use stobie_imaging::{ActiveTool, Camera, CameraState, ImagePoint, LinkGroupId, ResultId, Session};

pub(crate) const RESULT_COUNT: u64 = 1_000_000;
pub(crate) const THUMBNAIL_COUNT: u64 = 100_000;

/// The image panes 1–4. Panes 1 and 2 start with linked cameras.
pub(crate) fn image_panes() -> BTreeSet<PaneId> {
    (1..=4).map(PaneId).collect()
}

/// The starting session: a camera and the navigate tool for each image pane.
pub(crate) fn default_session() -> Session {
    let linked = Some(LinkGroupId(1));
    Session {
        cameras: image_panes()
            .into_iter()
            .map(|pane| CameraState {
                pane,
                camera: Camera::default(),
                link: if pane.0 <= 2 { linked } else { None },
            })
            .collect(),
        active_tools: image_panes()
            .into_iter()
            .map(|pane| (pane, ActiveTool::Navigate))
            .collect(),
        ..Session::default()
    }
}

/// Four image panes in a 2×2 grid beside results/thumbnails and
/// inspector/diagnostics tab groups.
pub(crate) fn analytical_workspace() -> Workspace {
    let tabs = |id, ids: &[u32]| DockNode::Tabs {
        id: DockNodeId(id),
        tabs: ids.iter().copied().map(PaneId).collect(),
        active: 0,
    };
    Workspace {
        schema_version: LAYOUT_SCHEMA_VERSION,
        active_pane: PaneId(1),
        closed_optional_panes: BTreeSet::new(),
        next_node_id: 12,
        root: DockNode::Split {
            id: DockNodeId(1),
            axis: SplitAxis::Horizontal,
            fraction: 0.72,
            first: Box::new(DockNode::Split {
                id: DockNodeId(2),
                axis: SplitAxis::Vertical,
                fraction: 0.68,
                first: Box::new(DockNode::Split {
                    id: DockNodeId(3),
                    axis: SplitAxis::Horizontal,
                    fraction: 0.5,
                    first: Box::new(tabs(4, &[1])),
                    second: Box::new(tabs(5, &[2])),
                }),
                second: Box::new(DockNode::Split {
                    id: DockNodeId(6),
                    axis: SplitAxis::Horizontal,
                    fraction: 0.5,
                    first: Box::new(tabs(7, &[3])),
                    second: Box::new(tabs(8, &[4])),
                }),
            }),
            second: Box::new(DockNode::Split {
                id: DockNodeId(9),
                axis: SplitAxis::Vertical,
                fraction: 0.52,
                first: Box::new(tabs(10, &[5, 6])),
                second: Box::new(tabs(11, &[7, 8])),
            }),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ResultRecord {
    pub id: ResultId,
    pub position: ImagePoint,
    pub confidence: f32,
    pub category: u8,
}

/// One synthetic result, generated on demand so the million-row collection is
/// never allocated.
pub(crate) fn result_at(index: u64) -> ResultRecord {
    let mixed = index.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(17);
    ResultRecord {
        id: ResultId(index),
        position: ImagePoint::new((mixed & 0x1_FFFF) as f64, ((mixed >> 21) & 0x1_FFFF) as f64),
        confidence: ((mixed >> 42) & 0xffff) as f32 / 65535.0,
        category: ((mixed >> 58) & 0x3) as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_collections_are_generated_on_demand() {
        assert_eq!(RESULT_COUNT, 1_000_000);
        assert_eq!(THUMBNAIL_COUNT, 100_000);
        assert_eq!(result_at(7), result_at(7));
    }

    #[test]
    fn default_layout_has_stable_unique_panes_and_valid_cameras() {
        let workspace = analytical_workspace();
        workspace.validate().unwrap();
        let mut panes = Vec::new();
        workspace.root.pane_ids(&mut panes);
        assert_eq!(panes, (1..=8).map(PaneId).collect::<Vec<_>>());
        let mut session = default_session();
        assert!(session.validate_cameras(&image_panes()).is_ok());
        session.cameras.pop();
        assert!(session.validate_cameras(&image_panes()).is_err());
    }
}
