use eframe::egui;
use polyorama_core::{DockNodeId, PaneId, ResultId};
pub use polyorama_ui_egui::UiRect;
use polyorama_ui_egui::{
    ActionTarget, Availability, SemanticUiId, TextAuditFinding, TextLayoutObservation, UiNode,
    UiRole, UiSnapshot, action_semantic_node,
};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PaneUiRect {
    pub pane: PaneId,
    pub rect: UiRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct SplitterUiRect {
    pub node: DockNodeId,
    pub rect: UiRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ControlUiRect {
    pub pane: Option<PaneId>,
    pub name: &'static str,
    pub rect: UiRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ResultUiRect {
    pub result: ResultId,
    pub rect: UiRect,
}

/// Current-frame Rust-owned geometry for physical smoke automation.
///
/// Coordinates are egui logical points in the root application surface. They
/// are observations only and never participate in authoritative workspace or
/// document state.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct UiGeometry {
    pub pixels_per_point: f32,
    pub root: Option<UiRect>,
    pub menu: Option<UiRect>,
    pub tabs: Vec<PaneUiRect>,
    pub pane_bodies: Vec<PaneUiRect>,
    pub splitters: Vec<SplitterUiRect>,
    pub image_toolbars: Vec<PaneUiRect>,
    pub image_viewports: Vec<PaneUiRect>,
    pub controls: Vec<ControlUiRect>,
    pub result_rows: Vec<ResultUiRect>,
    pub results_scroll: Option<UiRect>,
    pub thumbnail_scroll: Option<UiRect>,
    /// Bounded observations for Polyorama-owned text components only.
    pub text_layouts: Vec<TextLayoutObservation>,
    pub text_audit: Vec<TextAuditFinding>,
    /// Bounded semantic observations for the current frame only.
    pub semantic_nodes: Vec<UiNode>,
}

impl UiGeometry {
    pub fn new(root: egui::Rect, pixels_per_point: f32) -> Self {
        let root_rect = root.into();
        Self {
            pixels_per_point,
            root: Some(root_rect),
            semantic_nodes: vec![UiNode::container(
                SemanticUiId::root(),
                None,
                UiRole::Application,
                root_rect,
            )],
            ..Self::default()
        }
    }

    pub fn control(&mut self, pane: Option<PaneId>, name: &'static str, rect: egui::Rect) {
        self.controls.push(ControlUiRect {
            pane,
            name,
            rect: rect.into(),
        });
    }

    pub fn record_node(&mut self, node: UiNode) {
        self.semantic_nodes.push(node);
    }

    pub fn action(
        &mut self,
        parent: SemanticUiId,
        target: ActionTarget,
        availability: &Availability,
        selected: bool,
        response: &egui::Response,
    ) {
        self.record_node(action_semantic_node(
            response,
            target,
            availability,
            selected,
            parent,
        ));
    }

    pub fn snapshot(&self, frame: u64) -> UiSnapshot {
        let mut snapshot = UiSnapshot {
            frame,
            pixels_per_point: self.pixels_per_point,
            root: SemanticUiId::root(),
            nodes: self.semantic_nodes.clone(),
            text: self.text_layouts.clone(),
            text_audit: self.text_audit.clone(),
            semantic_audit: Vec::new(),
        };
        snapshot.semantic_audit = snapshot.audit();
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_rect_reports_a_finite_centre() {
        let rect: UiRect =
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(90.0, 70.0)).into();
        assert!(
            [rect.min_x, rect.min_y, rect.max_x, rect.max_y]
                .into_iter()
                .all(f32::is_finite)
        );
        assert!(rect.max_x > rect.min_x && rect.max_y > rect.min_y);
        assert_eq!(
            (
                (rect.min_x + rect.max_x) * 0.5,
                (rect.min_y + rect.max_y) * 0.5
            ),
            (50.0, 45.0)
        );
    }

    #[test]
    fn geometry_serialises_the_bounded_text_observation_surface() {
        let geometry = UiGeometry::default();
        let value = serde_json::to_value(geometry).unwrap();
        assert_eq!(value["text_layouts"], serde_json::json!([]));
        assert_eq!(value["text_audit"], serde_json::json!([]));
    }

    #[test]
    fn current_geometry_promotes_to_a_reusable_semantic_snapshot() {
        let geometry = UiGeometry::new(
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            1.25,
        );
        let snapshot = geometry.snapshot(42);
        assert_eq!(snapshot.frame, 42);
        assert_eq!(snapshot.pixels_per_point, 1.25);
        assert_eq!(snapshot.nodes.len(), 1);
        assert!(snapshot.semantic_audit.is_empty());
    }
}
