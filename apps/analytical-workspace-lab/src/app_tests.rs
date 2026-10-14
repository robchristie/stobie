//! The hosted Lab: actions, gestures and layout through the framework host.

use egui_kittest::Harness;
use stobie_ui_egui::{
    Inspection, InspectionOperation, InspectionRequest, InspectionResult, InspectionSelector,
    ReceiptState, ResolvedTarget,
};

use super::*;

type Probe = Option<AnalyticalWorkspaceApp>;

/// The Lab hosted in a 1440×900 window, constructed on the first pass as eframe
/// would construct it, without stored state or a GPU.
fn harness() -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .build_ui_state(
            |ui, probe: &mut Probe| match probe {
                Some(host) => host.present(ui),
                None => {
                    let creation = eframe::CreationContext::_new_kittest(ui.ctx().clone());
                    let mut host = AppHost::new(ui.ctx(), Lab::new(&creation));
                    host.enable_inspection(Inspection::new("lab-test"));
                    *probe = Some(host);
                    ui.ctx().request_repaint();
                }
            },
            None,
        );
    harness.run_steps(6);
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AnalyticalWorkspaceApp {
    harness.state().as_ref().unwrap()
}

fn lab<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut Lab {
    harness.state_mut().as_mut().unwrap().app_mut()
}

fn viewport(harness: &Harness<'_, Probe>, pane: PaneId) -> egui::Rect {
    host(harness)
        .ui_snapshot()
        .node(&SemanticUiId::viewport(pane))
        .expect("viewport is presented")
        .rect
        .into()
}

fn click(harness: &mut Harness<'_, Probe>, position: egui::Pos2) {
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    harness.step();
    // Let egui's double-click window lapse before the next click.
    harness.run_steps(40);
}

fn request(
    harness: &Harness<'_, Probe>,
    id: &str,
    operation: InspectionOperation,
) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host(harness).inspection().unwrap().instance()),
        operation,
    }
}

fn discover(
    harness: &Harness<'_, Probe>,
    action: LabAction,
    pane: Option<PaneId>,
) -> (bool, ResolvedTarget) {
    let selector = InspectionSelector {
        capability: Some(action.stable_id().into()),
        pane,
        ..Default::default()
    };
    let operation = InspectionOperation::Discover {
        selector,
        limit: 8,
        cursor: None,
    };
    let reply = host(harness)
        .inspection()
        .unwrap()
        .handle(request(harness, "discover", operation));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    let capability = &capabilities[0];
    (capability.availability.enabled(), capability.target.clone())
}

#[test]
fn polygon_clicks_edit_the_session_preview_and_one_commit_records_one_command() {
    let mut harness = harness();
    let app = lab(&mut harness);
    app.session
        .active_tools
        .insert(PaneId(1), ActiveTool::Polygon);
    app.workspace.active_pane = PaneId(1);
    harness.run_steps(2);
    let rect = viewport(&harness, PaneId(1));
    let undo_before = host(&harness).app().history.undo_len();
    for offset in [0.0, 40.0, 80.0] {
        click(
            &mut harness,
            rect.center() + egui::vec2(offset, offset * 0.5),
        );
    }
    // Each click reached the session's preview through the gesture intent,
    // without a command.
    let app = host(&harness).app();
    assert_eq!(
        crate::actions::polygon_vertex_count(app.session.gesture.as_ref()),
        3
    );
    assert_eq!(app.history.undo_len(), undo_before);
    assert_eq!(app.document.annotations.len(), 0);

    // The toolbar's Commit takes the click route: one command, one undo
    // record, and no remaining preview.
    let commit: egui::Rect = host(&harness)
        .ui_snapshot()
        .node(&SemanticUiId::new("action.commit_polygon.pane.1"))
        .expect("commit is presented")
        .rect
        .into();
    click(&mut harness, commit.center());
    let app = host(&harness).app();
    assert_eq!(app.document.annotations.len(), 1);
    assert_eq!(app.history.undo_len(), undo_before + 1);
    assert!(app.session.gesture.is_none());
}

