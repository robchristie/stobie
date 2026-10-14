#![cfg(test)]
//! Framework set text and typed action arguments through the host.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, ArgumentContract, CapabilityDescription, ChoiceArgument, InspectionErrorCode,
    InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    NativeTextControlKind, PresentationScope, ReceiptState, SET_TEXT_CAPABILITY,
};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Colour {
    Red,
    Blue,
}

impl ChoiceArgument for Colour {
    const CHOICES: &'static [(Self, &'static str, &'static str)] =
        &[(Self::Red, "red", "Red"), (Self::Blue, "blue", "Blue")];
}

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "desk")]
enum Act {
    #[action(label = "Apply", description = "Commit the draft")]
    Apply,
    #[action(
        label = "Colour",
        description = "Colour the draft",
        argument = "choice",
        choices = Colour
    )]
    Colour,
    #[action(label = "Pin", description = "Pin the draft", argument = "boolean")]
    Pin,
}

enum Intent {
    Title(String),
    Notes(String),
}

/// A draft edited through two native text fields, committed by Apply.
#[derive(Default)]
struct Desk {
    title: String,
    notes: String,
    /// Draft intents applied, one per changed pass.
    edits: u64,
    /// Committed (title, notes) pairs, the application's history.
    history: Vec<(String, String)>,
    colour: Option<Colour>,
    pinned: Option<bool>,
    /// The title field is presented disabled.
    locked: bool,
    /// The title is a password field.
    secret: bool,
    /// The notes widget presents this semantic ID; pinning changes it, as an
    /// application that reuses one editor for each selection would.
    notes_id: Option<&'static str>,
    errors: Vec<String>,
    /// A colour for the next pass to trigger as a control would.
    physical_colour: Option<ActionArgument>,
}

#[derive(Default)]
struct View {
    title: Option<Rect>,
}

impl Application for Desk {
    type Action = Act;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Desk")
    }

    fn availability(&self, _target: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, Act, Intent>) {
        if let Some(argument) = self.physical_colour.clone() {
            frame.trigger_with(Act::Colour, argument);
        }
        let mut p = frame.presentation(ui, PresentationScope::new("desk"), SemanticUiId::root());
        frame.action(ui, &mut p, Act::Apply, ActionEmphasis::Normal);
        // The title sets its AccessKit author ID, as Stobie recipes do.
        let mut title = self.title.clone();
        let locked = self.locked;
        let (response, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
            let response = ui.add_enabled(
                !locked,
                egui::TextEdit::singleline(&mut title)
                    .password(self.secret)
                    .id(egui::Id::new("desk.title")),
            );
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.set_label("Title");
                node.set_author_id("desk.title".to_owned());
            });
            (response, ())
        });
        view.title = Some(response.rect);
        let mut node = UiNode::container(
            SemanticUiId::new("desk.title"),
            Some(SemanticUiId::root()),
            UiRole::TextInput,
            response.rect.into(),
        );
        node.name = "Title".into();
        node.focused = response.has_focus();
        if locked {
            node.enabled = false;
            node.disabled_reason = Some("The record is locked".into());
        }
        p.observe_node(ui, node);
        if title != self.title {
            frame.emit(Intent::Title(title));
        }
        // The notes field sets no author ID; the host pairs it by geometry.
        let mut notes = self.notes.clone();
        let (response, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
            let response = ui.add(egui::TextEdit::multiline(&mut notes).id(egui::Id::new("notes")));
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_label("Notes"));
            (response, ())
        });
        let mut node = UiNode::container(
            SemanticUiId::new(self.notes_id.unwrap_or("desk.notes")),
            Some(SemanticUiId::root()),
            UiRole::MultilineTextInput,
            response.rect.into(),
        );
        node.name = "Notes".into();
        node.focused = response.has_focus();
        p.observe_node(ui, node);
        if notes != self.notes {
            frame.emit(Intent::Notes(notes));
        }
    }

    fn apply(&mut self, intent: Intent, _state: &mut View) -> Result<bool, String> {
        self.edits += 1;
        match intent {
            Intent::Title(title) => self.title = title,
            Intent::Notes(notes) => self.notes = notes,
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<Act>, _state: &mut View) -> Result<bool, String> {
        match target.action {
            Act::Apply => {
                self.history.push((self.title.clone(), self.notes.clone()));
                Ok(true)
            }
            _ => Err("needs an argument".into()),
        }
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<Act>,
        argument: ActionArgument,
        state: &mut View,
    ) -> Result<bool, String> {
        match target.action {
            Act::Colour => {
                self.physical_colour = None;
                self.colour = Some(argument.as_choice()?);
                Ok(true)
            }
            Act::Pin => {
                self.pinned = argument.as_boolean();
                self.notes_id = Some("desk.notes.pinned");
                Ok(true)
            }
            Act::Apply => self.perform(target, state),
        }
    }

    fn report_error(&mut self, message: String) {
        self.physical_colour = None;
        self.errors.push(message);
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Desk>>,
}

fn harness(desk: Desk) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(600.0, 400.0))
        .build_ui_state(
            |ui, probe: &mut Probe| {
                if let Some(host) = &mut probe.host {
                    host.present(ui);
                }
            },
            Probe::default(),
        );
    crate::install_typography_fonts(&harness.ctx);
    let context = harness.ctx.clone();
    let mut host = AppHost::new(&context, desk);
    host.enable_inspection(Inspection::new("desk-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Desk> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Desk> {
    harness.state_mut().host.as_mut().unwrap()
}

fn request(host: &AppHost<Desk>, id: &str, operation: InspectionOperation) -> InspectionRequest {
    InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(host.inspection().unwrap().instance()),
        operation,
    }
}

