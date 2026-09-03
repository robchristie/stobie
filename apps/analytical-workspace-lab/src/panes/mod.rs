#![allow(clippy::collapsible_if)] // Nested event guards mirror the gesture state machine.

use std::{collections::BTreeMap, time::Duration};

use eframe::egui;
use polyorama_core::*;
use polyorama_render_wgpu::{
    DisplayMap, DisplaySettings, ImageRenderRequest, PhysicalViewport, RenderPlan,
};
use polyorama_ui_egui::{
    ActionButtonSpec, ActionEmphasis, ActionKey, ActionScope, ActionTarget, Availability,
    DesignTokens, DomainReference, ImagePlanTarget, ImageStatusSpec, PanePresenter, SemanticUiId,
    TextLayoutObservation, UiNode, UiRole, action_button, allocate_viewport, choice_control,
    consume_action_shortcut, diagnostic_row, image_status_height, paint_image_status,
    range_control, section_heading, stage_render_callback,
};
use web_time::Instant;

use crate::actions::LabAction;
use crate::thumbnail_cache::ThumbnailCache;
use crate::ui_geometry::{PaneUiRect, SplitterUiRect, UiGeometry};

mod annotations;
mod camera_gestures;
mod diagnostics;
mod image;
mod inspector;
mod results;
mod thumbnails;

use annotations::paint_image_overlay;
#[cfg(test)]
use camera_gestures::{CameraGestureKey, WHEEL_GESTURE_IDLE, drag_pointer_sample};
pub use camera_gestures::{UiBehaviour, should_cancel_camera_drag};
use camera_gestures::{derive_image_frame, image_demands};

#[derive(Default)]
pub struct FrameOutput {
    pub intents: Vec<ImageIntent>,
    pub pane_intents: Vec<PaneIntent>,
    pub commands: Vec<Command>,
    pub demands: Vec<TileDemand>,
    pub render_plan: RenderPlan,
    pub render_targets: Vec<ImagePlanTarget>,
    overlays: Vec<ImageOverlayRequest>,
    annotation_release_preview: Option<GesturePreview>,
    statuses: Vec<ImageStatusRequest>,
    pub interaction_active: bool,
    pub repaint_after: Option<Duration>,
    pub ui_geometry: UiGeometry,
}

#[allow(clippy::too_many_arguments)]
fn present_action(
    ui: &mut egui::Ui,
    outputs: &mut FrameOutput,
    tokens: &DesignTokens,
    font_scale: f32,
    parent: &SemanticUiId,
    target: ActionTarget<LabAction>,
    availability: Availability,
    selected: bool,
    compact: bool,
    active_pane: bool,
    legacy_name: &'static str,
) -> bool {
    if !availability.visible()
        || (target.action.specification().scope == ActionScope::ActivePane && !active_pane)
    {
        return false;
    }
    let response = action_button(
        ui,
        ActionButtonSpec {
            target,
            availability: availability.clone(),
            selected,
            emphasis: ActionEmphasis::Quiet,
            compact,
        },
        tokens,
        font_scale,
        &mut outputs.ui_geometry.text_layouts,
    );
    let inside_root = outputs
        .ui_geometry
        .root
        .is_some_and(|root| root.contains(response.rect.into(), 1.0));
    if inside_root && response.rect.intersects(ui.clip_rect()) {
        outputs
            .ui_geometry
            .control(target.pane, legacy_name, response.rect);
        outputs
            .ui_geometry
            .action(parent.clone(), target, &availability, selected, &response);
    }
    availability.enabled()
        && (response.clicked() || consume_action_shortcut(ui, target.action, active_pane))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaneIntent {
    SetActiveTool {
        pane: PaneId,
        tool: ActiveTool,
    },
    SelectAnnotation(Option<AnnotationId>),
    SetDisplay {
        pane: PaneId,
        settings: DisplaySettings,
    },
}

pub struct AnnotationUiState<'a> {
    gesture: &'a mut Option<GesturePreview>,
}

