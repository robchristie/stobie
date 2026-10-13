use super::*;

fn present(host: &mut RecordDeskApp, context: &egui::Context, width: f32) {
    let mut output = context.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 760.0),
            )),
            ..Default::default()
        },
        |ui| host.present(ui),
    );
    output.textures_delta.clear();
}

fn hosted(context: &egui::Context, directory: &tempfile::TempDir) -> RecordDeskApp {
    let mut host = RecordDesk::hosted(
        context,
        Store::at_path(directory.path().join("records.json")),
    );
    host.enable_inspection(Inspection::new("record-desk-test"));
    host
}

fn request(host: &RecordDeskApp, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn selector(action: Action) -> InspectionSelector {
    InspectionSelector {
        capability: Some(action.stable_id().into()),
        ..Default::default()
    }
}

fn target(host: &RecordDeskApp, action: Action) -> ResolvedTarget {
    let operation = InspectionOperation::Discover {
        selector: selector(action),
        limit: 1,
        cursor: None,
    };
    let reply = host
        .inspection()
        .unwrap()
        .handle(request(host, "discover", operation));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery");
    };
    capabilities[0].target.clone()
}

fn invoke(
    host: &mut RecordDeskApp,
    action: Action,
    expected: ResolvedTarget,
    id: &str,
) -> InvocationReceipt {
    let operation = InspectionOperation::Invoke {
        selector: selector(action),
        expected,
        arguments: serde_json::Value::Null,
    };
    let reply = host.dispatch(request(host, id, operation)).unwrap();
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt");
    };
    receipt
}

/// Act directly on the model, outside the semantic route under test.
fn perform(host: &mut RecordDeskApp, action: Action) {
    let target = ActionTarget::application(action);
    host.app_mut()
        .perform(target, &mut View::default())
        .unwrap();
}

fn resize(host: &mut RecordDeskApp, before: f32, after: f32) {
    stobie_core::WorkspaceResize {
        node: stobie_core::DockNodeId(1),
        before,
        after,
    }
    .apply(&mut host.app_mut().workspace)
    .unwrap();
}

#[test]
fn semantic_guards_ignore_focus_and_layout_but_track_draft_and_history() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    let mut draft = host.app().desk.draft().unwrap().clone();
    draft.title = "One valid edit".into();
    host.app_mut().desk.edit_draft(draft.clone()).unwrap();
    present(&mut host, &context, 1080.0);
    let apply = target(&host, Action::Apply);
    perform(&mut host, Action::Search);
    let before = match &host.app().workspace.root {
        DockNode::Split { fraction, .. } => *fraction,
        _ => unreachable!(),
    };
    resize(&mut host, before, 0.45);
    present(&mut host, &context, 1080.0);
    assert_eq!(
        invoke(&mut host, Action::Apply, apply, "apply").state,
        ReceiptState::Completed
    );
    present(&mut host, &context, 1080.0);
    let undo = target(&host, Action::Undo);
    perform(&mut host, Action::Search);
    resize(&mut host, 0.45, 0.5);
    present(&mut host, &context, 1080.0);
    assert_eq!(
        invoke(&mut host, Action::Undo, undo.clone(), "undo").state,
        ReceiptState::Completed
    );
    present(&mut host, &context, 1080.0);
    assert_eq!(
        invoke(&mut host, Action::Undo, undo, "old-undo")
            .error
            .unwrap()
            .code,
        InspectionErrorCode::StaleTarget
    );
    host.app_mut().desk.edit_draft(draft.clone()).unwrap();
    present(&mut host, &context, 1080.0);
    let old_apply = target(&host, Action::Apply);
    draft.title = "A different edit".into();
    host.app_mut().desk.edit_draft(draft).unwrap();
    present(&mut host, &context, 1080.0);
    assert_eq!(
        invoke(&mut host, Action::Apply, old_apply, "old-apply")
            .error
            .unwrap()
            .code,
        InspectionErrorCode::StaleTarget
    );
    assert_eq!(host.app().desk.undo_len(), 0);
}

#[test]
fn draft_field_capabilities_are_hidden_without_a_selected_record() {
    let directory = tempfile::tempdir().unwrap();
    let mut desk = RecordDesk::with_store(Store::at_path(directory.path().join("r.json")));
    desk.desk = Desk::new(Vec::new()).unwrap();
    let availability = |action| desk.availability(ActionTarget::application(action));
    assert_eq!(availability(Action::ToggleReviewed), Availability::Hidden);
    assert_eq!(availability(Action::EditCategory), Availability::Hidden);
    assert!(!availability(Action::Apply).enabled());
}

#[test]
fn draft_and_filter_edits_apply_before_a_same_pass_selection() {
    let directory = tempfile::tempdir().unwrap();
    let desk = RecordDesk::with_store(Store::at_path(directory.path().join("r.json")));
    let draft = desk.desk.draft().unwrap().clone();
    let mut intents = vec![
        Intent::Select(RecordId(1)),
        Intent::Draft(draft),
        Intent::Filters(Filters::default()),
    ];
    desk.order_intents(&mut intents);
    assert!(matches!(
        intents.as_slice(),
        [Intent::Draft(_), Intent::Filters(_), Intent::Select(_)]
    ));
}

#[test]
fn reviewed_toggle_is_invocable_and_tracks_the_draft() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    present(&mut host, &context, 1080.0);
    let reviewed = host.app().desk.draft().unwrap().reviewed;
    let toggle = target(&host, Action::ToggleReviewed);
    assert_eq!(
        invoke(&mut host, Action::ToggleReviewed, toggle.clone(), "toggle").state,
        ReceiptState::Completed
    );
    assert_eq!(host.app().desk.draft().unwrap().reviewed, !reviewed);
    assert!(host.app().desk.is_dirty());
    present(&mut host, &context, 1080.0);
    // The draft changed, so a target discovered before the toggle is stale.
    assert_eq!(
        invoke(&mut host, Action::ToggleReviewed, toggle, "again")
            .error
            .unwrap()
            .code,
        InspectionErrorCode::StaleTarget
    );
    assert_eq!(host.app().desk.draft().unwrap().reviewed, !reviewed);
}

#[test]
fn queued_arrange_rechecks_the_current_narrow_window_policy() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    present(&mut host, &context, 1080.0);
    let expected = target(&host, Action::Arrange);
    let message = host.app().message.clone();
    let operation = InspectionOperation::Invoke {
        selector: selector(Action::Arrange),
        expected,
        arguments: serde_json::Value::Null,
    };
    host.inspection()
        .unwrap()
        .handle(request(&host, "arrange", operation));
    present(&mut host, &context, 390.0);
    let operation = InspectionOperation::Receipt {
        request_id: "arrange".into(),
    };
    let reply = host
        .inspection()
        .unwrap()
        .handle(request(&host, "receipt", operation));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt");
    };
    assert_eq!(receipt.state, ReceiptState::Rejected);
    assert!(matches!(
        receipt.error.unwrap().code,
        InspectionErrorCode::StaleTarget | InspectionErrorCode::Unavailable
    ));
    assert_eq!(host.app().message, message);
    assert_eq!(host.app().desk.undo_len(), 0);
    assert!(matches!(
        host.app().workspace.root,
        DockNode::Split {
            axis: SplitAxis::Vertical,
            ..
        }
    ));
}
