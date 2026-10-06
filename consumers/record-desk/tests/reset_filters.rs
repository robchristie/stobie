#![cfg(not(target_arch = "wasm32"))]

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use polyorama_ui_egui::{ActionKey, SemanticActionId, UiNode};
use record_desk::{
    actions::Action,
    app::{RecordDeskApp, Snapshot},
    model::{
        Category, Desk, Filters, Record, RecordId, ReviewFilter, SavedState, default_workspace,
    },
    store::Store,
};

#[derive(Default)]
struct TestState {
    app: Option<RecordDeskApp>,
    unconsumed_reset_presses: usize,
}

fn harness() -> (Harness<'static, TestState>, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.json");
    let records = vec![
        Record {
            id: RecordId(71),
            title: "First record".into(),
            category: Category::Research,
            notes: "First notes".into(),
            reviewed: false,
        },
        Record {
            id: RecordId(83),
            title: "Second record".into(),
            category: Category::Operations,
            notes: "Second notes".into(),
            reviewed: true,
        },
    ];
    Store::at_path(&path)
        .save(&SavedState::from_desk(
            &Desk::new(records).unwrap(),
            &default_workspace(),
        ))
        .unwrap();
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1100.0, 1000.0))
        .build_ui_state(
            move |ui, state: &mut TestState| {
                if state.app.is_none() {
                    state.app = Some(RecordDeskApp::with_store(ui.ctx(), Store::at_path(&path)));
                    // Font installation needs a new pass before measured presentation.
                    ui.ctx().request_repaint();
                    return;
                }
                state.app.as_mut().unwrap().present(ui);
                let shortcut = Action::ResetFilters
                    .specification()
                    .shortcut
                    .unwrap()
                    .egui();
                if ui.input_mut(|input| input.consume_shortcut(&shortcut)) {
                    state.unconsumed_reset_presses += 1;
                }
            },
            TestState::default(),
        );
    harness.run();
    (harness, directory)
}

fn snapshot(harness: &Harness<'_, TestState>) -> Snapshot {
    harness.state().app.as_ref().unwrap().snapshot()
}

fn reset_node(snapshot: &Snapshot) -> &UiNode {
    let identity = SemanticActionId::from_action(Action::ResetFilters);
    snapshot
        .ui
        .nodes
        .iter()
        .find(|node| node.actions.contains(&identity))
        .expect("Reset filters has a current semantic action node")
}

fn replace_text(harness: &mut Harness<'_, TestState>, label: &str, text: &str) {
    harness.get_by_label(label).click();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    harness.get_by_label(label).type_text(text);
    harness.run();
}

fn choose(harness: &mut Harness<'_, TestState>, label: &str, value: &str) {
    harness.get_by_label(label).click();
    harness.run();
    harness.get_by_label(value).click();
    harness.run();
}

fn hidden_draft(harness: &mut Harness<'_, TestState>) -> Snapshot {
    harness
        .get_by_label("Second record · Operations · Reviewed")
        .click();
    harness.run();
    replace_text(harness, "Record title", "Committed title");
    harness.get_by_label("Apply").click();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    replace_text(harness, "Record title", "Draft retained after reset");
    choose(harness, "Category filter", "Research");
    choose(harness, "Review filter", "Unreviewed");
    replace_text(harness, "Search records", "no matching title");
    let before = snapshot(harness);
    assert_eq!(before.selected, Some(RecordId(83)));
    assert!(before.draft_dirty);
    assert!(before.visible_ids.is_empty());
    assert_eq!(before.redo_entries, 1);
    assert_eq!(
        before.filters,
        Filters {
            query: "no matching title".into(),
            category: Some(Category::Research),
            review: ReviewFilter::Unreviewed,
        }
    );
    assert!(reset_node(&before).enabled);
    before
}

fn assert_reset(before: &Snapshot, after: &Snapshot) {
    assert_eq!(after.filters, Filters::default());
    assert_eq!(after.visible_ids, vec![RecordId(71), RecordId(83)]);
    assert_eq!(after.selected, before.selected);
    assert_eq!(after.draft, before.draft);
    assert_eq!(after.records, before.records);
    assert_eq!(after.undo_entries, before.undo_entries);
    assert_eq!(after.redo_entries, before.redo_entries);
    assert_eq!(after.unsaved, before.unsaved);
    assert!(after.draft_dirty);
    assert!(!after.error);
    let node = reset_node(after);
    assert!(!node.enabled);
    assert_eq!(
        node.disabled_reason.as_deref(),
        Some("There are no active filters")
    );
}

#[test]
fn button_resets_all_filters_and_preserves_hidden_selection_draft_and_history() {
    let (mut harness, _directory) = harness();
    let before = hidden_draft(&mut harness);
    harness.get_by_label("Reset filters").click();
    harness.run();
    assert_reset(&before, &snapshot(&harness));
}

#[test]
fn registered_shortcut_resets_filters_while_search_has_keyboard_focus() {
    let (mut harness, _directory) = harness();
    let before = hidden_draft(&mut harness);
    assert!(harness.get_by_label("Search records").is_focused());
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
    harness.run();
    assert_reset(&before, &snapshot(&harness));
    assert_eq!(harness.state().unconsumed_reset_presses, 0);
}

#[test]
fn disabled_reset_does_not_consume_the_shortcut_or_activate_search() {
    let (mut harness, _directory) = harness();
    let before = snapshot(&harness);
    assert!(!reset_node(&before).enabled);
    harness.get_by_label("Record title").click();
    harness.run();
    assert!(harness.get_by_label("Record title").is_focused());
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::F);
    harness.run();
    let after = snapshot(&harness);
    assert_eq!(after.filters, before.filters);
    assert_eq!(after.selected, before.selected);
    assert_eq!(after.draft, before.draft);
    assert_eq!(after.message, before.message);
    assert!(harness.get_by_label("Record title").is_focused());
    assert_eq!(harness.state().unconsumed_reset_presses, 1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert!(harness.get_by_label("Search records").is_focused());
}

#[test]
fn each_filter_independently_enables_reset() {
    let (mut harness, _directory) = harness();
    for (label, value) in [
        ("Search records", "record"),
        ("Category filter", "Research"),
        ("Review filter", "Unreviewed"),
    ] {
        if label == "Search records" {
            replace_text(&mut harness, label, value);
        } else {
            choose(&mut harness, label, value);
        }
        assert!(reset_node(&snapshot(&harness)).enabled, "{label}");
        harness.get_by_label("Reset filters").click();
        harness.run();
        assert_eq!(snapshot(&harness).filters, Filters::default());
        assert!(!reset_node(&snapshot(&harness)).enabled, "{label}");
    }
}