impl<'a> AnnotationUiState<'a> {
    pub fn new(gesture: &'a mut Option<GesturePreview>) -> Self {
        Self { gesture }
    }

    fn get(&self) -> Option<&GesturePreview> {
        self.gesture.as_ref()
    }

    fn get_mut(&mut self) -> Option<&mut GesturePreview> {
        self.gesture.as_mut()
    }

    fn take(&mut self) -> Option<GesturePreview> {
        self.gesture.take()
    }

    fn set(&mut self, gesture: GesturePreview) {
        *self.gesture = Some(gesture);
    }

    fn cloned(&self) -> Option<GesturePreview> {
        self.gesture.clone()
    }
}

#[derive(Clone)]
struct ImageOverlayRequest {
    pane: PaneId,
    layer_id: egui::LayerId,
    rect: egui::Rect,
    annotations: Vec<Polygon>,
    gesture: Option<GesturePreview>,
    selected_annotation: Option<AnnotationId>,
    hover: Option<egui::Pos2>,
    tokens: DesignTokens,
}

struct ImageStatusRequest {
    pane: PaneId,
    layer_id: egui::LayerId,
    rect: egui::Rect,
    viewport: egui::Rect,
    pointer_local: Option<ViewportPoint>,
    fallback_pointer: ImagePoint,
    tokens: DesignTokens,
    font_scale: f32,
}

impl FrameOutput {
    pub fn with_ui_geometry(ui_geometry: UiGeometry) -> Self {
        Self {
            ui_geometry,
            ..Self::default()
        }
    }

    pub fn finalise_camera_previews(
        &mut self,
        root_ui: &mut egui::Ui,
        behaviour: &UiBehaviour,
        committed: &[CameraState],
        generation: u64,
    ) {
        let context = root_ui.ctx().clone();
        self.demands
            .retain(|demand| demand.key.source != SourceId(1));
        for request in &mut self.render_plan.images {
            request.camera = behaviour.camera(request.pane, committed);
            request.source_generation = generation;
            let viewport = (
                request.viewport.size.x / f64::from(request.viewport.scale_factor),
                request.viewport.size.y / f64::from(request.viewport.scale_factor),
            );
            let demands = image_demands(request.camera, viewport, generation);
            request.desired_tiles = demands.iter().map(|demand| demand.key).collect();
            self.demands.extend(demands);
        }
        for overlay in &self.overlays {
            let camera = behaviour.camera(overlay.pane, committed);
            let painter = context
                .layer_painter(overlay.layer_id)
                .with_clip_rect(overlay.rect);
            paint_image_overlay(
                &painter,
                overlay,
                self.annotation_gesture_for(overlay),
                camera,
                behaviour.camera(PaneId(1), committed),
                &overlay.tokens,
            );
        }
        for status in &self.statuses {
            let camera = behaviour.camera(status.pane, committed);
            let pointer = status
                .pointer_local
                .map(|pointer| {
                    camera.image_at(
                        pointer,
                        ViewportPoint::new(
                            status.viewport.width() as f64,
                            status.viewport.height() as f64,
                        ),
                    )
                })
                .unwrap_or(status.fallback_pointer);
            let world = ImageToWorld::default().image_to_world(pointer);
            let tile_count = self
                .render_plan
                .images
                .iter()
                .find(|request| request.pane == status.pane)
                .map_or(0, |request| request.desired_tiles.len());
            let coordinates = format!(
                "image {:>8.1}, {:>8.1}  ·  world {:>10.1}, {:>10.1}",
                pointer.x, pointer.y, world.x, world.y
            );
            let detail = format!(
                "L{} · {} tiles",
                camera.pixels_per_screen_point.log2().round().max(0.0) as u8,
                tile_count
            );
            let status_id = format!("pane.{}.image_status", status.pane.0);
            if let Some(node) = self
                .ui_geometry
                .semantic_nodes
                .iter_mut()
                .find(|node| node.id.0 == status_id)
            {
                node.description = Some(format!("{coordinates}; {detail}"));
            }
            let mut status_ui = root_ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("polyorama.image-status", status.pane))
                    .layer_id(status.layer_id)
                    .max_rect(status.rect),
            );
            status_ui.set_clip_rect(status.rect);
            self.ui_geometry.text_layouts.extend(paint_image_status(
                &mut status_ui,
                ImageStatusSpec {
                    instance: u64::from(status.pane.0),
                    rect: status.rect,
                    coordinates: &coordinates,
                    detail: &detail,
                },
                &status.tokens,
                status.font_scale,
            ));
        }
    }

    fn finish_vertex_drag(&mut self, gesture: GesturePreview) {
        let GesturePreview::Vertex {
            annotation,
            vertex,
            original,
            preview,
        } = &gesture
        else {
            return;
        };
        self.commands.push(Command::MoveVertex {
            annotation: *annotation,
            vertex: *vertex,
            before: *original,
            after: *preview,
        });
        self.annotation_release_preview = Some(gesture);
    }

    fn annotation_gesture_for<'a>(
        &'a self,
        overlay: &'a ImageOverlayRequest,
    ) -> Option<&'a GesturePreview> {
        self.annotation_release_preview
            .as_ref()
            .or(overlay.gesture.as_ref())
    }
}

