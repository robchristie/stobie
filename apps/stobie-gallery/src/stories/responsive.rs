use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionKey, ActionScope, ActionSpec, ActionTarget, ArgumentContract,
    Availability, AvatarSize, AvatarSpec, ConversationRowSpec, DesignTokens, DomainReference,
    IconId, MessageBubbleSpec, MessageSide, NavigationBadge, NavigationContent, NavigationDetail,
    NavigationItemSpec, NavigationPane, NavigationSpec, PresentationContext, PresentationScope,
    ReadingTextSpec, SemanticUiId, TextRole,
};

use super::StoryReport;
use crate::catalogue::StoryId;

/// Consumer-owned navigation state: the conversation open, whether its
/// details show, and what the reader did.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ResponsiveFixtureState {
    #[serde(skip)]
    story: Option<StoryId>,
    pub open: Option<String>,
    pub details: bool,
    /// The layout the recipe presented last: `stack`, `split` or `columns`.
    pub layout: String,
    pub opened: u32,
    pub backs: u32,
    pub toggles: u32,
}

impl ResponsiveFixtureState {
    /// Each story starts from its own fixture: the responsive story with
    /// Ada's conversation and her details open, the narrow one at its root,
    /// the list, with nothing open.
    fn reset_for(&mut self, story: StoryId) {
        if self.story == Some(story) {
            return;
        }
        let responsive = story == StoryId::NavigationResponsive;
        *self = Self {
            story: Some(story),
            open: responsive.then(|| "ada".into()),
            details: responsive,
            ..Self::default()
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum ResponsiveAction {
    Open,
    Back,
    Details,
}

impl ActionKey for ResponsiveAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Open => "gallery.navigation.open",
            Self::Back => "gallery.navigation.back",
            Self::Details => "gallery.navigation.details",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: match self {
                Self::Open => "Open",
                Self::Back => "Back",
                Self::Details => "Details",
            },
            description: match self {
                Self::Open => "Open the conversation with this ID",
                Self::Back => "Return to the previous level",
                Self::Details => "Show or hide this conversation's details",
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
            Self::Back | Self::Details => ArgumentContract::None,
        }
    }
}

struct Conversation {
    id: &'static str,
    title: &'static str,
    preview: &'static str,
    time: &'static str,
    unread: u64,
    archived: bool,
}

const CONVERSATIONS: [Conversation; 4] = [
    Conversation {
        id: "ada",
        title: "Ada Lovelace",
        preview: "The notes on the engine are ready",
        time: "09:41",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "grace",
        title: "Grace Hopper",
        preview: "The hall is booked for Friday",
        time: "Mon",
        unread: 1,
        archived: false,
    },
    Conversation {
        id: "edsger",
        title: "Edsger Dijkstra",
        preview: "Shortest route found",
        time: "12 Oct",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "katherine",
        title: "Katherine Johnson",
        preview: "Trajectory checked by hand",
        time: "1 Sep",
        unread: 0,
        archived: false,
    },
];

const LONG_CONVERSATIONS: [Conversation; 9] = [
    Conversation {
        id: "augusta",
        title: "Augusta Ada King, Countess of Lovelace",
        preview: "",
        time: "",
        unread: 3,
        archived: false,
    },
    Conversation {
        id: "planner",
        title: "Weekend planner for the walking group",
        preview: "",
        time: "",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "kueche",
        title: "Küchenhilfe für die Wochenendeinkäufe",
        preview: "",
        time: "",
        unread: 125,
        archived: false,
    },
    Conversation {
        id: "katherine",
        title: "Katherine Coleman Goble Johnson",
        preview: "",
        time: "",
        unread: 0,
        archived: true,
    },
    Conversation {
        id: "grace",
        title: "Grace Brewster Murray Hopper",
        preview: "",
        time: "",
        unread: 1,
        archived: false,
    },
    Conversation {
        id: "edsger",
        title: "Edsger Wybe Dijkstra",
        preview: "",
        time: "",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "barbara",
        title: "Barbara Liskov",
        preview: "",
        time: "",
        unread: 0,
        archived: false,
    },
    Conversation {
        id: "frances",
        title: "Frances Elizabeth Allen",
        preview: "",
        time: "",
        unread: 2,
        archived: false,
    },
    Conversation {
        id: "radia",
        title: "Radia Joy Perlman",
        preview: "",
        time: "",
        unread: 0,
        archived: false,
    },
];

/// The navigation recipe holding a conversation list, the open
/// conversation and its details, laid out for the Gallery's width.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut ResponsiveFixtureState,
    report: &mut StoryReport,
) {
    state.reset_for(story);
    let narrow = story == StoryId::NavigationResponsiveNarrow;
    let conversations: &[Conversation] = if narrow {
        &LONG_CONVERSATIONS
    } else {
        &CONVERSATIONS
    };
    let open = state.open.as_deref().and_then(|id| {
        conversations
            .iter()
            .find(|conversation| conversation.id == id)
    });
    let about = open.map(|conversation| format!("About {}", conversation.title));
    let spec = NavigationSpec {
        list_title: if narrow {
            "Conversations and shared assistants"
        } else {
            "Conversations"
        },
        content: open.map(|conversation| NavigationContent {
            title: conversation.title,
            reference: Some(DomainReference::External {
                namespace: "gallery.conversation".into(),
                id: conversation.id.into(),
            }),
            detail: about.as_deref().map(|title| NavigationDetail {
                title,
                shown: state.details,
                toggle: ResponsiveAction::Details,
                icon: IconId::Info,
            }),
        }),
        back: ResponsiveAction::Back,
    };
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.navigation"),
        SemanticUiId::new("gallery.story"),
    );
    let mut opened = None;
    let response = p.navigation(
        ui,
        "navigation",
        &spec,
        |_, _| Availability::Enabled,
        |ui, p, pane| match pane {
            NavigationPane::List => {
                if let Some(id) = list(ui, p, conversations, narrow, state.open.as_deref()) {
                    opened = Some(id);
                }
            }
            NavigationPane::Content => {
                if let Some(conversation) = open {
                    messages(ui, p, conversation);
                }
            }
            NavigationPane::Detail => {
                if let Some(conversation) = open {
                    details(ui, p, conversation);
                }
            }
            NavigationPane::Empty => {
                ui.add_space(p.tokens().spacing.section.0);
                p.reading_text(
                    ui,
                    "empty",
                    "Choose a conversation",
                    ReadingTextSpec::new(TextRole::Body),
                );
            }
        },
    );
    for pane in &response.panes {
        report.presented(&pane.observations);
    }
    state.layout = response.layout.as_str().into();
    if response
        .back
        .as_ref()
        .is_some_and(|back| back.response.clicked())
    {
        if state.details {
            state.details = false;
        } else {
            state.open = None;
        }
        state.backs += 1;
        report.repaint = Some("navigation back");
    }
    if response
        .toggle
        .as_ref()
        .is_some_and(|toggle| toggle.response.clicked())
    {
        state.details = !state.details;
        state.toggles += 1;
        report.repaint = Some("details toggled");
    }
    if let Some(id) = opened {
        state.open = Some(id.into());
        state.opened += 1;
        report.repaint = Some("conversation opened");
    }
    p.finish(ui);
}

