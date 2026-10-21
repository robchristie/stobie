#![cfg(test)]
//! The labelled text field recipe through the host: its caption names it,
//! edits reach the application, Return submits a single line, an error is
//! announced and described, agents set its text, and a password field is
//! never published or set.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::Harness;
use serde::Serialize;

use super::*;
use crate::{
    Action, InspectionOperation, InspectionResult, InspectionSelector, InvocationReceipt,
    PresentationScope, ReceiptState, TextFieldSpec, TextRole,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "join")]
enum Act {
    #[action(label = "Join", description = "Join the household")]
    Join,
}

enum Intent {
    Name(String),
    Notes(String),
    Code(String),
    Password(String),
}

/// A form to join a household: a name, notes, an invitation code that is
/// checked as it is typed, and a password.
#[derive(Default)]
struct Form {
    name: String,
    notes: String,
    code: String,
    password: String,
    /// The form is disabled, as while it is being sent.
    disabled: bool,
    edits: u32,
}

#[derive(Default)]
struct View {
    name: Option<egui::Response>,
    notes: Option<egui::Response>,
    submitted: u32,
    /// The token hit height.
    hit: f32,
}

const PASSWORD: &str = "hunter2-correct-horse";
const CODE_ERROR: &str = "An invitation code has six letters.";

impl Application for Form {
    type Action = Act;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Join")
    }

    fn availability(&self, _: ActionTarget<Act>) -> Availability {
        Availability::Enabled
    }

    fn report_error(&mut self, message: String) {
        panic!("{message}");
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, Act, Intent>) {
        let mut p = frame.presentation(ui, PresentationScope::new("join"), SemanticUiId::root());
        view.hit = crate::components::action_hit_height(p.tokens(), p.font_scale());
        ui.add_enabled_ui(!self.disabled, |ui| {
            let name = frame.text_field(
                ui,
                &mut p,
                "name",
                &TextFieldSpec::new("Name").hint("Ada Lovelace"),
                &self.name,
                Intent::Name,
            );
            view.submitted += u32::from(name.submitted);
            view.name = Some(name.field);
            let notes = frame.text_field(
                ui,
                &mut p,
                "notes",
                &TextFieldSpec::new("Notes")
                    .multiline(2)
                    .description("Only the household sees these."),
                &self.notes,
                Intent::Notes,
            );
            view.submitted += u32::from(notes.submitted);
            view.notes = Some(notes.field);
            let invalid = !self.code.is_empty() && self.code.chars().count() != 6;
            frame.text_field(
                ui,
                &mut p,
                "code",
                &TextFieldSpec::new("Invitation code")
                    .id("join.code")
                    .spellcheck(false)
                    .error(invalid.then_some(CODE_ERROR)),
                &self.code,
                Intent::Code,
            );
            frame.text_field(
                ui,
                &mut p,
                "password",
                &TextFieldSpec::new("Password")
                    .id("join.password")
                    .password(),
                &self.password,
                Intent::Password,
            );
        });
    }

    fn apply(&mut self, intent: Intent, _: &mut View) -> Result<bool, String> {
        self.edits += 1;
        match intent {
            Intent::Name(text) => self.name = text,
            Intent::Notes(text) => self.notes = text,
            Intent::Code(text) => self.code = text,
            Intent::Password(text) => self.password = text,
        }
        Ok(true)
    }

    fn perform(&mut self, _: ActionTarget<Act>, _: &mut View) -> Result<bool, String> {
        Ok(true)
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Form>>,
}

fn harness(form: Form) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(480.0, 600.0))
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
    let mut host = AppHost::new(&context, form);
    host.enable_inspection(Inspection::new("join-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Form> {
    harness.state().host.as_ref().unwrap()
}

fn node(harness: &Harness<'_, Probe>, name: &str) -> UiNode {
    let snapshot = host(harness).ui_snapshot();
    let mut found: Vec<_> = snapshot
        .nodes
        .iter()
        .filter(|node| {
            matches!(node.role, UiRole::TextInput | UiRole::MultilineTextInput) && node.name == name
        })
        .collect();
    assert_eq!(found.len(), 1, "one field named {name}: {found:?}");
    found.remove(0).clone()
}

fn accesskit_update(harness: &Harness<'_, Probe>) -> egui::accesskit::TreeUpdate {
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
    update
}

fn accesskit_node(harness: &Harness<'_, Probe>, author: &str) -> egui::accesskit::Node {
    accesskit_update(harness)
        .nodes
        .into_iter()
        .find(|(_, node)| node.author_id() == Some(author))
        .unwrap_or_else(|| panic!("no AccessKit node for {author}"))
        .1
}

fn assert_clean(harness: &Harness<'_, Probe>) {
    let snapshot = host(harness).ui_snapshot();
    assert!(
        snapshot.semantic_audit.is_empty(),
        "{:?}",
        snapshot.semantic_audit
    );
    assert!(snapshot.text_audit.is_empty(), "{:?}", snapshot.text_audit);
    let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(harness));
    assert!(mismatches.is_empty(), "{mismatches:?}");
}

