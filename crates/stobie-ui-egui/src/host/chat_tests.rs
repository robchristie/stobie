#![cfg(test)]
//! Chat recipes in a hosted application: conversation rows carry their
//! conversation through clicks, semantics and invocation, a switch states
//! its setting, and message bubbles hold their content on either side.

use egui::Vec2;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use serde::Serialize;

use super::*;
use crate::{
    Action, BoundAction, ConversationRowSpec, InspectionOperation, InspectionResult,
    InspectionSelector, MarkdownSpec, MessageBubbleSpec, MessageSide, PresentationScope,
    ReadingTextSpec, ReceiptState, SemanticActionId, SwitchSpec, TextComponentKind,
};
use crate::{DensityVariant, ThemeVariant};

#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "chat")]
enum ChatAct {
    #[action(
        label = "Open",
        description = "Open the conversation with this ID",
        argument = "text",
        max_length = 8
    )]
    Open,
    #[action(label = "Notifications", description = "Turn notifications on or off")]
    Notifications,
}

enum Intent {}

struct Conversation {
    id: &'static str,
    title: &'static str,
    preview: &'static str,
    time: &'static str,
    unread: u32,
}

struct Message {
    id: u32,
    side: MessageSide,
    text: String,
}

struct Chat {
    conversations: Vec<Conversation>,
    open: &'static str,
    notifications: bool,
    messages: Vec<Message>,
    performed: Vec<(ChatAct, ActionArgument)>,
    errors: Vec<String>,
    preferences: crate::UiPreferences,
}

const LONG: &str = "The hall is booked for Friday from six until late, the key is at the \
     front desk, and the caretaker asks that the last person out turns off the \
     heaters and locks the side door.";

impl Default for Chat {
    fn default() -> Self {
        Self {
            conversations: vec![
                Conversation {
                    id: "c1",
                    title: "Ada",
                    preview: "See you at six",
                    time: "09:41",
                    unread: 0,
                },
                Conversation {
                    id: "c2",
                    title: "Grace",
                    preview: "The hall is booked",
                    time: "Mon",
                    unread: 2,
                },
            ],
            open: "c1",
            notifications: false,
            messages: vec![
                Message {
                    id: 1,
                    side: MessageSide::Own,
                    text: "Is the hall booked?".into(),
                },
                Message {
                    id: 2,
                    side: MessageSide::Other,
                    text: format!("**Yes.** {LONG}"),
                },
            ],
            performed: Vec::new(),
            errors: Vec::new(),
            preferences: crate::UiPreferences::default(),
        }
    }
}

impl Chat {
    fn conversation(&self, argument: &ActionArgument) -> Option<&Conversation> {
        let ActionArgument::Text(id) = argument else {
            return None;
        };
        self.conversations
            .iter()
            .find(|conversation| conversation.id == id.as_str())
    }
}