fn by_id(id: &str) -> InspectionSelector {
    InspectionSelector {
        id: Some(SemanticUiId::new(id)),
        ..Default::default()
    }
}

fn discover(host: &AppHost<Desk>, selector: InspectionSelector) -> CapabilityDescription {
    let reply = host.inspection().unwrap().handle(request(
        host,
        "discover",
        InspectionOperation::Discover {
            selector,
            limit: 8,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1, "{capabilities:?}");
    capabilities.into_iter().next().unwrap()
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

/// Invoke one capability with `arguments` through the native queue, run the
/// passes that drain and apply it, and return its final receipt.
fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    selector: InspectionSelector,
    arguments: serde_json::Value,
) -> InvocationReceipt {
    let current = host(harness);
    let expected = discover(current, selector.clone()).target;
    let admitted = receipt(current.inspection().unwrap().handle(request(
        current,
        id,
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments,
        },
    )));
    if admitted.state != ReceiptState::Queued {
        return admitted;
    }
    harness.run();
    let host = host(harness);
    receipt(host.inspection().unwrap().handle(request(
        host,
        &format!("{id}-receipt"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    )))
}

fn set_text(harness: &mut Harness<'_, Probe>, field: &str, text: &str) -> InvocationReceipt {
    invoke(
        harness,
        &format!("set-{field}-{text}"),
        by_id(field),
        serde_json::json!({ "value": text }),
    )
}

fn node<'a>(harness: &'a Harness<'_, Probe>, id: &str) -> &'a UiNode {
    host(harness)
        .ui_snapshot()
        .node(&SemanticUiId::new(id))
        .unwrap()
}

fn type_into_title(harness: &mut Harness<'_, Probe>, text: &str) {
    let centre = host(harness).ui_state().title.unwrap().center();
    harness.event(egui::Event::PointerMoved(centre));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: centre,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    for character in text.chars() {
        harness.event(egui::Event::Text(character.to_string()));
        harness.run();
    }
}

fn desk() -> Desk {
    Desk {
        title: "Old title".into(),
        ..Desk::default()
    }
}

fn apply(harness: &mut Harness<'_, Probe>) {
    let receipt = invoke(
        harness,
        "apply",
        InspectionSelector {
            capability: Some("desk.apply".into()),
            ..Default::default()
        },
        serde_json::Value::Null,
    );
    assert_eq!(receipt.state, ReceiptState::Completed);
    harness.run();
}

#[test]
fn text_fields_publish_values_and_a_set_text_capability() {
    let harness = harness(desk());
    assert_eq!(
        node(&harness, "desk.title").value.as_deref(),
        Some("Old title")
    );
    assert_eq!(node(&harness, "desk.notes").value.as_deref(), Some(""));
    for id in ["desk.title", "desk.notes"] {
        let capability = discover(host(&harness), by_id(id));
        assert_eq!(capability.capability, SET_TEXT_CAPABILITY);
        assert_eq!(
            capability.arguments,
            ArgumentContract::Text { max_length: None }
        );
        assert!(capability.availability.enabled());
    }
    // Other nodes publish no value.
    assert!(host(&harness).ui_snapshot().nodes.iter().all(|node| {
        node.value.is_some() == matches!(node.role, UiRole::TextInput | UiRole::MultilineTextInput)
    }));
}

#[test]
fn set_text_gives_the_same_state_and_history_as_typing() {
    let mut typed = harness(desk());
    type_into_title(&mut typed, "Fresh");
    apply(&mut typed);

    let mut set = harness(desk());
    let receipt = set_text(&mut set, "desk.title", "Fresh");
    assert_eq!(receipt.state, ReceiptState::Completed, "{receipt:?}");
    set.run();
    assert_eq!(node(&set, "desk.title").value.as_deref(), Some("Fresh"));
    apply(&mut set);

    let (typed, set) = (host(&typed).app(), host(&set).app());
    assert_eq!(set.title, "Fresh");
    assert_eq!(typed.title, set.title);
    assert_eq!(typed.history, set.history);
    assert_eq!(set.history, [("Fresh".to_owned(), String::new())]);
    // One set text is one edit; typing five characters was five.
    assert_eq!(set.edits, 1);
    assert!(set.errors.is_empty() && typed.errors.is_empty());
}

#[test]
fn set_text_returns_focus_to_where_it_was() {
    let mut harness = harness(desk());
    set_text(&mut harness, "desk.title", "Fresh");
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), None);
    assert!(!node(&harness, "desk.title").focused);
    // A field that already has focus keeps it.
    harness
        .ctx
        .memory_mut(|memory| memory.request_focus(egui::Id::new("notes")));
    harness.run();
    set_text(&mut harness, "desk.title", "Again");
    harness.run();
    assert_eq!(host(&harness).app().title, "Again");
    assert_eq!(
        harness.ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new("notes"))
    );
}