fn click(harness: &mut Harness<'_, Probe>, at: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run();
}

fn focused(harness: &Harness<'_, Probe>, response: &egui::Response) -> bool {
    harness.ctx.memory(|memory| memory.has_focus(response.id))
}

fn request(
    harness: &Harness<'_, Probe>,
    id: &str,
    operation: InspectionOperation,
) -> InspectionReply {
    let host = host(harness);
    let inspection = host.inspection().unwrap();
    inspection.handle(InspectionRequest {
        version: 1,
        request_id: id.into(),
        instance: Some(inspection.instance()),
        operation,
    })
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

fn by_id(id: &SemanticUiId) -> InspectionSelector {
    InspectionSelector {
        id: Some(id.clone()),
        ..Default::default()
    }
}

#[test]
fn the_visible_caption_names_the_field() {
    let harness = harness(Form::default());
    assert_clean(&harness);
    let snapshot = host(&harness).ui_snapshot();
    for caption in ["Name", "Notes", "Invitation code", "Password"] {
        assert!(
            snapshot
                .text
                .iter()
                .any(|text| text.role == TextRole::Caption && text.content_text() == Some(caption)),
            "{caption} is a measured caption"
        );
    }
    let name = node(&harness, "Name");
    assert_eq!(name.role, UiRole::TextInput);
    assert!(name.text_selectable);
    let notes = node(&harness, "Notes");
    assert_eq!(notes.role, UiRole::MultilineTextInput);
    assert_eq!(
        notes.description.as_deref(),
        Some("Only the household sees these.")
    );
    // AccessKit labels the field by its caption's node.
    let field = accesskit_node(&harness, &name.id.0);
    assert_eq!(field.label(), Some("Name"));
    let caption = accesskit_update(&harness)
        .nodes
        .into_iter()
        .find(|(id, _)| field.labelled_by().contains(id))
        .expect("the caption's node")
        .1;
    assert_eq!(caption.value().or(caption.label()), Some("Name"));
    // An explicit ID is the field's semantic ID.
    assert_eq!(node(&harness, "Invitation code").id.0, "join.code");
}

#[test]
fn a_single_line_field_is_at_least_the_hit_height_and_as_wide_as_offered() {
    let harness = harness(Form::default());
    let name = Rect::from(node(&harness, "Name").rect);
    let hit = host(&harness).ui_state().hit;
    assert!(name.height() >= hit - 0.5, "{name:?} < {hit}");
    let notes = Rect::from(node(&harness, "Notes").rect);
    assert!(notes.height() > name.height(), "two rows are taller");
    assert!((notes.width() - name.width()).abs() < 0.5);
}

#[test]
fn typing_emits_the_new_text_and_return_submits_a_single_line() {
    let mut harness = harness(Form::default());
    let name = host(&harness).ui_state().name.clone().unwrap();
    click(&mut harness, name.rect.center());
    assert!(focused(&harness, &name));
    harness.event(egui::Event::Text("Ada".into()));
    harness.run();
    assert_eq!(host(&harness).app().name, "Ada");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(host(&harness).ui_state().submitted, 1);
    assert!(focused(&harness, &name), "Return keeps the focus");
    assert_eq!(host(&harness).app().name, "Ada");
    // Shift+Return is not a submission.
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Enter);
    harness.run();
    assert_eq!(host(&harness).ui_state().submitted, 1);

    // A multiline field's Return types a newline.
    let notes = host(&harness).ui_state().notes.clone().unwrap();
    click(&mut harness, notes.rect.center());
    harness.event(egui::Event::Text("One".into()));
    harness.key_press(Key::Enter);
    harness.event(egui::Event::Text("Two".into()));
    harness.run();
    assert_eq!(host(&harness).app().notes, "One\nTwo");
    assert_eq!(host(&harness).ui_state().submitted, 1);
}

