use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionKey, ActionScope, ActionSpec, ActionTarget, ArgumentContract,
    Availability, AvatarFace, AvatarOutline, AvatarSize, AvatarSpec, ConversationRowSpec,
    DesignTokens, MarkdownSpec, MessageBubbleSpec, MessageSide, PresentationContext,
    PresentationScope, ReadingTextSpec, SemanticUiId, SwitchSpec, TextLayoutObservation,
    observe_visible_only,
};

use super::StoryReport;
use crate::catalogue::StoryId;

/// Consumer-owned chat state: the conversation on show, which ones have
/// been read, and the settings' switches.
#[derive(Clone, Debug, Serialize)]
pub struct ChatFixtureState {
    pub open: String,
    pub read: Vec<String>,
    pub notifications: bool,
    pub pinned: bool,
    pub opened: u32,
    pub toggled: u32,
}

impl Default for ChatFixtureState {
    fn default() -> Self {
        Self {
            open: "ada".into(),
            read: Vec::new(),
            notifications: true,
            pinned: false,
            opened: 0,
            toggled: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum ChatAction {
    Open,
    Notifications,
    Pin,
    Archive,
}

impl ActionKey for ChatAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Open => "gallery.conversation.open",
            Self::Notifications => "gallery.settings.notifications",
            Self::Pin => "gallery.conversation.pin",
            Self::Archive => "gallery.settings.archive",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: match self {
                Self::Open => "Open",
                Self::Notifications => "Notifications",
                Self::Pin => "Keep at the top",
                Self::Archive => "Archive old conversations",
            },
            description: match self {
                Self::Open => "Open the conversation with this ID",
                Self::Notifications => "Turn notifications for new messages on or off",
                Self::Pin => "Keep this conversation at the top of the list",
                Self::Archive => "Archive conversations without a message for a month",
            },
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
    fn argument(self) -> ArgumentContract {
        match self {
            Self::Open => ArgumentContract::Text {
                max_length: Some(16),
            },
            Self::Notifications | Self::Pin | Self::Archive => ArgumentContract::None,
        }
    }
}

/// How a conversation's avatar looks: a person's initials or a shape.
#[derive(Clone, Copy)]
enum Figure {
    Person,
    Shape(egui::Color32),
}

struct Conversation {
    id: &'static str,
    title: &'static str,
    figure: Figure,
    preview: &'static str,
    time: &'static str,
    /// Unread messages until the conversation is opened.
    unread: u32,
    archived: bool,
}

const CONVERSATIONS: [Conversation; 4] = [
    Conversation {
        id: "ada",
        title: "Ada Lovelace",
        figure: Figure::Person,
        preview: "The notes on the engine are ready",
        time: "09:41",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "helper",
        title: "Kitchen helper",
        figure: Figure::Shape(egui::Color32::from_rgb(96, 196, 150)),
        preview: "Three dinners that use the leftover rice",
        time: "08:15",
        unread: 2,
        archived: false,
    },
    Conversation {
        id: "grace",
        title: "Grace Hopper",
        figure: Figure::Person,
        preview: "The hall is booked for Friday",
        time: "Mon",
        unread: 1,
        archived: false,
    },
    Conversation {
        id: "edsger",
        title: "Edsger Dijkstra",
        figure: Figure::Person,
        preview: "Shortest route found",
        time: "12 Oct",
        unread: 0,
        archived: false,
    },
];

const LONG_CONVERSATIONS: [Conversation; 4] = [
    Conversation {
        id: "augusta",
        title: "Augusta Ada King, Countess of Lovelace",
        figure: Figure::Person,
        preview: "Die Anmerkungen zur Analytischen Maschine sind fertig und liegen bei",
        time: "gestern",
        unread: 3,
        archived: false,
    },
    Conversation {
        id: "planner",
        title: "Weekend planner for the whole household",
        figure: Figure::Shape(egui::Color32::from_rgb(92, 158, 238)),
        preview: "Saturday: market at nine, swimming at eleven, then lunch",
        time: "23:59",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "katherine",
        title: "Katherine Coleman Goble Johnson",
        figure: Figure::Person,
        preview: "Trajectory checked by hand",
        time: "1 Sep 2026",
        unread: 0,
        archived: true,
    },
    Conversation {
        id: "grace-long",
        title: "Grace Brewster Murray Hopper",
        figure: Figure::Person,
        preview: "",
        time: "Mon",
        unread: 1,
        archived: false,
    },
];

/// One message: who sent it and its Markdown.
struct Message {
    id: u32,
    side: MessageSide,
    markdown: &'static str,
}

const THREAD: [Message; 4] = [
    Message {
        id: 1,
        side: MessageSide::Other,
        markdown: "The notes on the engine are ready. **Note G** has the table of \
                   Bernoulli numbers.",
    },
    Message {
        id: 2,
        side: MessageSide::Own,
        markdown: "Thank you!",
    },
    Message {
        id: 3,
        side: MessageSide::Own,
        markdown: "Could you send the table as well, so I can check the third and \
                   fifth numbers against my own working before Friday?",
    },
    Message {
        id: 4,
        side: MessageSide::Other,
        markdown: "Of course:\n\n- B1 is 1/6\n- B3 is 1/30\n\nThe full table is in \
                   [the published notes](https://example.org/notes).",
    },
];

const LONG_THREAD: [Message; 4] = [
    Message {
        id: 11,
        side: MessageSide::Own,
        markdown: "Wo liegen die Anmerkungen zur Analytischen Maschine, und wann \
                   können wir sie gemeinsam durchgehen?",
    },
    Message {
        id: 12,
        side: MessageSide::Other,
        markdown: "In the shared folder: \
                   https://example.org/shared/household/notes/analytical-engine/2026/october/final",
    },
    Message {
        id: 13,
        side: MessageSide::Other,
        markdown: "```\nfor n in 1..=7 { table.push(bernoulli(n)); }\n```",
    },
    Message {
        id: 14,
        side: MessageSide::Own,
        markdown: "OK",
    },
];

/// A conversation list, a conversation's messages and its settings. The
/// long story scrolls, so it publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut ChatFixtureState,
    report: &mut StoryReport,
) {
    let long = story == StoryId::ChatLongNarrow;
    let (conversations, thread): (&[Conversation], &[Message]) = if long {
        (&LONG_CONVERSATIONS, &LONG_THREAD)
    } else {
        (&CONVERSATIONS, &THREAD)
    };
    if !conversations
        .iter()
        .any(|conversation| conversation.id == state.open)
    {
        state.open = conversations[0].id.into();
    }
    observe_visible_only(ui, |ui| {
        let mut p = PresentationContext::new(
            ui,
            *tokens,
            scale,
            PresentationScope::new("gallery.chat"),
            SemanticUiId::new("gallery.story"),
        );
        egui::ScrollArea::vertical()
            .id_salt("chat-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let wide = ui.available_width() >= 720.0 * scale;
                if wide {
                    // Desktop: the list beside the conversation.
                    ui.horizontal_top(|ui| {
                        let list = 280.0 * scale;
                        ui.allocate_ui_with_layout(
                            egui::vec2(list, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_width(list);
                                list_and_settings(ui, &mut p, conversations, long, state, report);
                            },
                        );
                        ui.separator();
                        ui.vertical(|ui| messages(ui, &mut p, thread));
                    });
                } else {
                    // Phone: the list, then the conversation.
                    list_and_settings(ui, &mut p, conversations, long, state, report);
                    ui.separator();
                    messages(ui, &mut p, thread);
                }
            });
        p.retain_visible_text(ui, fully_visible_text);
        report.presented(&p.finish(ui));
    });
}

