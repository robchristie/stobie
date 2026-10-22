#![cfg(test)]
//! The composer recipe through the host: growth, Return conventions, the
//! send control, focus, refusal, hidden controls and agent routes; and the
//! hidden-action fix for the plain action controls.

use egui::{Key, Modifiers, Vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde::Serialize;

use super::*;
use crate::{
    Action, ActionButtonSpec, ActionButtonState, CapabilityDescription, ComposerControl,
    ComposerSpec, InspectionErrorCode, InspectionOperation, InspectionResult, InspectionSelector,
    InvocationReceipt, PresentationScope, ReceiptState, ResolvedTarget,
};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "chat")]
enum ChatAct {
    #[action(
        label = "Send",
        description = "Send the message",
        argument = "text",
        max_length = 40
    )]
    Send,
    #[action(label = "Attach", description = "Attach a file to the message")]
    Attach,
    #[action(label = "Dictate", description = "Dictate a message")]
    Dictate,
    #[action(label = "Archive", description = "Archive the conversation")]
    Archive,
    #[action(label = "Mute", description = "Mute the conversation")]
    Mute,
}

enum Intent {
    Draft(String),
}

/// A conversation whose draft the application owns.
struct Chat {
    draft: String,
    sent: Vec<String>,
    attached: u32,
    dictated: u32,
    errors: Vec<String>,
    /// Refuse each send, as a service over its limit would.
    refuse: bool,
    /// A send is in flight: Send is disabled.
    sending: bool,
    /// An empty draft shows the microphone alone: Send is hidden.
    mic_mode: bool,
    hide_attach: bool,
    /// Present Archive and Mute through the plain action controls.
    plain_controls: bool,
    /// Archive and Mute are hidden.
    plain_hidden: bool,
    max_rows: usize,
    /// Draft edits applied, one per changed pass.
    edits: u32,
    /// The composer's own ID, given with `composer_with_id`.
    id: Option<&'static str>,
}

impl Default for Chat {
    fn default() -> Self {
        Self {
            draft: String::new(),
            sent: Vec::new(),
            attached: 0,
            dictated: 0,
            errors: Vec::new(),
            refuse: false,
            sending: false,
            mic_mode: false,
            hide_attach: false,
            plain_controls: false,
            plain_hidden: false,
            max_rows: 3,
            edits: 0,
            id: None,
        }
    }
}

#[derive(Default)]
struct View {
    field: Option<egui::Response>,
    send: Option<egui::Response>,
    leading: Option<egui::Response>,
    trailing: Option<egui::Response>,
    submitted: Vec<String>,
}