pub struct PaneReadModel<'a> {
    pub document: &'a Document,
    pub cameras: &'a [CameraState],
    pub active_tools: BTreeMap<PaneId, ActiveTool>,
    pub selected_result: Option<ResultId>,
    pub selected_annotation: Option<AnnotationId>,
    pub display: BTreeMap<PaneId, DisplaySettings>,
    pub diagnostics: &'a DiagnosticsSnapshot,
    pub generation: u64,
    pub frame_number: u64,
    pub active_pane: PaneId,
    pub tokens: DesignTokens,
    pub font_scale: f32,
}

pub struct PaneFeatureState<'a> {
    pub annotation_ui: AnnotationUiState<'a>,
    pub ui_behaviour: &'a mut UiBehaviour,
    pub virtualisation: &'a mut VirtualisationMetrics,
    pub thumbnail_cache: &'a mut ThumbnailCache,
    pub outputs: &'a mut FrameOutput,
}

pub struct PaneSurface<'a> {
    document: &'a Document,
    cameras: &'a [CameraState],
    active_tools: BTreeMap<PaneId, ActiveTool>,
    annotation_ui: AnnotationUiState<'a>,
    selected_result: Option<ResultId>,
    selected_annotation: Option<AnnotationId>,
    ui_behaviour: &'a mut UiBehaviour,
    display: BTreeMap<PaneId, DisplaySettings>,
    diagnostics: &'a DiagnosticsSnapshot,
    virtualisation: &'a mut VirtualisationMetrics,
    thumbnail_cache: &'a mut ThumbnailCache,
    generation: u64,
    frame_number: u64,
    active_pane: PaneId,
    tokens: DesignTokens,
    font_scale: f32,
    outputs: &'a mut FrameOutput,
}

impl<'a> PaneSurface<'a> {
    pub fn new(read: PaneReadModel<'a>, feature: PaneFeatureState<'a>) -> Self {
        Self {
            document: read.document,
            cameras: read.cameras,
            active_tools: read.active_tools,
            annotation_ui: feature.annotation_ui,
            selected_result: read.selected_result,
            selected_annotation: read.selected_annotation,
            ui_behaviour: feature.ui_behaviour,
            display: read.display,
            diagnostics: read.diagnostics,
            virtualisation: feature.virtualisation,
            thumbnail_cache: feature.thumbnail_cache,
            generation: read.generation,
            frame_number: read.frame_number,
            active_pane: read.active_pane,
            tokens: read.tokens,
            font_scale: read.font_scale,
            outputs: feature.outputs,
        }
    }

    pub fn push_shell_command(&mut self, command: Command) {
        self.outputs.commands.push(command);
    }

    pub fn record_shell_interaction(&mut self, active: bool) {
        self.outputs.interaction_active |= active;
    }
}

