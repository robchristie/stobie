use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionKey, ActionScope, ActionSpec, ActionTarget, ArgumentContract,
    Availability, ComposerControl, ComposerSpec, ContentTextSpec, DesignTokens, IconId,
    PresentationContext, PresentationScope, SemanticUiId, TextInteraction, TextOverflow, TextRole,
};

use super::StoryReport;
use crate::catalogue::StoryId;

/// Consumer-owned drafts, one per composer, and the messages sent.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ComposerFixtureState {
    pub seeded: bool,
    pub drafts: Vec<String>,
    pub sent: Vec<String>,
    pub attached: u32,
    pub dictated: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum ComposerAction {
    Send,
    Attach,
    Dictate,
}

impl ActionKey for ComposerAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Send => "gallery.message.send",
            Self::Attach => "gallery.message.attach",
            Self::Dictate => "gallery.message.dictate",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: match self {
                Self::Send => "Send",
                Self::Attach => "Attach",
                Self::Dictate => "Dictate",
            },
            description: match self {
                Self::Send => "Send the message",
                Self::Attach => "Attach a file to the message",
                Self::Dictate => "Dictate a message",
            },
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
    fn argument(self) -> ArgumentContract {
        match self {
            Self::Send => ArgumentContract::Text {
                max_length: Some(2000),
            },
            Self::Attach | Self::Dictate => ArgumentContract::None,
        }
    }
}

/// One composer of the story: its caption, hint, line limit, seeded draft,
/// and whether a send is in flight.
struct Example {
    caption: &'static str,
    hint: &'static str,
    max_rows: usize,
    draft: &'static str,
    sending: bool,
}

const CHAT: [Example; 3] = [
    Example {
        caption: "Empty: the microphone stands in for send",
        hint: "Message Ada",
        max_rows: 6,
        draft: "",
        sending: false,
    },
    Example {
        caption: "A draft ready to send",
        hint: "Message Ada",
        max_rows: 6,
        draft: "Can you book the 9:30 train to Adelaide?",
        sending: false,
    },
    Example {
        caption: "Three lines, grown to fit",
        hint: "Message Ada",
        max_rows: 6,
        draft: "Shopping for Saturday:\nbread, milk and eggs\nsomething for dessert",
        sending: false,
    },
];

const LONG_NARROW: [Example; 3] = [
    Example {
        caption: "Ein langer Hinweis in einer schmalen Spalte",
        hint: "Nachricht an Augusta Ada King, Gräfin von Lovelace",
        max_rows: 4,
        draft: "",
        sending: false,
    },
    Example {
        caption: "Past its four-line limit, the field scrolls",
        hint: "Message Ada",
        max_rows: 4,
        draft: "Größere Schrift, schmale Spalte und hoher Kontrast bleiben lesbar. \
Every line wraps at the field's width, and once the draft passes its limit \
the field stops growing and scrolls, so the controls stay in reach.",
        sending: false,
    },
    Example {
        caption: "Sending: send is disabled with its reason",
        hint: "Message Ada",
        max_rows: 4,
        draft: "On my way.",
        sending: true,
    },
];

/// Composers for a chat: a field that grows with its draft, an attach
/// control, a microphone shown by its label, and send, which carries the
/// draft. Sending clears the consumer's draft.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut ComposerFixtureState,
    report: &mut StoryReport,
) {
    let examples: &[Example] = if story == StoryId::ComposerChat {
        &CHAT
    } else {
        &LONG_NARROW
    };
    if !state.seeded {
        state.seeded = true;
        state.drafts = examples
            .iter()
            .map(|example| example.draft.to_owned())
            .collect();
    }
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.composer"),
        SemanticUiId::new("gallery.story"),
    );
    for (index, example) in examples.iter().enumerate() {
        p.content(
            ui,
            ("caption", index),
            example.caption,
            ContentTextSpec {
                role: TextRole::Caption,
                overflow: TextOverflow::Ellipsis,
                max_lines: 1,
                interaction: TextInteraction::Inert,
            },
        );
        let spec = ComposerSpec::new(
            "Message",
            ComposerControl::icon(ComposerAction::Send, IconId::Send),
        )
        .hint(example.hint)
        .max_rows(example.max_rows)
        .leading(ComposerControl::icon(
            ComposerAction::Attach,
            IconId::Attach,
        ))
        .trailing(ComposerControl::label(ComposerAction::Dictate));
        let sending = example.sending;
        let availability = |target: ActionTarget<ComposerAction>, argument: &ActionArgument| {
            match (target.action, argument) {
                (ComposerAction::Send, _) if sending => Availability::Disabled {
                    reason: "The last message is still sending".into(),
                },
                (ComposerAction::Send, ActionArgument::Text(text)) if !text.trim().is_empty() => {
                    Availability::Enabled
                }
                // An empty draft shows the microphone alone.
                (ComposerAction::Send, _) => Availability::Hidden,
                _ => Availability::Enabled,
            }
        };
        let draft = &mut state.drafts[index];
        let response = p.composer(ui, ("composer", index), &spec, draft, availability);
        if let Some(sent) = response.submitted {
            state.sent.push(sent);
            draft.clear();
            report.repaint = Some("message sent");
        }
        if response.changed {
            report.repaint = Some("draft edited");
        }
        if response
            .leading
            .is_some_and(|control| control.response.clicked())
        {
            state.attached += 1;
        }
        if response
            .trailing
            .is_some_and(|control| control.response.clicked())
        {
            state.dictated += 1;
        }
        ui.add_space(tokens.spacing.section.0);
    }
    report.presented(&p.finish(ui));
}