impl Application for Chat {
    type Action = ChatAct;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Chat")
    }

    fn availability(&self, target: ActionTarget<ChatAct>) -> Availability {
        self.availability_with(target, &ActionArgument::None)
    }

    fn availability_with(
        &self,
        target: ActionTarget<ChatAct>,
        argument: &ActionArgument,
    ) -> Availability {
        match target.action {
            ChatAct::Send if self.sending => Availability::Disabled {
                reason: "The last message is still sending".into(),
            },
            ChatAct::Send => match argument {
                ActionArgument::Text(text) if !text.trim().is_empty() => Availability::Enabled,
                _ if self.mic_mode => Availability::Hidden,
                _ => Availability::Disabled {
                    reason: "Type a message first".into(),
                },
            },
            ChatAct::Attach if self.hide_attach => Availability::Hidden,
            ChatAct::Archive | ChatAct::Mute if self.plain_hidden => Availability::Hidden,
            _ => Availability::Enabled,
        }
    }

    fn ui(&self, ui: &mut Ui, view: &mut View, frame: &mut Frame<'_, ChatAct, Intent>) {
        let mut p = frame.presentation(ui, PresentationScope::new("chat"), SemanticUiId::root());
        if self.plain_controls {
            frame.action(ui, &mut p, ChatAct::Archive, ActionEmphasis::Normal);
            frame.icon_action(
                ui,
                &mut p,
                ChatAct::Mute,
                IconId::Notifications,
                ActionEmphasis::Quiet,
            );
            frame.toggle(ui, &mut p, "mute", ChatAct::Mute, false);
            p.action(
                ui,
                "archive-direct",
                ActionButtonSpec {
                    target: ActionTarget::application(ChatAct::Archive),
                    availability: frame.availability(ChatAct::Archive),
                    state: ActionButtonState::Momentary,
                    emphasis: ActionEmphasis::Normal,
                    compact: false,
                },
            );
        }
        let spec = ComposerSpec::new(
            "Message",
            ComposerControl::icon(ChatAct::Send, IconId::Send),
        )
        .hint("Message Ada")
        .max_rows(self.max_rows)
        .leading(ComposerControl::icon(ChatAct::Attach, IconId::Attach))
        .trailing(ComposerControl::label(ChatAct::Dictate));
        let response = match self.id {
            Some(id) => frame.composer_with_id(ui, &mut p, id, &spec, &self.draft, Intent::Draft),
            None => frame.composer(ui, &mut p, "composer", &spec, &self.draft, Intent::Draft),
        };
        view.field = Some(response.field);
        view.send = Some(response.send.response);
        view.leading = response.leading.map(|control| control.response);
        view.trailing = response.trailing.map(|control| control.response);
        view.submitted.extend(response.submitted);
    }

    fn apply(&mut self, intent: Intent, _view: &mut View) -> Result<bool, String> {
        match intent {
            Intent::Draft(text) => {
                self.edits += 1;
                self.draft = text;
            }
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<ChatAct>, _view: &mut View) -> Result<bool, String> {
        match target.action {
            ChatAct::Attach => self.attached += 1,
            ChatAct::Dictate => self.dictated += 1,
            ChatAct::Archive | ChatAct::Mute => {}
            ChatAct::Send => return Err("Send needs a message".into()),
        }
        Ok(true)
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<ChatAct>,
        argument: ActionArgument,
        view: &mut View,
    ) -> Result<bool, String> {
        if target.action != ChatAct::Send {
            return self.perform(target, view);
        }
        let ActionArgument::Text(text) = argument else {
            return Err("Send needs a message".into());
        };
        if self.refuse {
            // The draft stays for the person to send later.
            return Err("Daily message limit reached".into());
        }
        self.sent.push(text);
        self.draft.clear();
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.errors.push(message);
    }
}

#[derive(Default)]
struct Probe {
    host: Option<AppHost<Chat>>,
}

fn harness(chat: Chat) -> Harness<'static, Probe> {
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
    let mut host = AppHost::new(&context, chat);
    host.enable_inspection(Inspection::new("chat-test"));
    harness.state_mut().host = Some(host);
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Chat> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Chat> {
    harness.state_mut().host.as_mut().unwrap()
}

fn app<'a>(harness: &'a Harness<'_, Probe>) -> &'a Chat {
    host(harness).app()
}

fn view<'a>(harness: &'a Harness<'_, Probe>) -> &'a View {
    host(harness).ui_state()
}

fn field_rect(harness: &Harness<'_, Probe>) -> Rect {
    view(harness).field.as_ref().unwrap().rect
}

fn send_rect(harness: &Harness<'_, Probe>) -> Rect {
    view(harness).send.as_ref().unwrap().rect
}

fn field_focused(harness: &Harness<'_, Probe>) -> bool {
    let id = view(harness).field.as_ref().unwrap().id;
    harness.ctx.memory(|memory| memory.has_focus(id))
}

fn field_node(harness: &Harness<'_, Probe>) -> UiNode {
    let snapshot = host(harness).ui_snapshot();
    let mut fields: Vec<_> = snapshot.by_role(UiRole::MultilineTextInput).collect();
    assert_eq!(fields.len(), 1, "one composer field");
    fields.remove(0).clone()
}

