use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionScope, ActionSpec,
    ActionTarget, AnchoredScrollArea, Availability, ContentTextSpec, DesignTokens,
    MessageBubbleSpec, MessageSide, PresentationContext, PresentationScope, ReadingTextSpec,
    SemanticUiId, TextInteraction, TextLayoutObservation, TextOverflow, TextRole,
    observe_visible_only,
};

use super::StoryReport;

/// Consumer-owned state: how many pages of older messages are loaded.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ScrollFixtureState {
    pub older_pages: u32,
    pub loads: u32,
}

/// The pages of older messages the conversation holds beyond the first.
const OLDER_PAGES: u32 = 3;
/// Messages in a page.
const PAGE: i32 = 12;
/// The messages shown before any older ones are loaded.
const RECENT: i32 = 16;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum ScrollAction {
    LoadOlder,
}

impl ActionKey for ScrollAction {
    fn stable_id(self) -> &'static str {
        "gallery.conversation.load_older"
    }

    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: "Load older",
            description: "Show the page of messages before the oldest one shown",
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
}

const PHRASES: [&str; 6] = [
    "The hall is booked for Friday from six.",
    "Could you bring the long tables from the shed, and the extension lead for the urn? \
     The caretaker leaves the side door open until seven.",
    "Thank you!",
    "I'll print the running order tonight.",
    "The choir needs the stage cleared by half past seven, so the raffle moves to the \
     foyer, next to the tea and the second-hand books.",
    "Sounds good.",
];

/// A message's words: numbered, so each one reads as its own.
fn message(id: i32) -> String {
    let phrase = PHRASES[id.rem_euclid(PHRASES.len() as i32) as usize];
    format!("{}. {phrase}", id + PAGE * OLDER_PAGES as i32 + 1)
}

/// A list pane whose footer sits at its foot, beside (or above) a
/// conversation that loads older messages above the ones in view. Both
/// panes scroll, so the story publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut ScrollFixtureState,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| {
        let mut p = PresentationContext::new(
            ui,
            *tokens,
            scale,
            PresentationScope::new("gallery.scroll"),
            SemanticUiId::new("gallery.story"),
        );
        let height = ui.available_height();
        if ui.available_width() >= 720.0 * scale {
            // Desktop: the list beside the conversation.
            ui.horizontal_top(|ui| {
                let list = 280.0 * scale;
                ui.allocate_ui_with_layout(
                    egui::vec2(list, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(list);
                        list_pane(ui, &mut p, height);
                    },
                );
                ui.separator();
                ui.vertical(|ui| conversation(ui, &mut p, height, state, report));
            });
        } else {
            // Phone: a short list above the conversation.
            let list = (height * 0.3).max(p.tokens().geometry.minimum_hit_size.0 * 3.0);
            list_pane(ui, &mut p, list);
            ui.separator();
            let rest = ui.available_height();
            conversation(ui, &mut p, rest, state, report);
        }
        p.retain_visible_text(ui, fully_visible_text);
        report.presented(&p.finish(ui));
    });
}

fn fully_visible_text(layout: &TextLayoutObservation) -> bool {
    let a = layout.allocated_rect;
    let c = layout.clip_rect;
    a.min_x >= c.min_x && a.min_y >= c.min_y && a.max_x <= c.max_x && a.max_y <= c.max_y
}

fn line(role: TextRole) -> ContentTextSpec {
    ContentTextSpec {
        role,
        overflow: TextOverflow::Ellipsis,
        max_lines: 1,
        interaction: TextInteraction::Inert,
    }
}

/// A short list with the account at the pane's foot.
fn list_pane(ui: &mut egui::Ui, p: &mut PresentationContext, height: f32) {
    let area = egui::ScrollArea::vertical()
        .max_height(height)
        .auto_shrink([false, false]);
    AnchoredScrollArea::new("lists", area).show(ui, |ui, rows| {
        for (key, name) in [
            ("hall", "Hall booking"),
            ("choir", "Choir rehearsal"),
            ("raffle", "Raffle prizes"),
        ] {
            rows.row(ui, key, |ui| {
                p.content(ui, ("list", key), name, line(TextRole::Body));
                ui.add_space(p.tokens().spacing.inline.0);
            });
        }
        rows.footer(ui, |ui| {
            ui.separator();
            p.content(ui, "account", "Ada Lovelace", line(TextRole::Body));
            p.content(ui, "account-status", "Signed in", line(TextRole::Secondary));
        });
    });
}

/// The conversation: a "Load older" control in the pane's header, above
/// its messages, newest at the foot, keeping the reader's place when older
/// messages arrive. The control stays in view wherever the reader is, so it
/// loads older messages both from history and from the newest message.
fn conversation(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    height: f32,
    state: &mut ScrollFixtureState,
    report: &mut StoryReport,
) {
    let top = ui.cursor().top();
    let more = state.older_pages < OLDER_PAGES;
    let availability = if more {
        Availability::Enabled
    } else {
        Availability::Disabled {
            reason: "There are no older messages".into(),
        }
    };
    let older = p.action(
        ui,
        ScrollAction::LoadOlder,
        ActionButtonSpec {
            target: ActionTarget::application(ScrollAction::LoadOlder),
            availability,
            state: ActionButtonState::Momentary,
            emphasis: ActionEmphasis::Quiet,
            compact: false,
        },
    );
    if older.clicked() && more {
        state.older_pages += 1;
        state.loads += 1;
        report.repaint = Some("older messages loaded");
    }
    let rest = height - (ui.cursor().top() - top);
    let area = egui::ScrollArea::vertical()
        .max_height(rest)
        .stick_to_bottom(true)
        .auto_shrink([false, false]);
    AnchoredScrollArea::new("messages", area).show(ui, |ui, rows| {
        let first = -(state.older_pages as i32) * PAGE;
        for id in first..RECENT {
            rows.row(ui, id, |ui| {
                let side = if id.rem_euclid(3) == 1 {
                    MessageSide::Own
                } else {
                    MessageSide::Other
                };
                let sender = match side {
                    MessageSide::Own => "You",
                    MessageSide::Other => "Ada Lovelace",
                };
                let text = message(id);
                p.message_bubble(
                    ui,
                    ("message", id),
                    MessageBubbleSpec { side, sender },
                    |ui, p| p.reading_text(ui, "text", &text, ReadingTextSpec::default()),
                );
                ui.add_space(p.tokens().spacing.inline.0);
            });
        }
    });
}