/// The destinations, in a list that scrolls; returns the one clicked.
fn list(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    conversations: &[Conversation],
    narrow: bool,
    open: Option<&str>,
) -> Option<&'static str> {
    let mut clicked = None;
    egui::ScrollArea::vertical()
        .id_salt("navigation-list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(p.tokens().spacing.unit.0);
            for conversation in conversations {
                let argument = ActionArgument::from(conversation.id);
                let availability = if conversation.archived {
                    Availability::Disabled {
                        reason: "This conversation is archived".into(),
                    }
                } else {
                    Availability::Enabled
                };
                let target = ActionTarget::application(ResponsiveAction::Open);
                let response = if narrow {
                    // Destinations without previews: navigation items with
                    // their unread counts.
                    let meaning = "unread messages";
                    p.bound_navigation_item(
                        ui,
                        ResponsiveAction::Open,
                        NavigationItemSpec {
                            target,
                            icon: IconId::User,
                            label: conversation.title,
                            description: "Open the conversation",
                            selected: open == Some(conversation.id),
                            availability,
                            badge: (conversation.unread > 0).then_some(NavigationBadge::Count {
                                value: conversation.unread,
                                meaning,
                            }),
                        },
                        &argument,
                    )
                } else {
                    let unread = match conversation.unread {
                        1 => "1 unread message".to_owned(),
                        count => format!("{count} unread messages"),
                    };
                    p.conversation_row(
                        ui,
                        ResponsiveAction::Open,
                        ConversationRowSpec {
                            title: conversation.title,
                            preview: Some(conversation.preview),
                            time: Some(conversation.time),
                            unread: (conversation.unread > 0).then_some(unread.as_str()),
                            selected: open == Some(conversation.id),
                            avatar: Some(AvatarSpec::person(conversation.title, AvatarSize::List)),
                        },
                        target,
                        &availability,
                        &argument,
                    )
                };
                if response.clicked() {
                    clicked = Some(conversation.id);
                }
            }
        });
    clicked
}