fn send_nodes(harness: &Harness<'_, Probe>) -> Vec<UiNode> {
    host(harness)
        .ui_snapshot()
        .by_action(ChatAct::Send)
        .cloned()
        .collect()
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

fn focus_field(harness: &mut Harness<'_, Probe>) {
    let centre = field_rect(harness).center();
    click(harness, centre);
    assert!(field_focused(harness), "a click focuses the field");
}

fn type_text(harness: &mut Harness<'_, Probe>, text: &str) {
    harness.event(egui::Event::Text(text.to_owned()));
    harness.run();
}

fn press(harness: &mut Harness<'_, Probe>, modifiers: Modifiers, key: Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
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

fn discover(
    harness: &Harness<'_, Probe>,
    selector: InspectionSelector,
) -> Vec<CapabilityDescription> {
    let reply = request(
        harness,
        "discover",
        InspectionOperation::Discover {
            selector,
            limit: 64,
            cursor: None,
        },
    );
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    capabilities
}

fn receipt(reply: InspectionReply) -> InvocationReceipt {
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("receipt failed: {:?}", reply.error);
    };
    receipt
}

/// Admit an invocation, run passes until it has drained, and read its receipt.
fn invoke(
    harness: &mut Harness<'_, Probe>,
    id: &str,
    selector: InspectionSelector,
    expected: ResolvedTarget,
    arguments: serde_json::Value,
) -> InvocationReceipt {
    let admitted = receipt(request(
        harness,
        id,
        InspectionOperation::Invoke {
            selector,
            expected,
            arguments,
        },
    ));
    if admitted.state != ReceiptState::Queued {
        return admitted;
    }
    harness.run();
    receipt(request(
        harness,
        &format!("lookup-{id}"),
        InspectionOperation::Receipt {
            request_id: id.into(),
        },
    ))
}

fn by_id(id: &SemanticUiId) -> InspectionSelector {
    InspectionSelector {
        id: Some(id.clone()),
        ..Default::default()
    }
}

fn by_capability(action: ChatAct) -> InspectionSelector {
    InspectionSelector {
        capability: Some(action.stable_id().into()),
        ..Default::default()
    }
}

/// Set the field's text as `stobie set-text` does.
fn set_text(harness: &mut Harness<'_, Probe>, text: &str) -> InvocationReceipt {
    let field = field_node(harness).id;
    let mut found = discover(harness, by_id(&field));
    assert_eq!(found.len(), 1, "{found:?}");
    let expected = found.remove(0).target;
    invoke(
        harness,
        &format!("set-{text}"),
        by_id(&field),
        expected,
        serde_json::json!({ "value": text }),
    )
}

#[test]
fn the_field_grows_with_its_lines_up_to_its_limit_then_scrolls() {
    let mut harness = harness(Chat::default());
    let one = field_rect(&harness);
    let hit = send_rect(&harness).height();
    assert!(
        one.height() >= hit && one.height() < hit + 1.0,
        "one line fills the controls' height: {one:?}"
    );
    // The controls sit on the field's line, at its height.
    assert!((send_rect(&harness).bottom() - one.bottom()).abs() < 0.5);
    // The icon-only Send and Attach controls take a whole touch target.
    let touch = host(&harness)
        .preferences
        .tokens(true)
        .touch_target_size()
        .0;
    let leading = view(&harness).leading.as_ref().unwrap().rect;
    for rect in [send_rect(&harness), leading] {
        assert!(
            rect.width() >= touch && rect.height() >= touch,
            "an icon control is a touch target: {rect:?}"
        );
    }

    focus_field(&mut harness);
    type_text(&mut harness, "one");
    let mut heights = vec![field_rect(&harness).height()];
    for line in ["two", "three", "four", "five"] {
        press(&mut harness, Modifiers::SHIFT, Key::Enter);
        type_text(&mut harness, line);
        heights.push(field_rect(&harness).height());
    }
    assert_eq!(app(&harness).draft, "one\ntwo\nthree\nfour\nfive");
    assert!(app(&harness).sent.is_empty(), "Shift+Return never sends");
    assert_eq!(heights[0], one.height(), "one line keeps its height");
    assert!(heights[1] > heights[0], "a second line grows: {heights:?}");
    assert!(heights[2] > heights[1], "a third line grows: {heights:?}");
    // The limit is three lines; more scroll within it.
    assert_eq!(
        heights[3], heights[2],
        "the limit stops growth: {heights:?}"
    );
    assert_eq!(
        heights[4], heights[3],
        "the limit stops growth: {heights:?}"
    );
    // The controls follow the field's bottom; the leading control too.
    let field = field_rect(&harness);
    assert!((send_rect(&harness).bottom() - field.bottom()).abs() < 0.5);
    let leading = view(&harness).leading.as_ref().unwrap().rect;
    assert!((leading.bottom() - field.bottom()).abs() < 0.5);
    assert!(leading.right() <= field.left() && field.right() <= send_rect(&harness).left());
    // The observed field is the part shown, and holds the whole draft.
    let node = field_node(&harness);
    assert!(
        (node.rect.max_y - node.rect.min_y - field.height()).abs() <= 1.0,
        "{node:?}"
    );
    assert!(node.focused);
    assert_eq!(node.name, "Message");

    // Sending returns the field to one line.
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(app(&harness).sent, ["one\ntwo\nthree\nfour\nfive"]);
    assert_eq!(field_rect(&harness).height(), one.height());
}

#[test]
fn return_sends_the_draft_and_keeps_the_fields_focus() {
    let mut harness = harness(Chat::default());
    focus_field(&mut harness);
    type_text(&mut harness, "Hello");
    assert_eq!(app(&harness).draft, "Hello");
    assert_eq!(
        send_nodes(&harness)[0].argument,
        Some(serde_json::json!("Hello"))
    );

    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(app(&harness).sent, ["Hello"]);
    assert_eq!(app(&harness).draft, "", "the application cleared its draft");
    assert!(field_focused(&harness), "the field keeps focus to type on");
    assert!(
        app(&harness).errors.is_empty(),
        "{:?}",
        app(&harness).errors
    );

    // Typing continues straight away; Cmd or Ctrl+Return also sends.
    type_text(&mut harness, "Again");
    press(&mut harness, Modifiers::COMMAND, Key::Enter);
    assert_eq!(app(&harness).sent, ["Hello", "Again"]);
    assert!(field_focused(&harness));
}

#[test]
fn shift_return_types_a_newline_without_sending() {
    let mut harness = harness(Chat::default());
    focus_field(&mut harness);
    type_text(&mut harness, "first");
    press(&mut harness, Modifiers::SHIFT, Key::Enter);
    type_text(&mut harness, "second");
    assert_eq!(app(&harness).draft, "first\nsecond");
    assert!(app(&harness).sent.is_empty());
    assert!(view(&harness).submitted.is_empty());

    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(app(&harness).sent, ["first\nsecond"]);
}

#[test]
fn a_labelled_control_shares_the_centre_line_of_the_icon_controls() {
    let harness = harness(Chat::default());
    let centre = |label: &str| harness.get_by_label(label).rect().center().y;
    let dictate = harness.get_by_label("Dictate").rect();
    // The icon-only Send takes the touch target and the labelled Dictate the
    // control height, centred on Send's and Attach's line (within the
    // fraction of a point by which the field's measured line height places
    // the controls after it).
    assert!(send_rect(&harness).height() > dictate.height());
    assert!((centre("Dictate") - centre("Send")).abs() < 0.5);
    assert!((centre("Attach") - centre("Send")).abs() < 0.5);
}

#[test]
fn the_send_control_sends_and_the_field_keeps_focus() {
    let mut harness = harness(Chat::default());
    focus_field(&mut harness);
    type_text(&mut harness, "By the button");
    harness.get_by_label("Send").click();
    harness.run();
    assert_eq!(app(&harness).sent, ["By the button"]);
    assert_eq!(app(&harness).draft, "");
    assert!(
        field_focused(&harness),
        "pressing send keeps the field's focus"
    );

    // Without focus first, a send still leaves the field focused.
    harness.ctx.memory_mut(|memory| memory.stop_text_input());
    harness.run();
    assert!(!field_focused(&harness));
    host_mut(&mut harness).app_mut().draft = "Unfocused".into();
    harness.run();
    harness.get_by_label("Send").click();
    harness.run();
    assert_eq!(app(&harness).sent, ["By the button", "Unfocused"]);
    assert!(field_focused(&harness));
}

#[test]
fn an_empty_draft_does_not_send() {
    let mut harness = harness(Chat::default());
    let send = send_nodes(&harness);
    assert_eq!(send.len(), 1, "send shows, disabled");
    assert!(!send[0].enabled);
    assert_eq!(
        send[0].disabled_reason.as_deref(),
        Some("Type a message first")
    );
    assert_eq!(send[0].argument, Some(serde_json::json!("")));

    focus_field(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    type_text(&mut harness, "   ");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness.get_by_label("Send").click();
    harness.run();
    assert!(app(&harness).sent.is_empty());
    assert!(view(&harness).submitted.is_empty());
    assert_eq!(app(&harness).draft, "   ", "Return types no newline either");
    assert!(
        app(&harness).errors.is_empty(),
        "{:?}",
        app(&harness).errors
    );
    assert!(field_focused(&harness));
}

#[test]
fn a_disabled_send_neither_sends_nor_types() {
    let mut harness = harness(Chat {
        sending: true,
        ..Chat::default()
    });
    focus_field(&mut harness);
    type_text(&mut harness, "Wait for it");
    let send = send_nodes(&harness);
    assert!(!send[0].enabled);
    assert_eq!(
        send[0].disabled_reason.as_deref(),
        Some("The last message is still sending")
    );
    press(&mut harness, Modifiers::NONE, Key::Enter);
    harness.get_by_label("Send").click();
    harness.run();
    assert!(app(&harness).sent.is_empty());
    assert_eq!(app(&harness).draft, "Wait for it");

    // A draft over the action's own limit is refused before the application.
    host_mut(&mut harness).app_mut().sending = false;
    host_mut(&mut harness).app_mut().draft = "x".repeat(41);
    harness.run();
    let send = send_nodes(&harness);
    assert!(!send[0].enabled);
    assert!(send[0].disabled_reason.is_some(), "{send:?}");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert!(app(&harness).sent.is_empty());

    host_mut(&mut harness).app_mut().draft = "Sent at last".into();
    harness.run();
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(app(&harness).sent, ["Sent at last"]);
}

#[test]
fn a_refused_send_keeps_the_draft() {
    let mut harness = harness(Chat {
        refuse: true,
        ..Chat::default()
    });
    focus_field(&mut harness);
    type_text(&mut harness, "Over the limit");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert!(app(&harness).sent.is_empty());
    assert_eq!(app(&harness).draft, "Over the limit");
    assert_eq!(app(&harness).errors, ["Daily message limit reached"]);
    assert_eq!(
        field_node(&harness).value.as_deref(),
        Some("Over the limit")
    );
    assert!(field_focused(&harness));

    host_mut(&mut harness).app_mut().refuse = false;
    harness.get_by_label("Send").click();
    harness.run();
    assert_eq!(app(&harness).sent, ["Over the limit"]);
}

#[test]
fn hidden_controls_present_nothing() {
    let mut harness = harness(Chat {
        mic_mode: true,
        hide_attach: true,
        ..Chat::default()
    });
    // An empty draft shows the microphone alone, and no attach.
    assert!(harness.query_by_label("Send").is_none());
    assert!(harness.query_by_label("Attach").is_none());
    assert!(harness.query_by_label("Dictate").is_some());
    assert!(send_nodes(&harness).is_empty());
    assert!(
        host(&harness)
            .ui_snapshot()
            .by_action(ChatAct::Attach)
            .next()
            .is_none()
    );
    assert_eq!(view(&harness).send.as_ref().unwrap().rect, Rect::NOTHING);
    assert_eq!(view(&harness).leading.as_ref().unwrap().rect, Rect::NOTHING);
    // The field takes the hidden controls' room.
    let field = field_rect(&harness);
    let dictate = view(&harness).trailing.as_ref().unwrap().rect;
    assert!(field.left() < 20.0, "{field:?}");
    assert!(dictate.right() > 570.0, "{dictate:?}");
    // Return on a hidden send does nothing.
    focus_field(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert!(app(&harness).sent.is_empty());
    assert_eq!(app(&harness).draft, "");

    // Typing shows send beside the microphone, and it sends.
    type_text(&mut harness, "Spoken later");
    assert!(harness.query_by_label("Send").is_some());
    assert!(field_rect(&harness).right() < dictate.left());
    harness.get_by_label("Dictate").click();
    harness.run();
    assert_eq!(app(&harness).dictated, 1);
    focus_field(&mut harness);
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(app(&harness).sent, ["Spoken later"]);
    assert!(harness.query_by_label("Send").is_none());

    // The leading control performs its own action when shown.
    host_mut(&mut harness).app_mut().hide_attach = false;
    harness.run();
    harness.get_by_label("Attach").click();
    harness.run();
    assert_eq!(app(&harness).attached, 1);
}

#[test]
fn agents_set_the_text_and_send_it() {
    let mut harness = harness(Chat::default());
    let set = set_text(&mut harness, "From an agent");
    assert_eq!(set.state, ReceiptState::Completed, "{set:?}");
    assert_eq!(app(&harness).draft, "From an agent");
    assert_eq!(field_node(&harness).value.as_deref(), Some("From an agent"));

    // The send control carries the draft; its ID sends it with no value.
    let send = send_nodes(&harness).remove(0);
    assert_eq!(send.argument, Some(serde_json::json!("From an agent")));
    let mut bindings = discover(&harness, by_id(&send.id));
    assert_eq!(bindings.len(), 1, "{bindings:?}");
    let binding = bindings.remove(0);
    let sent = invoke(
        &mut harness,
        "send-by-id",
        by_id(&send.id),
        binding.target,
        serde_json::Value::Null,
    );
    assert_eq!(sent.state, ReceiptState::Completed, "{sent:?}");
    assert_eq!(app(&harness).sent, ["From an agent"]);
    assert_eq!(app(&harness).draft, "");

    // Or the action, with the draft as its value, chooses the control.
    set_text(&mut harness, "Second");
    let found = discover(&harness, by_capability(ChatAct::Send));
    let control = found
        .iter()
        .find(|capability| capability.argument == Some(serde_json::json!("Second")))
        .expect("the send control carries the draft");
    let sent = invoke(
        &mut harness,
        "send-by-value",
        by_capability(ChatAct::Send),
        control.target.clone(),
        serde_json::json!({ "value": "Second" }),
    );
    assert_eq!(sent.state, ReceiptState::Completed, "{sent:?}");
    assert_eq!(
        sent.target.as_ref().map(|target| &target.id),
        Some(&control.id)
    );
    assert_eq!(app(&harness).sent, ["From an agent", "Second"]);

    // A refused send reports the application's error and keeps the draft.
    host_mut(&mut harness).app_mut().refuse = true;
    set_text(&mut harness, "Refused");
    let send = send_nodes(&harness).remove(0);
    let binding = discover(&harness, by_id(&send.id)).remove(0);
    let refused = invoke(
        &mut harness,
        "send-refused",
        by_id(&send.id),
        binding.target,
        serde_json::Value::Null,
    );
    assert_ne!(refused.state, ReceiptState::Completed, "{refused:?}");
    assert_eq!(
        refused.error.as_ref().map(|error| error.message.as_str()),
        Some("Daily message limit reached")
    );
    assert_eq!(app(&harness).draft, "Refused");
    assert_eq!(app(&harness).sent.len(), 2);

    // An empty draft's send is refused as unavailable.
    host_mut(&mut harness).app_mut().refuse = false;
    set_text(&mut harness, "");
    let send = send_nodes(&harness).remove(0);
    let binding = discover(&harness, by_id(&send.id)).remove(0);
    let empty = invoke(
        &mut harness,
        "send-empty",
        by_id(&send.id),
        binding.target,
        serde_json::Value::Null,
    );
    assert_ne!(empty.state, ReceiptState::Completed, "{empty:?}");
    assert_eq!(app(&harness).sent.len(), 2);
}

#[test]
fn hidden_actions_are_not_drawn_or_observed() {
    // Frame::action, Frame::icon_action, Frame::toggle and
    // PresentationContext::action with a hidden argument-less action: a
    // debug build used to panic, and a release build drew a disabled button.
    let mut harness = harness(Chat {
        plain_controls: true,
        plain_hidden: true,
        ..Chat::default()
    });
    assert!(harness.query_by_label("Archive").is_none());
    assert!(harness.query_by_label("Mute").is_none());
    let snapshot = host(&harness).ui_snapshot();
    assert!(snapshot.by_action(ChatAct::Archive).next().is_none());
    assert!(snapshot.by_action(ChatAct::Mute).next().is_none());
    let hidden_text = snapshot.text.len();
    // The composer below lays out from the top: hidden controls take no room.
    assert!(
        field_rect(&harness).top() < 20.0,
        "{:?}",
        field_rect(&harness)
    );

    // An agent cannot invoke a hidden action.
    let found = discover(&harness, by_capability(ChatAct::Archive));
    assert_eq!(found.len(), 1, "the action's own binding: {found:?}");
    let binding = found[0].clone();
    // It is discoverable, not rendered, and refused as unavailable.
    assert!(!binding.rendered, "{binding:?}");
    let refused = invoke(
        &mut harness,
        "archive-hidden",
        by_capability(ChatAct::Archive),
        binding.target.clone(),
        serde_json::Value::Null,
    );
    assert_eq!(refused.state, ReceiptState::Rejected, "{refused:?}");
    assert_eq!(
        refused.error.map(|error| error.code),
        Some(InspectionErrorCode::Unavailable)
    );

    // Shown again, each control is drawn and observed.
    host_mut(&mut harness).app_mut().plain_hidden = false;
    harness.run();
    assert_eq!(harness.query_all_by_label("Archive").count(), 2);
    assert_eq!(harness.query_all_by_label("Mute").count(), 2);
    let snapshot = host(&harness).ui_snapshot();
    assert_eq!(snapshot.by_action(ChatAct::Archive).count(), 2);
    assert_eq!(snapshot.by_action(ChatAct::Mute).count(), 2);
    assert!(field_rect(&harness).top() > 40.0);
    // Archive, the toggle and the direct Archive measure their labels.
    assert!(host(&harness).ui_snapshot().text.len() >= hidden_text + 3);
}

/// The AccessKit tree as kittest holds it, for the parity audit.
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

#[test]
fn a_composer_with_an_id_names_its_field_and_controls_by_it() {
    // Without one, the field's ID derives from the scope and the key.
    let harness = harness(Chat::default());
    let derived = field_node(&harness);
    assert!(
        derived.id.0.starts_with("presentation."),
        "{:?}",
        derived.id
    );

    let mut harness = self::harness(Chat {
        id: Some("chat.composer"),
        ..Chat::default()
    });
    let field = field_node(&harness);
    assert_eq!(field.id, SemanticUiId::new("chat.composer"));
    assert_eq!(
        (field.name.as_str(), field.role),
        ("Message", UiRole::MultilineTextInput)
    );
    // It lays out as the derived composer does.
    assert_eq!(field.rect, derived.rect);
    let snapshot = host(&harness).ui_snapshot();
    for (action, id) in [
        (ChatAct::Attach, "chat.composer.leading"),
        (ChatAct::Dictate, "chat.composer.trailing"),
        (ChatAct::Send, "chat.composer.send"),
    ] {
        let ids: Vec<_> = snapshot
            .by_action(action)
            .map(|node| node.id.0.as_str())
            .collect();
        assert_eq!(ids, [id], "{action:?}");
    }
    // AccessKit gives the field the same name and ID.
    let mismatches = crate::audit_accesskit(snapshot, &accesskit_update(&harness));
    assert!(mismatches.is_empty(), "{mismatches:?}");
    let (_, node) = accesskit_update(&harness)
        .nodes
        .into_iter()
        .find(|(_, node)| node.author_id() == Some("chat.composer"))
        .expect("the field is in the AccessKit tree");
    assert_eq!(
        (node.role(), node.label()),
        (egui::accesskit::Role::MultilineTextInput, Some("Message"))
    );

    // An agent sets the text by the field's ID and sends it by the send
    // control's.
    let set = set_text(&mut harness, "Named");
    assert_eq!(set.state, ReceiptState::Completed, "{set:?}");
    assert_eq!(set.target.map(|target| target.id), Some(field.id));
    let send = SemanticUiId::new("chat.composer.send");
    let mut bindings = discover(&harness, by_id(&send));
    assert_eq!(bindings.len(), 1, "{bindings:?}");
    let binding = bindings.remove(0);
    assert_eq!(binding.argument, Some(serde_json::json!("Named")));
    let sent = invoke(
        &mut harness,
        "send-named",
        by_id(&send),
        binding.target,
        serde_json::Value::Null,
    );
    assert_eq!(sent.state, ReceiptState::Completed, "{sent:?}");
    assert_eq!(sent.target.map(|target| target.id), Some(send));
    assert_eq!(app(&harness).sent, ["Named"]);
}
