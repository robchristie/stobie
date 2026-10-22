use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionButtonContent, ActionButtonSpec, ActionButtonState, ActionEmphasis,
    ActionKey, ActionTarget, ArgumentContract, Availability, DesignTokens, IconId, ListRowSpec,
    PresentationContext, PresentationScope, SemanticUiId, TextLayoutObservation,
    observe_visible_only,
};

use super::StoryReport;
use crate::catalogue::StoryId;

/// Consumer-owned row state: who is pinned and who was last opened.
#[derive(Clone, Debug, Default, Serialize)]
pub struct RowFixtureState {
    pub seeded: bool,
    pub pinned: Vec<u32>,
    pub opened: Option<u32>,
    pub selected: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum RowAction {
    Select,
    Open,
    Pin,
    Remove,
}

impl ActionKey for RowAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Select => "gallery.person.select",
            Self::Open => "gallery.person.open",
            Self::Pin => "gallery.person.pin",
            Self::Remove => "gallery.person.remove",
        }
    }
    fn specification(self) -> stobie_ui_egui::ActionSpec<Self> {
        stobie_ui_egui::ActionSpec {
            id: self,
            label: match self {
                Self::Select => "Select",
                Self::Open => "Open",
                Self::Pin => "Pinned",
                Self::Remove => "Remove",
            },
            description: match self {
                Self::Select => "Select the person with this ID",
                Self::Open => "Open the person with this ID",
                Self::Pin => "Keep the person with this ID at the top",
                Self::Remove => "Remove the person with this ID",
            },
            compact_label: None,
            shortcut: None,
            scope: stobie_ui_egui::ActionScope::Application,
        }
    }
    fn argument(self) -> ArgumentContract {
        ArgumentContract::Number {
            minimum: 1.0,
            maximum: 99.0,
            integer: true,
        }
    }
}

struct Person {
    id: u32,
    name: &'static str,
    role: &'static str,
    away: bool,
}

const PEOPLE: [Person; 4] = [
    Person {
        id: 1,
        name: "Ada Lovelace",
        role: "Mathematician",
        away: false,
    },
    Person {
        id: 2,
        name: "Grace Hopper",
        role: "Rear admiral and computer scientist",
        away: false,
    },
    Person {
        id: 3,
        name: "Edsger Dijkstra",
        role: "Computer scientist",
        away: false,
    },
    Person {
        id: 4,
        name: "Katherine Johnson",
        role: "Mathematician at NASA",
        away: false,
    },
];

const LONG_PEOPLE: [Person; 4] = [
    Person {
        id: 11,
        name: "Augusta Ada King, Countess of Lovelace",
        role: "Mathematikerin und Verfasserin der ersten veröffentlichten Programme",
        away: false,
    },
    Person {
        id: 12,
        name: "Grace Brewster Murray Hopper",
        role: "Contre-amirale, informaticienne et autrice du premier compilateur",
        away: true,
    },
    Person {
        id: 13,
        name: "Edsger Wybe Dijkstra",
        role: "Informaticus",
        away: false,
    },
    Person {
        id: 14,
        name: "Katherine Coleman Goble Johnson",
        role: "Mathematician who calculated trajectories for crewed spaceflight",
        away: false,
    },
];

/// A list of people, each row selecting its person and offering Open,
/// Pinned and Remove for that person alone. The long story scrolls, so it
/// publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut RowFixtureState,
    report: &mut StoryReport,
) {
    if !state.seeded {
        // The seed: the first person is pinned, so their Remove is disabled.
        state.seeded = true;
        state
            .pinned
            .push(if story == StoryId::RowsActions { 1 } else { 11 });
    }
    observe_visible_only(ui, |ui| {
        let mut p = PresentationContext::new(
            ui,
            *tokens,
            scale,
            PresentationScope::new("gallery.rows"),
            SemanticUiId::new("gallery.story"),
        );
        let people: &[Person] = if story == StoryId::RowsActions {
            &PEOPLE
        } else {
            &LONG_PEOPLE
        };
        egui::ScrollArea::vertical()
            .id_salt("rows-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for person in people {
                    if row(ui, &mut p, person, state) {
                        report.repaint = Some("row action");
                    }
                    ui.separator();
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

/// One person's row and actions; returns whether a click changed the state.
fn row(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    person: &Person,
    state: &mut RowFixtureState,
) -> bool {
    let id = ActionArgument::from(person.id);
    let pinned = state.pinned.contains(&person.id);
    let mut changed = false;
    let availability = if person.away {
        Availability::Disabled {
            reason: format!("{} is away", person.name).into(),
        }
    } else {
        Availability::Enabled
    };
    if p.list_row(
        ui,
        RowAction::Select,
        ListRowSpec {
            title: person.name,
            detail: Some(person.role),
        },
        ActionTarget::application(RowAction::Select),
        &availability,
        &id,
    )
    .clicked()
    {
        state.selected = Some(person.id);
        changed = true;
    }
    ui.horizontal_wrapped(|ui| {
        // Icon-only Remove takes the touch target; a row as tall centres the
        // labelled Open and Pinned on its line.
        ui.set_row_height(p.tokens().touch_target_size().0);
        let button = |ui: &mut egui::Ui,
                      p: &mut PresentationContext,
                      action: RowAction,
                      state: ActionButtonState,
                      emphasis: ActionEmphasis,
                      content: ActionButtonContent,
                      availability: Availability| {
            let name = format!("{} {}", action.specification().label, person.name);
            p.bound_action(
                ui,
                action,
                ActionButtonSpec {
                    target: ActionTarget::application(action),
                    availability,
                    state,
                    emphasis,
                    compact: false,
                },
                content,
                &id,
                Some(&name),
            )
            .clicked()
        };
        if button(
            ui,
            p,
            RowAction::Open,
            ActionButtonState::Momentary,
            ActionEmphasis::Quiet,
            ActionButtonContent::Text,
            availability.clone(),
        ) {
            state.opened = Some(person.id);
            changed = true;
        }
        if button(
            ui,
            p,
            RowAction::Pin,
            ActionButtonState::Toggle { pressed: pinned },
            ActionEmphasis::Normal,
            ActionButtonContent::Text,
            availability.clone(),
        ) {
            if pinned {
                state.pinned.retain(|id| *id != person.id);
            } else {
                state.pinned.push(person.id);
            }
            changed = true;
        }
        let remove = if pinned {
            Availability::Disabled {
                reason: format!("Unpin {} first", person.name).into(),
            }
        } else {
            availability.clone()
        };
        // Removing is the consumer's; the fixture keeps everyone listed.
        button(
            ui,
            p,
            RowAction::Remove,
            ActionButtonState::Momentary,
            ActionEmphasis::Quiet,
            ActionButtonContent::IconOnly(IconId::Remove),
            remove,
        );
    });
    changed
}
