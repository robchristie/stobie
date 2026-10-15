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

#[test]
fn unsaved_follows_the_history_save_point_and_the_saved_layout() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    assert!(host.app().unsaved(), "nothing has been saved yet");
    perform(&mut host, Action::Save);
    assert!(!host.app().unsaved());
    let mut draft = host.app().desk.draft().unwrap().clone();
    draft.title = "Committed after saving".into();
    host.app_mut().desk.edit_draft(draft).unwrap();
    assert!(!host.app().unsaved(), "a draft is not committed state");
    perform(&mut host, Action::Apply);
    assert!(host.app().unsaved());
    perform(&mut host, Action::Undo);
    assert!(!host.app().unsaved(), "undo returned to the save point");
    perform(&mut host, Action::Redo);
    assert!(host.app().unsaved());
    perform(&mut host, Action::Save);
    assert!(!host.app().unsaved());
    resize(&mut host, 0.36, 0.5);
    assert!(
        host.app().unsaved(),
        "layout is saved state outside history"
    );
    resize(&mut host, 0.5, 0.36);
    assert!(!host.app().unsaved());
    perform(&mut host, Action::Restore);
    assert!(!host.app().unsaved());
    assert_eq!(host.app().desk.undo_len(), 0);
}

fn invoke_with(
    host: &mut RecordDeskApp,
    selector: InspectionSelector,
    value: serde_json::Value,
    id: &str,
) -> InvocationReceipt {
    let reply = host.inspection().unwrap().handle(request(
        host,
        &format!("{id}-discover"),
        InspectionOperation::Discover {
            selector: selector.clone(),
            limit: 2,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery");
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    let operation = InspectionOperation::Invoke {
        selector,
        expected: capabilities[0].target.clone(),
        arguments: serde_json::json!({ "value": value }),
    };
    let reply = host.dispatch(request(host, id, operation)).unwrap();
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt");
    };
    receipt
}

#[test]
fn category_and_review_choices_are_typed_actions() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    present(&mut host, &context, 1080.0);
    let reply = host.inspection().unwrap().handle(request(
        &host,
        "discover-category",
        InspectionOperation::Discover {
            selector: selector(Action::EditCategory),
            limit: 1,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery");
    };
    let ArgumentContract::Choice { options } = &capabilities[0].arguments else {
        panic!("a choice: {:?}", capabilities[0].arguments);
    };
    let ids: Vec<_> = options.iter().map(|option| option.id.as_str()).collect();
    assert_eq!(ids, ["research", "operations", "ideas"]);
    assert!(capabilities[0].semantic_invocable);

    let receipt = invoke_with(
        &mut host,
        selector(Action::EditCategory),
        serde_json::json!("ideas"),
        "category",
    );
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    assert_eq!(host.app().desk.draft().unwrap().category, Category::Ideas);
    assert!(host.app().desk.is_dirty());
    present(&mut host, &context, 1080.0);

    let receipt = invoke_with(
        &mut host,
        selector(Action::FilterReview),
        serde_json::json!("unreviewed"),
        "review",
    );
    assert_eq!(receipt.state, ReceiptState::Completed);
    assert_eq!(host.app().desk.filters().review, ReviewFilter::Unreviewed);
    present(&mut host, &context, 1080.0);
    let receipt = invoke_with(
        &mut host,
        selector(Action::FilterCategory),
        serde_json::json!("all"),
        "all-categories",
    );
    assert_eq!(receipt.state, ReceiptState::Completed);
    assert_eq!(host.app().desk.filters().category, None);

    present(&mut host, &context, 1080.0);
    let rejected = invoke_with(
        &mut host,
        selector(Action::FilterCategory),
        serde_json::json!("archive"),
        "unknown-category",
    );
    assert_eq!(rejected.state, ReceiptState::Rejected);
    let error = rejected.error.unwrap();
    assert_eq!(error.code, InspectionErrorCode::InvalidArguments);
    assert!(
        error.message.contains(r#""all", "research""#),
        "{}",
        error.message
    );
}

#[test]
fn set_text_edits_notes_as_typing_would() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    present(&mut host, &context, 1080.0);
    let record = host.app().desk.draft().unwrap().id.0;
    let notes = format!("record-desk.notes.{record}");
    let field = host
        .ui_snapshot()
        .node(&SemanticUiId::new(notes.clone()))
        .unwrap();
    assert_eq!(
        field.value.as_deref(),
        Some(host.app().desk.draft().unwrap().notes.as_str())
    );
    let receipt = invoke_with(
        &mut host,
        InspectionSelector {
            id: Some(SemanticUiId::new(notes.clone())),
            ..Default::default()
        },
        serde_json::json!("Checked\nagainst the survey"),
        "notes",
    );
    // The edit applies on the next pass, through egui's text editing, and
    // completes once the draft intent it causes has applied.
    assert_eq!(receipt.state, ReceiptState::Queued, "{receipt:?}");
    present(&mut host, &context, 1080.0);
    let reply = host.inspection().unwrap().handle(request(
        &host,
        "notes-receipt",
        InspectionOperation::Receipt {
            request_id: "notes".into(),
        },
    ));
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt");
    };
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    let effect = receipt
        .after_observation
        .expect("a completed receipt names its effect");
    // Every observation from `after_observation` presents the effect: the
    // draft is dirty and Apply is available.
    let observe = |host: &RecordDeskApp| {
        let reply = host.inspection().unwrap().handle(request(
            host,
            "observe",
            InspectionOperation::Observe,
        ));
        let Some(InspectionResult::Observe { observation }) = reply.result else {
            panic!("observation");
        };
        observation
    };
    assert!(observe(&host).id < effect);
    present(&mut host, &context, 1080.0);
    let observation = observe(&host);
    assert_eq!(observation.id, effect);
    assert_eq!(
        observation.facts["draft_dirty"],
        stobie_ui_egui::ObservationValue::Boolean(true)
    );
    let expected = target(&host, Action::Apply);
    let apply = invoke(&mut host, Action::Apply, expected, "apply-after-set-text");
    assert_eq!(apply.state, ReceiptState::Completed, "{apply:?}");
    present(&mut host, &context, 1080.0);
    let draft = host.app().desk.draft().unwrap();
    assert_eq!(draft.notes, "Checked\nagainst the survey");
    assert_eq!(
        host.ui_snapshot()
            .node(&SemanticUiId::new(notes))
            .unwrap()
            .value
            .as_deref(),
        Some("Checked\nagainst the survey")
    );
    assert_eq!(host.app().desk.undo_len(), 1);
}

/// Present one pass and return its AccessKit tree.
fn present_tree(
    host: &mut RecordDeskApp,
    context: &egui::Context,
    width: f32,
) -> egui::accesskit::TreeUpdate {
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
    output
        .platform_output
        .accesskit_update
        .expect("automation enables AccessKit")
}

/// The ARIA layer projected from the AccessKit tree and the inspection
/// snapshot agree in representative states.
#[test]
fn semantic_views_agree_across_representative_states() {
    let directory = tempfile::tempdir().unwrap();
    let context = egui::Context::default();
    let mut host = hosted(&context, &directory);
    let check = |host: &mut RecordDeskApp, state: &str, width: f32| {
        // Settle intents and focus before comparing the views of one pass.
        for _ in 0..2 {
            present(host, &context, width);
        }
        let update = present_tree(host, &context, width);
        let snapshot = host.ui_snapshot();
        let compared = snapshot
            .nodes
            .iter()
            .filter(|node| expected_aria_role(node.role).is_some())
            .count();
        assert!(compared > 10, "{state}: {compared} nodes compared");
        let disagreements = semantic_agreement(snapshot, &update);
        assert!(disagreements.is_empty(), "{state}: {disagreements:#?}");
    };
    check(&mut host, "initial", 1080.0);
    // Focused search field.
    let search = target(&host, Action::Search);
    invoke(&mut host, Action::Search, search, "search");
    check(&mut host, "search focused", 1080.0);
    assert!(
        host.ui_snapshot()
            .node(&SemanticUiId::new("record-desk.search"))
            .unwrap()
            .focused
    );
    // Filters and a dirty draft with a changed category.
    let filters = Filters {
        review: ReviewFilter::Unreviewed,
        ..host.app().desk.filters().clone()
    };
    host.submit(Intent::Filters(filters));
    let mut draft = host.app().desk.draft().unwrap().clone();
    draft.category = Category::Ideas;
    draft.notes = "Checked\nagainst the survey".into();
    host.submit(Intent::Draft(draft));
    check(&mut host, "filtered with a draft", 1080.0);
    // Another record in the narrow, stacked layout.
    let other = host
        .app()
        .desk
        .visible()
        .iter()
        .map(|record| record.id)
        .find(|id| Some(*id) != host.app().desk.draft().map(|draft| draft.id))
        .unwrap();
    perform(&mut host, Action::Cancel);
    host.submit(Intent::Select(other));
    check(&mut host, "narrow with another record", 560.0);
}