fn fully_visible_text(layout: &TextLayoutObservation) -> bool {
    let a = layout.allocated_rect;
    let c = layout.clip_rect;
    a.min_x >= c.min_x && a.min_y >= c.min_y && a.max_x <= c.max_x && a.max_y <= c.max_y
}

fn avatar(conversation: &Conversation) -> AvatarSpec<'static> {
    match conversation.figure {
        Figure::Person => AvatarSpec::person(conversation.title, AvatarSize::List),
        Figure::Shape(fill) => AvatarSpec::shape(
            AvatarOutline::ROUNDED_SQUARE,
            fill,
            AvatarFace::EYES,
            AvatarSize::List,
        ),
    }
}

/// The conversation rows and the switches beneath them.
fn list_and_settings(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    conversations: &[Conversation],
    long: bool,
    state: &mut ChatFixtureState,
    report: &mut StoryReport,
) {
    for conversation in conversations {
        let unread_words = match conversation.unread {
            1 => "1 unread message".to_owned(),
            count => format!("{count} unread messages"),
        };
        let unread = conversation.unread > 0 && !state.read.iter().any(|id| id == conversation.id);
        let availability = if conversation.archived {
            Availability::Disabled {
                reason: "This conversation is archived".into(),
            }
        } else {
            Availability::Enabled
        };
        let opened = p
            .conversation_row(
                ui,
                ChatAction::Open,
                ConversationRowSpec {
                    title: conversation.title,
                    preview: (!conversation.preview.is_empty()).then_some(conversation.preview),
                    time: Some(conversation.time),
                    unread: unread.then_some(unread_words.as_str()),
                    selected: state.open == conversation.id,
                    avatar: Some(avatar(conversation)),
                },
                ActionTarget::application(ChatAction::Open),
                &availability,
                &ActionArgument::from(conversation.id),
            )
            .clicked();
        if opened {
            state.open = conversation.id.into();
            if !state.read.iter().any(|id| id == conversation.id) {
                state.read.push(conversation.id.into());
            }
            state.opened += 1;
            report.repaint = Some("conversation opened");
        }
    }
    ui.add_space(p.tokens().spacing.section.0);
    let none = ActionArgument::None;
    if p.switch(
        ui,
        ChatAction::Notifications,
        SwitchSpec {
            label: "Notifications",
            on: state.notifications,
        },
        ActionTarget::application(ChatAction::Notifications),
        &Availability::Enabled,
        &none,
    )
    .clicked()
    {
        state.notifications = !state.notifications;
        state.toggled += 1;
        report.repaint = Some("setting changed");
    }
    let pin_label = if long {
        "Keep this conversation at the top of the list, above newer ones"
    } else {
        "Keep at the top"
    };
    if p.switch(
        ui,
        ChatAction::Pin,
        SwitchSpec {
            label: pin_label,
            on: state.pinned,
        },
        ActionTarget::application(ChatAction::Pin),
        &Availability::Enabled,
        &none,
    )
    .clicked()
    {
        state.pinned = !state.pinned;
        state.toggled += 1;
        report.repaint = Some("setting changed");
    }
    if long {
        p.switch(
            ui,
            ChatAction::Archive,
            SwitchSpec {
                label: "Archive old conversations",
                on: true,
            },
            ActionTarget::application(ChatAction::Archive),
            &Availability::Disabled {
                reason: "The household's administrator sets this".into(),
            },
            &none,
        );
    }
}

/// The open conversation's messages, each in its bubble.
fn messages(ui: &mut egui::Ui, p: &mut PresentationContext, thread: &[Message]) {
    for message in thread {
        let sender = match message.side {
            MessageSide::Own => "You",
            MessageSide::Other => "Ada Lovelace",
        };
        p.message_bubble(
            ui,
            ("message", message.id),
            MessageBubbleSpec {
                side: message.side,
                sender,
            },
            |ui, p| {
                if message.markdown.contains(['*', '[', '`', '-']) {
                    p.markdown(ui, "text", message.markdown, MarkdownSpec::default());
                } else {
                    p.reading_text(ui, "text", message.markdown, ReadingTextSpec::default());
                }
            },
        );
        ui.add_space(p.tokens().spacing.inline.0);
    }
}