impl PanePresenter for PaneSurface<'_> {
    fn title(&self, pane: PaneId) -> &'static str {
        match pane.0 {
            1 => "Primary View",
            2 => "Linked Detail",
            3 => "Overview",
            4 => "Derived View",
            5 => "Results",
            6 => "Thumbnails",
            7 => "Inspector",
            8 => "Diagnostics",
            _ => "Unknown pane",
        }
    }

    fn pane_ui(&mut self, ui: &mut egui::Ui, pane: PaneId, pane_rect: egui::Rect) {
        self.outputs.ui_geometry.pane_bodies.push(PaneUiRect {
            pane,
            rect: pane_rect.into(),
        });
        let mut pane_node = UiNode::container(
            SemanticUiId::pane(pane),
            Some(SemanticUiId::root()),
            UiRole::Pane,
            pane_rect.into(),
        );
        pane_node.name = self.title(pane).to_owned();
        pane_node.pane = Some(pane);
        pane_node.domain_reference = Some(DomainReference::Pane(pane));
        self.outputs.ui_geometry.record_node(pane_node);
        ui.push_id(("window", 1_u32, "pane", pane.0), |ui| match pane.0 {
            1..=4 => self.image_pane(ui, pane, pane_rect),
            5 => self.results_pane(ui),
            6 => self.thumbnails_pane(ui),
            7 => self.inspector_pane(ui),
            8 => self.diagnostics_pane(ui),
            _ => {
                polyorama_ui_egui::measured_content_label(
                    ui,
                    u64::from(pane.0),
                    "Unknown pane",
                    polyorama_ui_egui::TextRole::Error,
                    polyorama_ui_egui::TextOverflow::Wrap,
                    2,
                    polyorama_ui_egui::TextInteraction::Selectable,
                    &self.tokens,
                    self.font_scale,
                    &mut self.outputs.ui_geometry.text_layouts,
                );
            }
        });
    }

    fn record_tab_rect(&mut self, pane: PaneId, rect: egui::Rect, selected: bool, focused: bool) {
        self.outputs.ui_geometry.tabs.push(PaneUiRect {
            pane,
            rect: rect.into(),
        });
        self.outputs.ui_geometry.record_node(UiNode {
            id: SemanticUiId::tab(pane),
            parent: Some(SemanticUiId::root()),
            role: UiRole::Tab,
            name: self.title(pane).to_owned(),
            description: None,
            rect: rect.into(),
            enabled: true,
            focused,
            selected,
            checked: None,
            expanded: None,
            pane: Some(pane),
            domain_reference: Some(DomainReference::Pane(pane)),
            actions: Vec::new(),
            text_selectable: false,
            disabled_reason: None,
        });
    }

    fn record_text_layout(&mut self, observation: TextLayoutObservation) {
        self.outputs.ui_geometry.text_layouts.push(observation);
    }

    fn record_splitter_rect(
        &mut self,
        node: DockNodeId,
        rect: egui::Rect,
        horizontal: bool,
        focused: bool,
    ) {
        self.outputs.ui_geometry.splitters.push(SplitterUiRect {
            node,
            rect: rect.into(),
        });
        self.outputs.ui_geometry.record_node(UiNode {
            id: SemanticUiId::splitter(node),
            parent: Some(SemanticUiId::root()),
            role: UiRole::Splitter,
            name: if horizontal {
                "Vertical splitter".into()
            } else {
                "Horizontal splitter".into()
            },
            description: Some("Resize adjacent dock panes".into()),
            rect: rect.into(),
            enabled: true,
            focused,
            selected: false,
            checked: None,
            expanded: None,
            pane: None,
            domain_reference: Some(DomainReference::DockNode(node)),
            actions: Vec::new(),
            text_selectable: false,
            disabled_reason: None,
        });
    }
}