#[test]
fn accesskit_set_value_takes_the_same_route() {
    let mut harness = harness(desk());
    let widget = egui::Id::new("desk.title");
    harness.event(egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::SetValue,
            target_node: widget.accesskit_id(),
            target_tree: egui::accesskit::TreeId::ROOT,
            data: Some(egui::accesskit::ActionData::Value("Spoken".into())),
        },
    ));
    harness.run();
    assert_eq!(host(&harness).app().title, "Spoken");
    assert_eq!(host(&harness).app().edits, 1);
    // Assistive technology edits in place, so the field keeps focus.
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(widget));
}

#[test]
fn multiline_keeps_newlines_and_single_line_flattens_them_as_paste_does() {
    let mut harness = harness(desk());
    set_text(&mut harness, "desk.notes", "First\nSecond");
    set_text(&mut harness, "desk.title", "One\nTwo");
    harness.run();
    let app = host(&harness).app();
    assert_eq!(app.notes, "First\nSecond");
    assert_eq!(app.title, "One Two");
}

#[test]
fn empty_text_clears_and_equal_text_changes_nothing() {
    let mut harness = harness(desk());
    assert_eq!(
        set_text(&mut harness, "desk.title", "Old title").state,
        ReceiptState::Completed
    );
    harness.run();
    assert_eq!(host(&harness).app().edits, 0);
    set_text(&mut harness, "desk.title", "");
    harness.run();
    assert_eq!(host(&harness).app().title, "");
    assert_eq!(host(&harness).app().edits, 1);
}

#[test]
fn queued_edits_apply_one_per_pass_in_order() {
    let mut harness = harness(desk());
    for (field, text) in [("desk.title", "A"), ("desk.notes", "B")] {
        let current = host(&harness);
        let expected = discover(current, by_id(field)).target;
        let admitted = receipt(current.inspection().unwrap().handle(request(
            current,
            field,
            InspectionOperation::Invoke {
                selector: by_id(field),
                expected,
                arguments: serde_json::json!({ "value": text }),
            },
        )));
        assert_eq!(admitted.state, ReceiptState::Queued);
    }
    harness.run();
    let app = host(&harness).app();
    assert_eq!((app.title.as_str(), app.notes.as_str()), ("A", "B"));
}

#[test]
fn a_disabled_field_rejects_set_text_and_ignores_set_value() {
    let mut harness = harness(Desk {
        locked: true,
        ..desk()
    });
    let capability = discover(host(&harness), by_id("desk.title"));
    assert!(!capability.availability.enabled());
    let receipt = set_text(&mut harness, "desk.title", "Changed");
    assert_eq!(receipt.state, ReceiptState::Rejected);
    let error = receipt.error.unwrap();
    assert_eq!(error.code, InspectionErrorCode::Unavailable);
    assert_eq!(error.message, "The record is locked");
    harness.event(egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::SetValue,
            target_node: egui::Id::new("desk.title").accesskit_id(),
            target_tree: egui::accesskit::TreeId::ROOT,
            data: Some(egui::accesskit::ActionData::Value("Changed".into())),
        },
    ));
    harness.run();
    assert_eq!(host(&harness).app().title, "Old title");
    assert_eq!(host(&harness).app().edits, 0);
}