impl Application for Chat {
    type Action = ChatAct;
    type Intent = Intent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        crate::application_identity!("Chat")
    }

    fn preferences(&self) -> crate::UiPreferences {
        self.preferences
    }

    fn availability(&self, _target: ActionTarget<ChatAct>) -> Availability {
        Availability::Enabled
    }

    fn availability_with(
        &self,
        _target: ActionTarget<ChatAct>,
        argument: &ActionArgument,
    ) -> Availability {
        match self.conversation(argument) {
            Some(_) => Availability::Enabled,
            None => Availability::Hidden,
        }
    }

    fn ui(&self, ui: &mut Ui, _view: &mut (), frame: &mut Frame<'_, ChatAct, Intent>) {
        let mut p = frame.presentation(ui, PresentationScope::new("chat"), SemanticUiId::root());
        let unread: Vec<String> = self
            .conversations
            .iter()
            .map(|conversation| format!("{} unread messages", conversation.unread))
            .collect();
        for (conversation, unread) in self.conversations.iter().zip(&unread) {
            frame.conversation_row(
                ui,
                &mut p,
                BoundAction::new(ChatAct::Open, conversation.id),
                ConversationRowSpec {
                    title: conversation.title,
                    preview: Some(conversation.preview),
                    time: Some(conversation.time),
                    unread: (conversation.unread > 0).then_some(unread.as_str()),
                    selected: conversation.id == self.open,
                    avatar: None,
                },
            );
        }
        frame.switch(
            ui,
            &mut p,
            ChatAct::Notifications,
            SwitchSpec {
                label: "Notifications",
                on: self.notifications,
            },
        );
        for message in &self.messages {
            let sender = match message.side {
                MessageSide::Own => "You",
                MessageSide::Other => "Grace Hopper",
            };
            p.message_bubble(
                ui,
                ("message", message.id),
                MessageBubbleSpec {
                    side: message.side,
                    sender,
                },
                |ui, p| match message.side {
                    MessageSide::Own => {
                        p.reading_text(ui, "text", &message.text, ReadingTextSpec::default());
                    }
                    MessageSide::Other => {
                        p.markdown(ui, "text", &message.text, MarkdownSpec::default());
                    }
                },
            );
        }
    }

    fn apply(&mut self, intent: Intent, _view: &mut ()) -> Result<bool, String> {
        match intent {}
    }

    fn perform(&mut self, target: ActionTarget<ChatAct>, _view: &mut ()) -> Result<bool, String> {
        match target.action {
            ChatAct::Notifications => {
                self.notifications = !self.notifications;
                self.performed
                    .push((ChatAct::Notifications, ActionArgument::None));
                Ok(true)
            }
            ChatAct::Open => Err("Open needs a conversation".into()),
        }
    }

    fn perform_with(
        &mut self,
        target: ActionTarget<ChatAct>,
        argument: ActionArgument,
        view: &mut (),
    ) -> Result<bool, String> {
        let Some(id) = self
            .conversation(&argument)
            .map(|conversation| conversation.id)
        else {
            return self.perform(target, view);
        };
        self.performed.push((target.action, argument));
        self.open = id;
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

fn harness(chat: Chat, width: f32) -> Harness<'static, Probe> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(width, 900.0))
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
    harness.state_mut().host = Some(AppHost::new(&context, chat));
    harness.run();
    harness
}

fn host<'a>(harness: &'a Harness<'_, Probe>) -> &'a AppHost<Chat> {
    harness.state().host.as_ref().unwrap()
}

fn host_mut<'a>(harness: &'a mut Harness<'_, Probe>) -> &'a mut AppHost<Chat> {
    harness.state_mut().host.as_mut().unwrap()
}

/// The AccessKit tree as an update, for the parity audit.
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

#[test]
fn rows_state_each_conversation_and_open_their_own() {
    for width in [360.0, 1024.0] {
        let mut harness = harness(Chat::default(), width);
        assert_clean(&harness);
        let snapshot = host(&harness).ui_snapshot();
        let rows: Vec<_> = snapshot.by_action(ChatAct::Open).collect();
        assert_eq!(
            rows.iter()
                .map(|row| (
                    row.role,
                    row.name.as_str(),
                    row.description.as_deref(),
                    row.selected,
                    row.argument.clone()
                ))
                .collect::<Vec<_>>(),
            [
                (
                    UiRole::NavigationItem,
                    "Ada",
                    Some("See you at six; 09:41; Open the conversation with this ID"),
                    true,
                    Some(serde_json::json!("c1"))
                ),
                (
                    UiRole::NavigationItem,
                    "Grace",
                    Some(
                        "2 unread messages; The hall is booked; Mon; \
                         Open the conversation with this ID"
                    ),
                    false,
                    Some(serde_json::json!("c2"))
                ),
            ]
        );
        // The row's text is observed under the row, each line by its kind.
        let kinds: Vec<_> = snapshot
            .text
            .iter()
            .filter(|text| {
                text.parent_id.map(|parent| parent.kind) == Some(TextComponentKind::ConversationRow)
            })
            .map(|text| text.component_id.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                TextComponentKind::ConversationRowTime,
                TextComponentKind::ConversationRowTitle,
                TextComponentKind::ConversationRowPreview,
            ]
            .repeat(2)
        );
        // A click opens that conversation, which becomes the current one.
        harness.get_by_label("Grace").click();
        harness.run();
        let chat = host(&harness).app();
        assert_eq!(chat.open, "c2");
        assert_eq!(
            chat.performed,
            [(ChatAct::Open, ActionArgument::Text("c2".into()))]
        );
        assert!(chat.errors.is_empty(), "{:?}", chat.errors);
        let selected: Vec<_> = host(&harness)
            .ui_snapshot()
            .by_action(ChatAct::Open)
            .map(|row| row.selected)
            .collect();
        assert_eq!(selected, [false, true]);
        assert_clean(&harness);
    }
}