impl PaneSurface<'_> {
    fn results_pane(&mut self, ui: &mut egui::Ui) {
        results::show(
            ui,
            self.selected_result,
            self.virtualisation,
            &self.tokens,
            self.font_scale,
            self.active_pane,
            self.outputs,
        );
    }

    fn thumbnails_pane(&mut self, ui: &mut egui::Ui) {
        thumbnails::show(
            ui,
            thumbnails::ThumbnailPaneView {
                selected_result: self.selected_result,
                generation: self.generation,
                tokens: &self.tokens,
                font_scale: self.font_scale,
            },
            self.thumbnail_cache,
            self.virtualisation,
            self.outputs,
        );
    }

    fn inspector_pane(&mut self, ui: &mut egui::Ui) {
        inspector::show(
            ui,
            self.selected_result,
            self.selected_annotation,
            &self.tokens,
            self.font_scale,
            self.active_pane,
            self.outputs,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn camera_with_centre(x: f64, y: f64) -> Camera {
        Camera {
            centre: ImagePoint::new(x, y),
            ..Camera::default()
        }
    }

    #[test]
    fn coalesced_final_pointer_move_and_release_is_sampled_before_drag_completion() {
        use std::{cell::RefCell, rc::Rc};

        fn frame(
            context: &egui::Context,
            events: Vec<egui::Event>,
        ) -> (bool, Option<ViewportPoint>) {
            let observed = Rc::new(RefCell::new(None));
            let output = observed.clone();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(200.0, 160.0),
                )),
                events,
                focused: true,
                ..Default::default()
            };
            let mut full_output = context.run_ui(input, |ui| {
                let response =
                    ui.allocate_response(egui::vec2(180.0, 140.0), egui::Sense::click_and_drag());
                *output.borrow_mut() =
                    Some((response.drag_stopped(), drag_pointer_sample(&response)));
            });
            full_output.textures_delta.clear();
            observed.borrow_mut().take().unwrap()
        }

        let context = egui::Context::default();
        let modifiers = egui::Modifiers::NONE;
        let origin = egui::pos2(10.0, 20.0);
        frame(&context, vec![egui::Event::PointerMoved(origin)]);
        frame(
            &context,
            vec![
                egui::Event::PointerMoved(origin),
                egui::Event::PointerButton {
                    pos: origin,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers,
                },
            ],
        );
        frame(
            &context,
            vec![egui::Event::PointerMoved(egui::pos2(40.0, 30.0))],
        );

        let final_position = egui::pos2(100.0, 70.0);
        let (stopped, final_sample) = frame(
            &context,
            vec![
                egui::Event::PointerMoved(final_position),
                egui::Event::PointerButton {
                    pos: final_position,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers,
                },
            ],
        );
        assert!(stopped);
        assert_eq!(final_sample, Some(ViewportPoint::new(100.0, 70.0)));
    }

    #[test]
    fn delayed_vertex_drag_recognition_retains_the_press_origin_for_hit_testing() {
        use std::{cell::RefCell, rc::Rc};

        fn frame(context: &egui::Context, events: Vec<egui::Event>) -> Option<egui::Pos2> {
            let observed = Rc::new(RefCell::new(None));
            let output = observed.clone();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(200.0, 160.0),
                )),
                events,
                focused: true,
                ..Default::default()
            };
            let mut full_output = context.run_ui(input, |ui| {
                let response =
                    ui.allocate_response(egui::vec2(180.0, 140.0), egui::Sense::click_and_drag());
                *output.borrow_mut() = annotations::drag_start_pointer_sample(&response);
            });
            full_output.textures_delta.clear();
            *observed.borrow()
        }

        let context = egui::Context::default();
        let origin = egui::pos2(30.0, 40.0);
        frame(&context, vec![egui::Event::PointerMoved(origin)]);
        frame(
            &context,
            vec![
                egui::Event::PointerMoved(origin),
                egui::Event::PointerButton {
                    pos: origin,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );

        let recognised_at = origin + egui::vec2(35.0, 25.0);
        assert_eq!(
            frame(&context, vec![egui::Event::PointerMoved(recognised_at)]),
            Some(origin)
        );
    }

    #[test]
    fn vertex_release_frame_keeps_the_final_preview_until_the_command_is_visible() {
        let annotation = AnnotationId(7);
        let original = WorldPoint::new(10.0, 20.0);
        let stale_preview = WorldPoint::new(30.0, 40.0);
        let final_preview = WorldPoint::new(50.0, 60.0);
        let mut output = FrameOutput::default();
        output.overlays.push(ImageOverlayRequest {
            pane: PaneId(1),
            layer_id: egui::LayerId::background(),
            rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 480.0)),
            annotations: vec![Polygon {
                id: annotation,
                layer: LayerId(1),
                vertices: vec![
                    original,
                    WorldPoint::new(80.0, 20.0),
                    WorldPoint::new(40.0, 90.0),
                ],
            }],
            gesture: Some(GesturePreview::Vertex {
                annotation,
                vertex: 0,
                original,
                preview: stale_preview,
            }),
            selected_annotation: Some(annotation),
            hover: None,
            tokens: DesignTokens::resolve(
                polyorama_ui_egui::ThemeVariant::Dark,
                polyorama_ui_egui::DensityVariant::Comfortable,
            ),
        });
        let release = GesturePreview::Vertex {
            annotation,
            vertex: 0,
            original,
            preview: final_preview,
        };

        output.finish_vertex_drag(release.clone());

        assert_eq!(
            output.annotation_gesture_for(&output.overlays[0]),
            Some(&release)
        );
        assert_eq!(
            output.commands,
            vec![Command::MoveVertex {
                annotation,
                vertex: 0,
                before: original,
                after: final_preview,
            }]
        );
    }

    #[test]
    fn hidden_wheel_origin_is_finalised_by_the_frame_global_lifecycle() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let started = Instant::now();
        let expected = camera_with_centre(12_000.0, 14_000.0);
        behaviour.update_wheel_gesture(PaneId(1), &committed, expected, started);

        let mut output = FrameOutput::default();
        behaviour.finish_camera_gestures(started + WHEEL_GESTURE_IDLE, &mut output);

        assert_eq!(behaviour.wheel_gesture_count(), 0);
        assert_eq!(output.commands.len(), 1);
        assert!(matches!(
            &output.commands[0],
            Command::SetCameras { changes }
                if changes.len() == 2 && changes.iter().all(|change| change.after == expected)
        ));
    }

    #[test]
    fn wheel_input_crossing_linked_panes_is_one_group_command() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let started = Instant::now();
        behaviour.update_wheel_gesture(
            PaneId(1),
            &committed,
            camera_with_centre(10_000.0, 11_000.0),
            started,
        );
        let final_camera = camera_with_centre(20_000.0, 21_000.0);
        behaviour.update_wheel_gesture(
            PaneId(2),
            &committed,
            final_camera,
            started + Duration::from_millis(20),
        );
        assert_eq!(behaviour.wheel_gesture_count(), 1);

        let mut output = FrameOutput::default();
        behaviour.finish_camera_gestures(
            started + Duration::from_millis(20) + WHEEL_GESTURE_IDLE,
            &mut output,
        );

        assert_eq!(output.commands.len(), 1);
        assert!(matches!(
            &output.commands[0],
            Command::SetCameras { changes }
                if changes.len() == 2 && changes.iter().all(|change| change.after == final_camera)
        ));
    }

    #[test]
    fn unlinked_wheel_gestures_finalise_independently() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let started = Instant::now();
        behaviour.update_wheel_gesture(
            PaneId(3),
            &committed,
            camera_with_centre(30_000.0, 31_000.0),
            started,
        );
        behaviour.update_wheel_gesture(
            PaneId(4),
            &committed,
            camera_with_centre(40_000.0, 41_000.0),
            started,
        );

        let mut output = FrameOutput::default();
        behaviour.finish_camera_gestures(started + WHEEL_GESTURE_IDLE, &mut output);

        assert_eq!(output.commands.len(), 2);
        assert!(output.commands.iter().all(
            |command| matches!(command, Command::SetCameras { changes } if changes.len() == 1)
        ));
    }

    #[test]
    fn wheel_to_drag_handoff_commits_and_reuses_the_preview() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let expected = camera_with_centre(50_000.0, 51_000.0);
        behaviour.update_wheel_gesture(PaneId(1), &committed, expected, Instant::now());
        let mut output = FrameOutput::default();

        let preview = behaviour
            .finish_wheel_key(CameraGestureKey::Link(LinkGroupId(1)), &mut output)
            .unwrap();

        assert_eq!(output.commands.len(), 1);
        assert_eq!(behaviour.wheel_gesture_count(), 0);
        assert!(
            preview
                .iter()
                .filter(|state| state.link == Some(LinkGroupId(1)))
                .all(|state| state.camera == expected)
        );
    }

    #[test]
    fn gesture_preview_is_the_single_camera_for_frame_demand_and_presentation() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let mut preview = committed.clone();
        let expected = Camera {
            centre: ImagePoint::new(8_192.0, 4_096.0),
            pixels_per_screen_point: 4.0,
        };
        let changes = linked_camera_changes(&committed, PaneId(1), expected);
        apply_camera_changes(&mut preview, &changes, true);
        behaviour.expose_preview(&preview);

        let frame = derive_image_frame(&behaviour, PaneId(1), &committed, (640.0, 480.0), 7);
        assert_eq!(frame.camera, expected);
        assert!(frame.demands.iter().all(|demand| demand.generation == 7));
        assert!(frame.demands.iter().any(|demand| demand.key.level == 2));
    }

    #[test]
    fn final_frame_plan_reconciles_an_interaction_from_a_later_linked_pane() {
        let committed = Session::default().cameras;
        let mut behaviour = UiBehaviour::default();
        let expected = Camera {
            centre: ImagePoint::new(12_000.0, 24_000.0),
            pixels_per_screen_point: 8.0,
        };
        let changes = linked_camera_changes(&committed, PaneId(2), expected);
        let mut preview = committed.clone();
        apply_camera_changes(&mut preview, &changes, true);
        behaviour.expose_preview(&preview);

        let mut output = FrameOutput::default();
        for pane in [PaneId(1), PaneId(2)] {
            output.render_plan.submit(ImageRenderRequest {
                pane,
                source: SourceId(1),
                source_generation: 9,
                viewport: PhysicalViewport {
                    origin: PhysicalPoint::new(0.0, 0.0),
                    size: PhysicalPoint::new(640.0, 480.0),
                    scale_factor: 1.0,
                },
                camera: committed
                    .iter()
                    .find(|state| state.pane == pane)
                    .unwrap()
                    .camera,
                display: DisplaySettings::default(),
                desired_tiles: Vec::new(),
            });
        }
        let context = egui::Context::default();
        let mut frame = context.run_ui(Default::default(), |ui| {
            output.finalise_camera_previews(ui, &behaviour, &committed, 9);
        });
        frame.textures_delta.clear();

        assert!(
            output
                .render_plan
                .images
                .iter()
                .all(|request| request.camera == expected && !request.desired_tiles.is_empty())
        );
        assert!(
            output
                .demands
                .iter()
                .all(|demand| demand.key.source == SourceId(1) && demand.generation == 9)
        );
    }

    #[test]
    fn materialised_workspace_semantics_are_bounded_and_match_accesskit() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let root = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1_000.0, 900.0));
        let results_rect = egui::Rect::from_min_max(root.min, egui::pos2(1_000.0, 440.0));
        let thumbnails_rect = egui::Rect::from_min_max(egui::pos2(0.0, 450.0), root.max);
        let mut output = FrameOutput {
            ui_geometry: UiGeometry::new(root, 1.0),
            ..Default::default()
        };
        for (pane, rect, name) in [
            (PaneId(5), results_rect, "Results"),
            (PaneId(6), thumbnails_rect, "Thumbnails"),
        ] {
            let mut node = UiNode::container(
                SemanticUiId::pane(pane),
                Some(SemanticUiId::root()),
                UiRole::Pane,
                rect.into(),
            );
            node.name = name.into();
            node.pane = Some(pane);
            output.ui_geometry.record_node(node);
        }
        let mut virtualisation = VirtualisationMetrics::default();
        let mut thumbnail_cache = ThumbnailCache::default();
        let tokens = polyorama_ui_egui::DesignTokens::resolve(
            polyorama_ui_egui::ThemeVariant::Dark,
            polyorama_ui_egui::DensityVariant::Comfortable,
        );
        let mut full_output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |ui| {
                ui.scope_builder(egui::UiBuilder::new().max_rect(results_rect), |ui| {
                    ui.set_clip_rect(results_rect);
                    results::show(
                        ui,
                        Some(ResultId(0)),
                        &mut virtualisation,
                        &tokens,
                        1.0,
                        PaneId(5),
                        &mut output,
                    );
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(thumbnails_rect), |ui| {
                    ui.set_clip_rect(thumbnails_rect);
                    thumbnails::show(
                        ui,
                        thumbnails::ThumbnailPaneView {
                            selected_result: Some(ResultId(0)),
                            generation: 1,
                            tokens: &tokens,
                            font_scale: 1.0,
                        },
                        &mut thumbnail_cache,
                        &mut virtualisation,
                        &mut output,
                    );
                });
            },
        );
        let accesskit = full_output
            .platform_output
            .accesskit_update
            .take()
            .expect("AccessKit update");
        full_output.textures_delta.clear();
        let snapshot = output.ui_geometry.snapshot(1);

        assert!(
            snapshot.semantic_audit.is_empty(),
            "{:#?}",
            snapshot.semantic_audit
        );
        let result_nodes = snapshot.by_role(UiRole::ResultRow).count();
        let thumbnail_nodes = snapshot.by_role(UiRole::ThumbnailCell).count();
        assert!(result_nodes > 0 && result_nodes <= virtualisation.materialised_rows);
        assert!(thumbnail_nodes > 0 && thumbnail_nodes <= virtualisation.materialised_thumbnails);
        assert!(snapshot.nodes.len() < 256);
        assert!(
            polyorama_ui_egui::audit_accesskit(&snapshot, &accesskit).is_empty(),
            "{:#?}",
            polyorama_ui_egui::audit_accesskit(&snapshot, &accesskit)
        );
    }

    #[test]
    fn active_pane_actions_have_one_semantic_and_physical_owner() {
        let context = egui::Context::default();
        let root = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(500.0, 160.0));
        let tokens = polyorama_ui_egui::DesignTokens::resolve(
            polyorama_ui_egui::ThemeVariant::Dark,
            polyorama_ui_egui::DensityVariant::Comfortable,
        );
        let mut output = FrameOutput {
            ui_geometry: UiGeometry::new(root, 1.0),
            ..Default::default()
        };
        let mut frame = context.run_ui(
            egui::RawInput {
                screen_rect: Some(root),
                ..Default::default()
            },
            |ui| {
                let parent = SemanticUiId::root();
                assert!(!present_action(
                    ui,
                    &mut output,
                    &tokens,
                    1.0,
                    &parent,
                    ActionTarget::pane(LabAction::DeleteAnnotation, PaneId(2)),
                    Availability::Enabled,
                    false,
                    false,
                    false,
                    "delete_annotation",
                ));
                assert!(!present_action(
                    ui,
                    &mut output,
                    &tokens,
                    1.0,
                    &parent,
                    ActionTarget::pane(LabAction::DeleteAnnotation, PaneId(1)),
                    Availability::Enabled,
                    false,
                    false,
                    true,
                    "delete_annotation",
                ));
            },
        );
        frame.textures_delta.clear();

        let owners: Vec<_> = output
            .ui_geometry
            .semantic_nodes
            .iter()
            .filter(|node| {
                node.actions
                    .iter()
                    .any(|action| action.as_str() == LabAction::DeleteAnnotation.stable_id())
            })
            .map(|node| node.pane)
            .collect();
        assert_eq!(owners, vec![Some(PaneId(1))]);
    }
}
