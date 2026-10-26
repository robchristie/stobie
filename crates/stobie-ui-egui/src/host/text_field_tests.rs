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
    #[action(label = "Join", description = "Join the group")]
    Join,
}

enum Intent {
    Name(String),
    Notes(String),
    Code(String),
    Password(String),
}

/// A form to join a group: a name, notes, an invitation code that is
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
                    .description("Only you see these."),
                &self.notes,
                Intent::Notes,
            );
            view.submitted += u32::from(notes.submitted);
            view.notes = Some(notes.field);
            let invalid = !self.code.is_empty() && self.code.len() != 6;
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

fn accesskit_update<S>(harness: &Harness<'_, S>) -> egui::accesskit::TreeUpdate {
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
    assert_eq!(notes.description.as_deref(), Some("Only you see these."));
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

/// A custom field, observed by hand with no AccessKit author, in a scroll
/// area inside an `observe_visible_only` region, as a field in a dock pane
/// is.
mod scrolled {
    use super::*;
    use crate::{NativeTextControlKind, observe_visible_only};

    /// The height of the scrolled view.
    const VIEW: f32 = 200.0;
    /// The space above the field in the scrolled content.
    const ABOVE: f32 = 300.0;
    /// How much of the field's top is scrolled out of view.
    const HIDDEN: f32 = 6.0;
    const ID: &str = "scrolled.draft";

    #[derive(Default)]
    struct Draft {
        text: String,
        /// Scroll the field's top out of view.
        scrolled: bool,
    }

    impl Application for Draft {
        type Action = Act;
        type Intent = String;
        type UiState = Option<egui::Response>;

        fn identity(&self) -> ApplicationIdentity {
            crate::application_identity!("Draft")
        }

        fn availability(&self, _: ActionTarget<Act>) -> Availability {
            Availability::Enabled
        }

        fn report_error(&mut self, message: String) {
            panic!("{message}");
        }

        fn ui(
            &self,
            ui: &mut Ui,
            view: &mut Option<egui::Response>,
            frame: &mut Frame<'_, Act, String>,
        ) {
            let offset = if self.scrolled { ABOVE + HIDDEN } else { 0.0 };
            observe_visible_only(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(VIEW)
                    .min_scrolled_height(VIEW)
                    .auto_shrink(false)
                    .vertical_scroll_offset(offset)
                    .show(ui, |ui| {
                        let mut p = frame.presentation(
                            ui,
                            PresentationScope::new("scrolled"),
                            SemanticUiId::root(),
                        );
                        ui.add_space(ABOVE);
                        let mut text = self.text.clone();
                        // A hand-built field: labelled, with no author ID.
                        let (field, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
                            let field = ui.add(egui::TextEdit::singleline(&mut text));
                            ui.ctx()
                                .accesskit_node_builder(field.id, |node| node.set_label("Draft"));
                            (field, ())
                        });
                        let mut node = UiNode::container(
                            SemanticUiId::new(ID),
                            Some(SemanticUiId::root()),
                            UiRole::TextInput,
                            field.rect.into(),
                        );
                        node.name = "Draft".into();
                        node.focused = field.has_focus();
                        p.observe_node(ui, node);
                        if text != self.text {
                            frame.emit(text);
                        }
                        ui.add_space(2.0 * VIEW);
                        p.finish(ui);
                        *view = Some(field);
                    });
            });
        }

        fn apply(&mut self, text: String, _: &mut Option<egui::Response>) -> Result<bool, String> {
            self.text = text;
            Ok(true)
        }

        fn perform(
            &mut self,
            _: ActionTarget<Act>,
            _: &mut Option<egui::Response>,
        ) -> Result<bool, String> {
            Ok(true)
        }
    }

    #[derive(Default)]
    struct Probe {
        host: Option<AppHost<Draft>>,
    }

    fn harness(draft: Draft) -> Harness<'static, Probe> {
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
        let mut host = AppHost::new(&context, draft);
        host.enable_inspection(Inspection::new("scrolled-test"));
        harness.state_mut().host = Some(host);
        harness.run();
        harness
    }

    fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Draft> {
        harness.state().host.as_ref().unwrap()
    }

    fn request(
        harness: &Harness<'_, Probe>,
        id: &str,
        operation: InspectionOperation,
    ) -> InspectionReply {
        let inspection = host(harness).inspection().unwrap();
        inspection.handle(InspectionRequest {
            version: 1,
            request_id: id.into(),
            instance: Some(inspection.instance()),
            operation,
        })
    }

    #[test]
    fn a_custom_field_partly_scrolled_out_of_view_keeps_set_text() {
        let mut harness = harness(Draft {
            scrolled: true,
            ..Draft::default()
        });
        // The field is partly in view: its node is clipped to the part shown.
        let field = host(&harness).ui_state().clone().unwrap();
        let snapshot = host(&harness).ui_snapshot();
        let node = snapshot
            .nodes
            .iter()
            .find(|node| node.id.0 == ID)
            .expect("the field is published while partly in view");
        let shown = Rect::from(node.rect);
        assert!(
            shown.height() < field.rect.height() - HIDDEN / 2.0,
            "{shown:?} is clipped from {:?}",
            field.rect
        );
        assert!((shown.max.y - field.rect.max.y).abs() < 0.5);
        // The host paired the field, which sets no author ID, and its
        // AccessKit node presents the same part.
        let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(&harness));
        assert!(mismatches.is_empty(), "{mismatches:?}");

        // It still pairs with its text edit, so agents can set its text.
        let id = SemanticUiId::new(ID);
        let reply = request(
            &harness,
            "discover",
            InspectionOperation::Discover {
                selector: by_id(&id),
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
        assert_eq!(capabilities.len(), 1, "{capabilities:?}");
        let expected = capabilities.remove(0).target;
        let admitted = receipt(request(
            &harness,
            "set",
            InspectionOperation::Invoke {
                selector: by_id(&id),
                expected,
                arguments: serde_json::json!({ "value": "Partly in view" }),
            },
        ));
        assert_eq!(admitted.state, ReceiptState::Queued, "{admitted:?}");
        harness.run();
        assert_eq!(host(&harness).app().text, "Partly in view");
        let snapshot = host(&harness).ui_snapshot();
        let node = snapshot.nodes.iter().find(|node| node.id.0 == ID).unwrap();
        assert_eq!(node.value.as_deref(), Some("Partly in view"));
        let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(&harness));
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }
}