#[test]
fn agents_open_a_conversation_by_its_argument() {
    let mut harness = harness(Chat::default(), 1024.0);
    host_mut(&mut harness).enable_inspection(Inspection::new("chat-test"));
    harness.run();
    let inspection = host(&harness).inspection().unwrap();
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: "discover".into(),
        instance: Some(inspection.instance()),
        operation: InspectionOperation::Discover {
            selector: InspectionSelector {
                capability: Some(ChatAct::Open.stable_id().into()),
                argument: Some(serde_json::json!("c2")),
                ..Default::default()
            },
            limit: 8,
            cursor: None,
        },
    });
    let Some(InspectionResult::Discover { capabilities, .. }) = reply.result else {
        panic!("discovery failed: {:?}", reply.error);
    };
    assert_eq!(capabilities.len(), 1);
    assert!(capabilities[0].rendered);
    let reply = inspection.handle(InspectionRequest {
        version: 1,
        request_id: "open".into(),
        instance: Some(inspection.instance()),
        operation: InspectionOperation::Invoke {
            selector: InspectionSelector {
                id: Some(capabilities[0].id.clone()),
                ..Default::default()
            },
            expected: capabilities[0].target.clone(),
            arguments: serde_json::Value::Null,
        },
    });
    let Some(InspectionResult::Receipt { receipt }) = reply.result else {
        panic!("invocation failed: {:?}", reply.error);
    };
    assert_eq!(receipt.state, ReceiptState::Queued, "{receipt:?}");
    harness.run();
    assert_eq!(host(&harness).app().open, "c2");
}

#[test]
fn a_switch_states_its_setting_and_turns_it_over() {
    let mut harness = harness(Chat::default(), 360.0);
    let checked = |harness: &Harness<'_, Probe>| {
        let snapshot = host(harness).ui_snapshot();
        let switches: Vec<_> = snapshot.by_role(UiRole::Switch).collect();
        assert_eq!(switches.len(), 1);
        assert_eq!(switches[0].name, "Notifications");
        assert_eq!(
            switches[0].actions,
            [SemanticActionId::from_action(ChatAct::Notifications)]
        );
        switches[0].checked
    };
    assert_eq!(checked(&harness), Some(false));
    harness.get_by_label("Notifications").click();
    harness.run();
    assert_eq!(checked(&harness), Some(true));
    assert_eq!(
        harness
            .get_by_label("Notifications")
            .accesskit_node()
            .toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    assert_clean(&harness);
    // Space and Enter turn it over too.
    harness.get_by_label("Notifications").focus();
    harness.run();
    harness.key_press(egui::Key::Space);
    harness.run();
    assert_eq!(checked(&harness), Some(false));
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(checked(&harness), Some(true));
    assert_eq!(host(&harness).app().performed.len(), 3);
}

/// The bubble's section node for a sender, and the text inside it.
fn bubble<'a>(snapshot: &'a crate::UiSnapshot, sender: &str) -> &'a UiNode {
    let found: Vec<_> = snapshot
        .by_role(UiRole::Section)
        .filter(|node| node.name == sender)
        .collect();
    assert_eq!(found.len(), 1, "one bubble from {sender}");
    found[0]
}