#[test]
fn a_field_without_an_author_id_pairs_by_geometry_and_gains_one() {
    let harness = harness(desk());
    let fields = &host(&harness).text_fields;
    let (notes, _) = fields
        .iter()
        .find(|(field, _)| field.id.0 == "desk.notes")
        .expect("notes pair by rectangle");
    assert_eq!(notes.widget, egui::Id::new("notes"));
    // The AccessKit tree now names the field by its semantic ID, as the
    // parity audit requires.
    fn collect(
        node: egui_kittest::kittest::AccessKitNode<'_>,
        update: &mut egui::accesskit::TreeUpdate,
    ) {
        update.nodes.push((node.locate().0, node.data().clone()));
        for child in node.children() {
            collect(child, update);
        }
    }
    let root = harness.kittest_state().root();
    let mut update = egui::accesskit::TreeUpdate {
        nodes: Vec::new(),
        tree: Some(egui::accesskit::Tree::new(root.locate().0)),
        tree_id: egui::accesskit::TreeId::ROOT,
        focus: root.locate().0,
    };
    collect(root, &mut update);
    let mismatches = crate::audit_accesskit(host(&harness).ui_snapshot(), &update);
    assert!(
        !mismatches.iter().any(|mismatch| matches!(
            mismatch,
            crate::AccessKitMismatch::MissingNode { id } if id.0 == "desk.notes"
        )),
        "{mismatches:?}"
    );
    assert!(
        update
            .nodes
            .iter()
            .any(|(_, node)| node.author_id() == Some("desk.notes")
                && node.supports_action(egui::accesskit::Action::SetValue))
    );
}

#[test]
fn typed_actions_reach_perform_with_from_controls_and_invocation() {
    let mut harness = harness(desk());
    // A control triggers a valid choice.
    host_mut(&mut harness).app_mut().physical_colour = Some(ActionArgument::choice(Colour::Blue));
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    assert_eq!(host(&harness).app().colour, Some(Colour::Blue));
    // An invalid physical argument is reported, not performed.
    host_mut(&mut harness).app_mut().physical_colour = Some(ActionArgument::Boolean(true));
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    assert_eq!(host(&harness).app().colour, Some(Colour::Blue));
    assert_eq!(host(&harness).app().errors.len(), 1);
    assert!(host(&harness).app().errors[0].contains("not a boolean"));
    // Semantic invocation delivers typed values; invalid ones never reach the
    // application.
    let colour = InspectionSelector {
        capability: Some("desk.colour".into()),
        ..Default::default()
    };
    let receipt = invoke(
        &mut harness,
        "red",
        colour.clone(),
        serde_json::json!({"value": "red"}),
    );
    assert_eq!(receipt.state, ReceiptState::Completed);
    assert_eq!(host(&harness).app().colour, Some(Colour::Red));
    let receipt = invoke(
        &mut harness,
        "green",
        colour,
        serde_json::json!({"value": "green"}),
    );
    assert_eq!(receipt.state, ReceiptState::Rejected);
    assert_eq!(
        receipt.error.unwrap().code,
        InspectionErrorCode::InvalidArguments
    );
    let pin = InspectionSelector {
        capability: Some("desk.pin".into()),
        ..Default::default()
    };
    invoke(
        &mut harness,
        "pin",
        pin,
        serde_json::json!({"value": false}),
    );
    assert_eq!(host(&harness).app().pinned, Some(false));
    assert_eq!(host(&harness).app().colour, Some(Colour::Red));
}

#[test]
fn consecutive_set_values_of_one_field_apply_the_latest_text_in_one_pass() {
    let mut harness = harness(desk());
    let widget = egui::Id::new("desk.title");
    // A layer field sends its whole text on each keystroke; one browser
    // frame can deliver several.
    harness
        .input_mut()
        .events
        .extend(["O", "Ol", "Old"].map(|text| {
            egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target_node: widget.accesskit_id(),
                target_tree: egui::accesskit::TreeId::ROOT,
                data: Some(egui::accesskit::ActionData::Value(text.into())),
            })
        }));
    harness.step();
    assert_eq!(host(&harness).app().title, "Old");
    assert_eq!(host(&harness).app().edits, 1);
    assert!(host(&harness).text_edits.is_empty());
}