#[test]
fn polygon_discovery_matches_rendered_availability_after_the_third_vertex() {
    let mut harness = harness();
    for count in [2, 3] {
        lab(&mut harness).session.gesture = Some(GesturePreview::Polygon {
            layer: LayerId(1),
            vertices: (0..count).map(|i| WorldPoint::new(i as f64, 0.0)).collect(),
        });
        harness.run_steps(2);
        let rendered = host(&harness)
            .ui_snapshot()
            .node(&SemanticUiId::new("action.commit_polygon.pane.1"))
            .expect("commit is presented while a polygon is in progress")
            .enabled;
        let (discovered, _) = discover(&harness, LabAction::CommitPolygon, Some(PaneId(1)));
        assert_eq!(discovered, rendered);
        assert_eq!(rendered, count == 3);
    }
}

#[test]
fn pane_shortcuts_reach_only_panes_that_offer_the_action() {
    let mut harness = harness();
    let app = lab(&mut harness);
    app.workspace.active_pane = PaneId(1);
    app.session.selected_result = Some(ResultId(42));
    harness.run_steps(2);
    let camera = |harness: &Harness<'_, Probe>| {
        host(harness)
            .app()
            .session
            .cameras
            .iter()
            .find(|state| state.pane == PaneId(1))
            .unwrap()
            .camera
    };
    let before = camera(&harness);
    // Recenter is offered by Results and Inspector only.
    harness.key_press(egui::Key::R);
    harness.run_steps(2);
    assert_eq!(camera(&harness), before);
    // Fit is offered by the active image pane.
    harness.key_press(egui::Key::F);
    harness.run_steps(2);
    assert_ne!(camera(&harness), before);
    assert_eq!(host(&harness).app().history.undo_len(), 1);
}

#[test]
fn plain_key_shortcuts_reach_a_clicked_viewport() {
    let mut harness = harness();
    let rect = viewport(&harness, PaneId(1));
    click(&mut harness, rect.center());
    assert!(harness.ctx.egui_wants_keyboard_input());
    assert_eq!(host(&harness).app().workspace.active_pane, PaneId(1));
    let tool = |harness: &Harness<'_, Probe>| {
        host(harness)
            .app()
            .session
            .active_tools
            .get(&PaneId(1))
            .copied()
    };
    // Keyboard focus on the image is not text entry, so its pane's
    // unmodified shortcuts still run.
    harness.key_press(egui::Key::Num2);
    harness.run_steps(2);
    assert_eq!(tool(&harness), Some(ActiveTool::Polygon));
    harness.key_press(egui::Key::Num1);
    harness.run_steps(2);
    assert_eq!(tool(&harness), Some(ActiveTool::Navigate));
    assert!(harness.ctx.egui_wants_keyboard_input());
}

#[test]
fn semantic_invocation_runs_the_same_action_and_rechecks_meaning() {
    let mut harness = harness();
    let (enabled, expected) = discover(&harness, LabAction::ToggleDiagnostics, None);
    assert!(enabled);
    let operation = InspectionOperation::Invoke {
        selector: InspectionSelector {
            capability: Some(LabAction::ToggleDiagnostics.stable_id().into()),
            ..Default::default()
        },
        expected: expected.clone(),
        arguments: serde_json::Value::Null,
    };
    let reply =
        host(&harness)
            .inspection()
            .unwrap()
            .handle(request(&harness, "toggle", operation.clone()));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt: {:?}", reply.error);
    };
    assert_eq!(receipt.state, ReceiptState::Queued);
    harness.run_steps(2);
    let app = host(&harness).app();
    assert!(
        app.workspace
            .closed_optional_panes
            .contains(&DIAGNOSTICS_PANE)
    );
    assert_eq!(app.status, "Diagnostics closed");
    // The target's meaning changed, so the earlier discovery is stale.
    let reply =
        host(&harness)
            .inspection()
            .unwrap()
            .handle(request(&harness, "toggle-again", operation));
    harness.run_steps(2);
    let lookup = InspectionOperation::Receipt {
        request_id: "toggle-again".into(),
    };
    let reply = match reply.result {
        Some(InspectionResult::Receipt { .. }) => host(&harness)
            .inspection()
            .unwrap()
            .handle(request(&harness, "lookup", lookup)),
        _ => reply,
    };
    let error = match reply.result {
        Some(InspectionResult::Receipt { receipt }) => receipt.error,
        _ => reply.error,
    };
    assert_eq!(
        error.map(|error| error.code),
        Some(stobie_ui_egui::InspectionErrorCode::StaleTarget)
    );
    assert!(
        host(&harness)
            .app()
            .workspace
            .closed_optional_panes
            .contains(&DIAGNOSTICS_PANE)
    );
}