#[test]
fn an_error_is_shown_described_marked_and_announced() {
    let mut harness = harness(Form::default());
    // Before any error, the live status is present and empty.
    let status = accesskit_node(&harness, "join.code.status");
    assert_eq!(status.role(), egui::accesskit::Role::Status);
    assert_eq!(status.live(), Some(egui::accesskit::Live::Polite));
    assert_eq!(status.label(), Some(""));
    assert!(
        !host(&harness)
            .ui_snapshot()
            .nodes
            .iter()
            .any(|node| node.role == UiRole::Status)
    );

    harness.state_mut().host.as_mut().unwrap().app_mut().code = "abc".into();
    harness.run();
    assert_clean(&harness);
    let code = node(&harness, "Invitation code");
    assert_eq!(code.description.as_deref(), Some(CODE_ERROR));
    let field = accesskit_node(&harness, "join.code");
    assert_eq!(field.invalid(), Some(egui::accesskit::Invalid::True));
    assert_eq!(field.description(), Some(CODE_ERROR));
    let status = accesskit_node(&harness, "join.code.status");
    assert_eq!(status.label(), Some(CODE_ERROR));
    let snapshot = host(&harness).ui_snapshot();
    let published = snapshot
        .nodes
        .iter()
        .find(|node| node.role == UiRole::Status)
        .expect("the error's status");
    assert_eq!(published.name, CODE_ERROR);
    assert!(
        snapshot
            .text
            .iter()
            .any(|text| text.role == TextRole::Error && text.content_text() == Some(CODE_ERROR)),
        "the error is measured text"
    );

    harness.state_mut().host.as_mut().unwrap().app_mut().code = "abcdef".into();
    harness.run();
    assert_eq!(node(&harness, "Invitation code").description, None);
    assert_eq!(accesskit_node(&harness, "join.code").invalid(), None);
}

#[test]
fn agents_set_the_text_of_a_field_by_its_id() {
    let mut harness = harness(Form::default());
    let name = node(&harness, "Name");
    let reply = request(
        &harness,
        "discover",
        InspectionOperation::Discover {
            selector: by_id(&name.id),
            limit: 8,
            cursor: None,
        },
    );
    let Some(InspectionResult::Discover {
        mut capabilities, ..
    }) = reply.result
    else {
        panic!("discovery: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1);
    let expected = capabilities.remove(0).target;
    let admitted = receipt(request(
        &harness,
        "set",
        InspectionOperation::Invoke {
            selector: by_id(&name.id),
            expected,
            arguments: serde_json::json!({ "value": "Grace Hopper" }),
        },
    ));
    assert_eq!(admitted.state, ReceiptState::Queued, "{admitted:?}");
    harness.run();
    assert_eq!(host(&harness).app().name, "Grace Hopper");
    assert_eq!(
        node(&harness, "Name").value.as_deref(),
        Some("Grace Hopper")
    );
}

#[test]
fn a_password_field_is_never_published_or_set() {
    let harness = harness(Form {
        password: PASSWORD.into(),
        ..Form::default()
    });
    assert_clean(&harness);
    let password = node(&harness, "Password");
    assert_eq!(password.role, UiRole::TextInput);
    assert_eq!(password.value, None);
    let json = serde_json::to_string(host(&harness).ui_snapshot()).unwrap();
    assert!(!json.contains(PASSWORD), "{json}");
    assert_eq!(
        accesskit_node(&harness, "join.password").role(),
        egui::accesskit::Role::PasswordInput
    );
    let reply = request(
        &harness,
        "discover",
        InspectionOperation::Discover {
            selector: by_id(&password.id),
            limit: 8,
            cursor: None,
        },
    );
    let Some(InspectionResult::Discover { total, .. }) = reply.result else {
        panic!("discovery");
    };
    assert_eq!(total, 0);
}

#[test]
fn a_disabled_ui_disables_the_fields() {
    let mut harness = harness(Form {
        disabled: true,
        ..Form::default()
    });
    assert_clean(&harness);
    assert!(!node(&harness, "Name").enabled);
    let name = host(&harness).ui_state().name.clone().unwrap();
    click(&mut harness, name.rect.center());
    harness.event(egui::Event::Text("Ada".into()));
    harness.run();
    assert_eq!(host(&harness).app().edits, 0);
}