#[test]
fn bubbles_sit_on_their_sides_and_hug_or_wrap_their_content() {
    for width in [360.0_f32, 1024.0] {
        let mut harness = harness(Chat::default(), width);
        // Later passes keep the fitted width.
        harness.run();
        let snapshot = host(&harness).ui_snapshot().clone();
        assert_clean(&harness);
        let own = bubble(&snapshot, "You");
        let other = bubble(&snapshot, "Grace Hopper");
        let column = snapshot
            .by_action(ChatAct::Open)
            .next()
            .expect("a row spans the column")
            .rect;
        let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
        let maximum = crate::message_bubble_max_width(&tokens, 1.0, column.max_x - column.min_x);
        // The person's own short message hugs its text at the column's end.
        assert!((own.rect.max_x - column.max_x).abs() < 1.0, "{own:?}");
        assert!((own.rect.max_x - own.rect.min_x) < maximum * 0.6, "{own:?}");
        // The long reply starts the line and wraps within the maximum,
        // hugging its widest line.
        assert!((other.rect.min_x - column.min_x).abs() < 1.0, "{other:?}");
        let reply_width = other.rect.max_x - other.rect.min_x;
        assert!(
            reply_width <= maximum + 0.5 && reply_width > maximum * 0.8,
            "{other:?} within {maximum}"
        );
        let inside = |node: &UiNode| {
            snapshot
                .text
                .iter()
                .filter(|text| {
                    text.allocated_rect.min_y >= node.rect.min_y
                        && text.allocated_rect.max_y <= node.rect.max_y + 0.5
                        && text.allocated_rect.min_x >= node.rect.min_x - 0.5
                })
                .collect::<Vec<_>>()
        };
        let reply = inside(other);
        assert!(!reply.is_empty());
        assert!(reply.iter().any(|text| text.line_count > 1));
        for text in reply.iter().chain(inside(own).iter()) {
            assert!(
                text.painted_rect.max_x <= own.rect.max_x.max(other.rect.max_x) + 0.5,
                "{text:?}"
            );
            assert!(!text.truncated);
        }
        // Agents read the messages' words as content; the rows' and
        // switch's text belongs to their semantic nodes.
        let content: Vec<_> = snapshot.content_text().collect();
        assert!(content.contains(&"Is the hall booked?"), "{content:?}");
        assert!(
            content.iter().any(|text| text.starts_with("Yes.")),
            "{content:?}"
        );
        assert!(snapshot.text.iter().all(|text| {
            !matches!(
                text.component_id.kind,
                TextComponentKind::ConversationRowTitle
                    | TextComponentKind::ConversationRowPreview
                    | TextComponentKind::ConversationRowTime
                    | TextComponentKind::SwitchLabel
            ) || text.content.is_none()
        }));
        // AccessKit groups each bubble's content under its sender.
        let group = harness.get_by_label("Grace Hopper");
        assert_eq!(group.accesskit_node().role(), egui::accesskit::Role::Group);
        assert_eq!(
            harness.get_by_label("You").accesskit_node().role(),
            egui::accesskit::Role::Group
        );
    }
}

#[test]
fn a_bubble_fits_again_when_its_text_or_column_changes() {
    let mut harness = harness(Chat::default(), 1024.0);
    harness.run();
    let width = |harness: &Harness<'_, Probe>| {
        let rect = bubble(host(harness).ui_snapshot(), "You").rect;
        rect.max_x - rect.min_x
    };
    let short = width(&harness);
    // Longer text takes the maximum and wraps.
    host_mut(&mut harness).app_mut().messages[0].text = LONG.into();
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    harness.run();
    let long = width(&harness);
    assert!(long > short * 2.0, "{short} then {long}");
    // Shorter text hugs again.
    host_mut(&mut harness).app_mut().messages[0].text = "Thanks".into();
    host_mut(&mut harness).request_repaint("test");
    harness.run();
    harness.run();
    let thanks = width(&harness);
    assert!(thanks < short, "{short} then {thanks}");
    // The fit is stable: another pass keeps it.
    harness.run();
    assert_eq!(width(&harness), thanks);
    // A narrower column caps the long reply at its new maximum.
    harness.set_size(Vec2::new(360.0, 900.0));
    harness.run();
    harness.run();
    let reply = bubble(host(&harness).ui_snapshot(), "Grace Hopper").rect;
    assert!(reply.max_x <= 360.0, "{reply:?}");
    assert_clean(&harness);
}

#[test]
fn a_bubble_fits_again_when_its_text_is_set_differently() {
    let mut harness = harness(Chat::default(), 1024.0);
    harness.run();
    let lines = |harness: &Harness<'_, Probe>| {
        host(harness)
            .ui_snapshot()
            .text
            .iter()
            .find(|text| {
                matches!(&text.content, Some(crate::TextContent::Excerpt { text, .. }) if text == "Is the hall booked?")
            })
            .map(|text| text.line_count)
    };
    assert_eq!(lines(&harness), Some(1));
    // Each density, contrast and text size sets the text afresh, and the
    // short message stays on one line.
    for (density, contrast, font_scale) in [
        (
            crate::DensityPreference::Compact,
            crate::ContrastPreference::Standard,
            1.0,
        ),
        (
            crate::DensityPreference::Comfortable,
            crate::ContrastPreference::High,
            1.0,
        ),
        (
            crate::DensityPreference::Comfortable,
            crate::ContrastPreference::Standard,
            1.5,
        ),
        (
            crate::DensityPreference::Comfortable,
            crate::ContrastPreference::Standard,
            1.0,
        ),
    ] {
        let preferences = &mut host_mut(&mut harness).app_mut().preferences;
        preferences.density = density;
        preferences.contrast = contrast;
        preferences.font_scale = font_scale;
        host_mut(&mut harness).request_repaint("test");
        harness.run();
        harness.run();
        assert_eq!(
            lines(&harness),
            Some(1),
            "{density:?} {contrast:?} {font_scale}"
        );
    }
    assert_clean(&harness);
}