#[test]
fn save_layout_asks_the_host_to_save_once() {
    let mut harness = harness();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(2);
    let app = lab(&mut harness);
    assert_eq!(app.status, "Workspace saved");
    assert!(app.take_save_request());
    assert!(!app.take_save_request());
}

#[test]
fn layout_resizes_are_undoable_and_activation_is_not() {
    let mut harness = harness();
    let mut view = LabView::default();
    let app = lab(&mut harness);
    let node = DockNodeId(1);
    let before = app.workspace.root.split_fraction(node).expect("root split");
    let resize = WorkspaceChange::Resize(stobie_core::WorkspaceResize {
        node,
        before,
        after: 0.4,
    });
    assert_eq!(app.apply(Intent::Layout(resize), &mut view), Ok(true));
    assert_eq!(app.workspace.root.split_fraction(node), Some(0.4));
    assert_eq!(app.history.undo_len(), 1);
    assert_eq!(
        app.apply(
            Intent::Layout(WorkspaceChange::Activate(PaneId(5))),
            &mut view
        ),
        Ok(true)
    );
    assert_eq!(app.workspace.active_pane, PaneId(5));
    assert_eq!(app.history.undo_len(), 1);
    assert!(
        app.history
            .undo(&mut app.document, &mut app.session, &mut app.workspace)
    );
    assert_eq!(app.workspace.root.split_fraction(node), Some(before));
}

#[test]
fn diagnostics_report_interaction_while_a_camera_drag_previews() {
    let mut harness = harness();
    lab(&mut harness).workspace.active_pane = PaneId(1);
    harness.run_steps(2);
    let centre = viewport(&harness, PaneId(1)).center();
    harness.event(egui::Event::PointerMoved(centre));
    harness.event(egui::Event::PointerButton {
        pos: centre,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.step();
    for step in 1..=6 {
        harness.event(egui::Event::PointerMoved(
            centre + egui::vec2(8.0 * step as f32, 0.0),
        ));
        harness.step();
    }
    assert!(host(&harness).app().diagnostics.frame.interaction_active);
    harness.event(egui::Event::PointerButton {
        pos: centre + egui::vec2(48.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run_steps(4);
    assert!(!host(&harness).app().diagnostics.frame.interaction_active);
}

#[test]
fn a_preference_change_counts_one_repaint() {
    let mut harness = harness();
    let preferences = |harness: &Harness<'_, Probe>| {
        host(harness)
            .app()
            .diagnostics
            .frame
            .recent_reasons
            .iter()
            .filter(|reason| **reason == RepaintReason::Preferences)
            .count()
    };
    let before = preferences(&harness);
    let mut changed = host(&harness).app().preferences;
    changed.font_scale = 1.25;
    harness
        .state_mut()
        .as_mut()
        .unwrap()
        .submit(Intent::Preferences(changed));
    // The pass that reapplies appearance records it once; later passes do not.
    harness.step();
    assert_eq!(preferences(&harness), before + 1);
    harness.step();
    assert_eq!(preferences(&harness), before + 1);
}