/// A few messages of the open conversation.
fn messages(ui: &mut egui::Ui, p: &mut PresentationContext, conversation: &Conversation) {
    egui::ScrollArea::vertical()
        .id_salt("navigation-content")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let margin = p.tokens().spacing.section.0;
            egui::Frame::new()
                .inner_margin(egui::Margin::same(margin as i8))
                .show(ui, |ui| {
                    let preview = if conversation.preview.is_empty() {
                        "Here is this week's plan."
                    } else {
                        conversation.preview
                    };
                    for (id, side, text) in [
                        (1, MessageSide::Own, "Any news?"),
                        (2, MessageSide::Other, preview),
                    ] {
                        let sender = match side {
                            MessageSide::Own => "You",
                            MessageSide::Other => conversation.title,
                        };
                        p.message_bubble(
                            ui,
                            ("message", id),
                            MessageBubbleSpec { side, sender },
                            |ui, p| {
                                p.reading_text(ui, "text", text, ReadingTextSpec::default());
                            },
                        );
                        ui.add_space(p.tokens().spacing.inline.0);
                    }
                });
        });
}

/// The open conversation's details.
fn details(ui: &mut egui::Ui, p: &mut PresentationContext, conversation: &Conversation) {
    let margin = p.tokens().spacing.section.0;
    egui::Frame::new()
        .inner_margin(egui::Margin::same(margin as i8))
        .show(ui, |ui| {
            p.property_row(ui, "name", "Name", conversation.title);
            p.property_row(ui, "last", "Last message", conversation.time);
            let unread = conversation.unread.to_string();
            p.property_row(ui, "unread", "Unread", &unread);
        });
}

#[cfg(test)]
mod tests {
    use eframe::egui;
    use stobie_ui_egui::{ApplicationTheme, UiRole};

    use super::super::probe::{self, StoryProbe};
    use crate::app::{GalleryConfiguration, GalleryWidth};
    use crate::catalogue::StoryId;

    /// The panes the responsive story presents at a Gallery width.
    fn panes(story: StoryId, width: GalleryWidth) -> Vec<String> {
        let configuration = GalleryConfiguration {
            width,
            ..GalleryConfiguration::default()
        };
        let preferences = configuration.preferences();
        let probe = StoryProbe {
            story,
            tokens: preferences.tokens(true),
            scale: 1.0,
            clip: None,
            theme: ApplicationTheme::analytical(),
            preferences,
            heading: false,
        };
        // The Gallery's story surface: its width less the surface's margins.
        let size = egui::vec2(width.points() - 24.0, 520.0);
        let (snapshot, _) = probe::render(&egui::Context::default(), probe, size);
        snapshot
            .by_role(UiRole::Pane)
            .map(|pane| pane.name.clone())
            .collect()
    }

    #[test]
    fn the_story_lays_its_panes_out_for_each_gallery_width() {
        let story = StoryId::NavigationResponsive;
        // A phone stacks the deepest open level: the details.
        assert_eq!(panes(story, GalleryWidth::Narrow), ["About Ada Lovelace"]);
        // A tablet splits the list beside the deepest level.
        assert_eq!(
            panes(story, GalleryWidth::Wide),
            ["Conversations", "About Ada Lovelace"]
        );
        // A desktop shows all three.
        assert_eq!(
            panes(story, GalleryWidth::Desktop),
            ["Conversations", "Ada Lovelace", "About Ada Lovelace"]
        );
        // The narrow story starts at its root, the list.
        assert_eq!(
            panes(StoryId::NavigationResponsiveNarrow, GalleryWidth::Narrow),
            ["Conversations and shared assistants"]
        );
    }
}