#[test]
fn a_semantic_edit_waits_out_a_state_change_and_rejects_a_repurposed_field() {
    let mut harness = harness(desk());
    let notes = discover(host(&harness), by_id("desk.notes")).target;
    let pin = InspectionSelector {
        capability: Some("desk.pin".into()),
        ..Default::default()
    };
    let pin_target = discover(host(&harness), pin.clone()).target;
    let current = host(&harness);
    for (id, selector, expected, value) in [
        (
            "notes",
            by_id("desk.notes"),
            notes,
            serde_json::json!("Pinned notes"),
        ),
        ("pin", pin, pin_target, serde_json::json!(true)),
    ] {
        let admitted = receipt(current.inspection().unwrap().handle(request(
            current,
            id,
            InspectionOperation::Invoke {
                selector,
                expected,
                arguments: serde_json::json!({ "value": value }),
            },
        )));
        assert_eq!(admitted.state, ReceiptState::Queued);
    }
    harness.run();
    let host = host(&harness);
    // Pinning repurposed the notes widget, so the edit queued for the old
    // field is rejected rather than written into the new one.
    assert_eq!(host.app().pinned, Some(true));
    assert_eq!(host.app().notes, "");
    let rejected = receipt(host.inspection().unwrap().handle(request(
        host,
        "notes-receipt",
        InspectionOperation::Receipt {
            request_id: "notes".into(),
        },
    )));
    assert_eq!(rejected.state, ReceiptState::Rejected);
    assert!(
        rejected
            .error
            .as_ref()
            .unwrap()
            .message
            .contains("no longer presented"),
        "{rejected:?}"
    );
}

#[test]
fn in_place_edits_go_before_queued_semantic_edits() {
    let mut harness = harness(desk());
    let current = host(&harness);
    let expected = discover(current, by_id("desk.notes")).target;
    receipt(current.inspection().unwrap().handle(request(
        current,
        "notes",
        InspectionOperation::Invoke {
            selector: by_id("desk.notes"),
            expected,
            arguments: serde_json::json!({"value": "Queued"}),
        },
    )));
    harness
        .input_mut()
        .events
        .push(egui::Event::AccessKitActionRequest(
            egui::accesskit::ActionRequest {
                action: egui::accesskit::Action::SetValue,
                target_node: egui::Id::new("desk.title").accesskit_id(),
                target_tree: egui::accesskit::TreeId::ROOT,
                data: Some(egui::accesskit::ActionData::Value("Typed".into())),
            },
        ));
    harness.step();
    assert_eq!(host(&harness).app().title, "Typed");
    assert_eq!(host(&harness).app().notes, "");
    harness.run();
    assert_eq!(host(&harness).app().notes, "Queued");
}

#[test]
fn password_fields_are_never_set_or_valued() {
    let harness = harness(Desk {
        secret: true,
        ..desk()
    });
    assert_eq!(node(&harness, "desk.title").value, None);
    let reply = host(&harness).inspection().unwrap().handle(request(
        host(&harness),
        "discover",
        InspectionOperation::Discover {
            selector: by_id("desk.title"),
            limit: 8,
            cursor: None,
        },
    ));
    let Some(InspectionResult::Discover { total, .. }) = reply.result else {
        panic!("discovery");
    };
    assert_eq!(total, 0);
}

#[test]
fn a_set_text_dispatched_between_passes_also_waits_out_a_state_change() {
    // The browser route runs invocations between passes.
    let mut harness = harness(desk());
    for (id, selector, value) in [
        (
            "notes",
            by_id("desk.notes"),
            serde_json::json!("Wrong place"),
        ),
        (
            "pin",
            InspectionSelector {
                capability: Some("desk.pin".into()),
                ..Default::default()
            },
            serde_json::json!(true),
        ),
    ] {
        let expected = discover(host(&harness), selector.clone()).target;
        let operation = InspectionOperation::Invoke {
            selector,
            expected,
            arguments: serde_json::json!({ "value": value }),
        };
        let request = request(host(&harness), id, operation);
        host_mut(&mut harness).dispatch(request).unwrap();
    }
    harness.run();
    let host = host(&harness);
    assert_eq!(host.app().notes_id, Some("desk.notes.pinned"));
    assert_eq!(host.app().notes, "");
    let rejected = receipt(host.inspection().unwrap().handle(request(
        host,
        "notes-receipt",
        InspectionOperation::Receipt {
            request_id: "notes".into(),
        },
    )));
    assert_eq!(rejected.state, ReceiptState::Rejected, "{rejected:?}");
}
