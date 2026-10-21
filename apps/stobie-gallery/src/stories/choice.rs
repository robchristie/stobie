use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionKey, ActionScope, ActionSpec, ActionTarget, ArgumentContract, Availability,
    ChoiceArgument, ChoiceOption, ContentTextSpec, DesignTokens, PresentationContext,
    PresentationScope, SegmentedChoiceSpec, SemanticUiId, TextInteraction, TextLayoutObservation,
    TextOverflow, TextRole, observe_visible_only,
};

use super::StoryReport;

/// Consumer-owned choices: how a routine repeats, the days it runs on and
/// when it reminds, and whether the fixture has focused its chosen option.
#[derive(Clone, Debug, Serialize)]
pub struct ChoiceFixtureState {
    pub repeat: usize,
    pub days: [bool; 7],
    pub reminder: usize,
    pub focused: bool,
}

impl Default for ChoiceFixtureState {
    fn default() -> Self {
        Self {
            repeat: 1,
            days: [true, false, true, false, true, false, false],
            reminder: 1,
            focused: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Repeat {
    Daily,
    Weekdays,
    Weekly,
    Hourly,
}

impl ChoiceArgument for Repeat {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Daily, "daily", "Daily"),
        (Self::Weekdays, "weekdays", "Weekdays"),
        (Self::Weekly, "weekly", "Weekly"),
        (Self::Hourly, "hourly", "Every few hours"),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Day {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl ChoiceArgument for Day {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::Monday, "mon", "Monday"),
        (Self::Tuesday, "tue", "Tuesday"),
        (Self::Wednesday, "wed", "Wednesday"),
        (Self::Thursday, "thu", "Thursday"),
        (Self::Friday, "fri", "Friday"),
        (Self::Saturday, "sat", "Saturday"),
        (Self::Sunday, "sun", "Sunday"),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Reminder {
    AtStart,
    Before,
    Evening,
    Never,
}

impl ChoiceArgument for Reminder {
    const CHOICES: &'static [(Self, &'static str, &'static str)] = &[
        (Self::AtStart, "start", "At the start time"),
        (Self::Before, "before", "Fifteen minutes before"),
        (Self::Evening, "evening", "The evening before"),
        (Self::Never, "never", "Never"),
    ];
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum ChoiceAction {
    Repeat,
    Day,
    Reminder,
    Notify,
}

impl ActionKey for ChoiceAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Repeat => "gallery.routine.repeat",
            Self::Day => "gallery.routine.day",
            Self::Reminder => "gallery.routine.reminder",
            Self::Notify => "gallery.routine.notify",
        }
    }

    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: match self {
                Self::Repeat => "Repeats",
                Self::Day => "Runs on",
                Self::Reminder => "Reminder",
                Self::Notify => "Notify",
            },
            description: match self {
                Self::Repeat => "Choose how often the routine runs",
                Self::Day => "Run the routine on this day, or stop",
                Self::Reminder => "Choose when the routine reminds you",
                Self::Notify => "Notify on this day, or stop",
            },
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }

    fn argument(self) -> ArgumentContract {
        let options = match self {
            Self::Repeat => Repeat::options(),
            Self::Day | Self::Notify => Day::options(),
            Self::Reminder => Reminder::options(),
        };
        ArgumentContract::Choice { options }
    }
}

const HEADING: ContentTextSpec = ContentTextSpec {
    role: TextRole::SectionHeading,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

/// The width of the story's narrow column, as a phone's sheet beside a
/// margin leaves it.
const NARROW_COLUMN: f32 = 240.0;

/// Segmented choices as a routine editor uses them: a single choice with an
/// unavailable option, focused at its chosen option; a multiple choice of
/// weekdays with short labels; long labels that wrap whole; a choice that is
/// unavailable throughout; and the weekdays again in a narrow column. The
/// page scrolls, so it publishes only what is visible.
pub(super) fn story(
    ui: &mut egui::Ui,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut ChoiceFixtureState,
    report: &mut StoryReport,
) {
    observe_visible_only(ui, |ui| {
        let mut p = PresentationContext::new(
            ui,
            *tokens,
            scale,
            PresentationScope::new("gallery.choice"),
            SemanticUiId::new("gallery.story"),
        );
        let gap = tokens.spacing.section.0;
        egui::ScrollArea::vertical()
            .id_salt("choice-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if choices(ui, &mut p, state, gap) {
                    report.repaint = Some("choice option");
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

/// Each choice's options, chosen where `chosen` says so.
fn options<C: ChoiceArgument>(chosen: impl Fn(usize) -> bool) -> Vec<ChoiceOption<'static>> {
    C::CHOICES
        .iter()
        .enumerate()
        .map(|(index, (value, ..))| ChoiceOption::choice(*value, chosen(index)))
        .collect()
}

fn weekdays(days: &[bool; 7]) -> Vec<ChoiceOption<'static>> {
    options::<Day>(|index| days[index])
        .into_iter()
        .map(|option| {
            let short = &option.label[..3];
            option.with_short_label(short)
        })
        .collect()
}

/// Present every choice; returns whether an option changed the state.
fn choices(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    state: &mut ChoiceFixtureState,
    gap: f32,
) -> bool {
    let mut changed = false;

    p.content(ui, "repeat-heading", "Repeats", HEADING);
    let repeats = options::<Repeat>(|index| index == state.repeat);
    let response = p.segmented_choice(
        ui,
        "repeat",
        ActionTarget::application(ChoiceAction::Repeat),
        &SegmentedChoiceSpec::single("Repeats", &repeats),
        |_, argument| {
            if argument.as_choice::<Repeat>() == Ok(Repeat::Hourly) {
                Availability::Disabled {
                    reason: "Turned off for this routine".into(),
                }
            } else {
                Availability::Enabled
            }
        },
    );
    if !state.focused {
        // The fixture shows keyboard focus on the chosen option.
        response.options[state.repeat].request_focus();
        state.focused = true;
        changed = true;
    }
    if let Some(index) = response.activated {
        state.repeat = index;
        changed = true;
    }
    ui.add_space(gap);

    p.content(ui, "days-heading", "Runs on", HEADING);
    let days = weekdays(&state.days);
    let response = p.segmented_choice(
        ui,
        "days",
        ActionTarget::application(ChoiceAction::Day),
        &SegmentedChoiceSpec::multiple("Runs on", &days),
        |_, _| Availability::Enabled,
    );
    if let Some(index) = response.activated {
        state.days[index] = !state.days[index];
        changed = true;
    }
    ui.add_space(gap);

    p.content(ui, "reminder-heading", "Reminder", HEADING);
    let reminders = options::<Reminder>(|index| index == state.reminder);
    let response = p.segmented_choice(
        ui,
        "reminder",
        ActionTarget::application(ChoiceAction::Reminder),
        &SegmentedChoiceSpec::single("Reminder", &reminders),
        |_, _| Availability::Enabled,
    );
    if let Some(index) = response.activated {
        state.reminder = index;
        changed = true;
    }
    ui.add_space(gap);

    p.content(ui, "notify-heading", "Notify on", HEADING);
    let notify = weekdays(&[false, false, false, false, true, false, false]);
    p.segmented_choice(
        ui,
        "notify",
        ActionTarget::application(ChoiceAction::Notify),
        &SegmentedChoiceSpec::multiple("Notify on", &notify),
        |_, _| Availability::Disabled {
            reason: "Notifications are off".into(),
        },
    );
    ui.add_space(gap);

    p.content(ui, "narrow-heading", "Runs on, in a narrow column", HEADING);
    let width = NARROW_COLUMN.min(ui.available_width());
    ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
        ui.set_max_width(width);
        let response = p.segmented_choice(
            ui,
            "narrow-days",
            ActionTarget::application(ChoiceAction::Day),
            &SegmentedChoiceSpec::multiple("Runs on", &days),
            |_, _| Availability::Enabled,
        );
        if let Some(index) = response.activated {
            state.days[index] = !state.days[index];
            changed = true;
        }
    });
    changed
}

#[cfg(test)]
mod tests {
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, ThemeVariant, UiRole, audit_accesskit, audit_text_layouts,
    };

    use super::*;
    use crate::catalogue::StoryId;
    use crate::stories::probe::{StoryProbe, render};

    fn probe(
        variant: ThemeVariant,
        density: DensityVariant,
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        let probe = || StoryProbe {
            story: StoryId::ChoiceSegmented,
            tokens: DesignTokens::resolve(variant, density),
            scale,
            clip: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            theme: ApplicationTheme::analytical(),
            preferences: Default::default(),
            heading: false,
        };
        // The fixture asks for focus on its first pass; the second shows it.
        render(&context, probe(), size);
        render(&context, probe(), size)
    }

    #[test]
    fn the_story_shows_radio_groups_toggles_and_their_states() {
        let (snapshot, mut output) = probe(
            ThemeVariant::Dark,
            DensityVariant::Comfortable,
            1.0,
            egui::vec2(616.0, 1_200.0),
        );
        assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
        let groups: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::Section)
            .map(|node| node.name.as_str())
            .collect();
        assert_eq!(
            groups,
            ["Repeats", "Runs on", "Reminder", "Notify on", "Runs on"]
        );
        let radios: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| node.role == UiRole::RadioButton)
            .collect();
        assert_eq!(radios.len(), 8);
        // The chosen option is checked and holds the fixture's focus.
        let weekdays = radios.iter().find(|node| node.name == "Weekdays").unwrap();
        assert_eq!(weekdays.checked, Some(true));
        assert!(weekdays.focused);
        let hourly = radios
            .iter()
            .find(|node| node.name == "Every few hours")
            .unwrap();
        assert!(!hourly.enabled);
        // The disabled choice keeps its pressed day and every reason.
        let notify: Vec<_> = snapshot
            .nodes
            .iter()
            .filter(|node| {
                node.role == UiRole::Button
                    && node.disabled_reason.as_deref() == Some("Notifications are off")
            })
            .collect();
        assert_eq!(notify.len(), 7);
        assert_eq!(
            notify
                .iter()
                .filter(|node| node.checked == Some(true))
                .count(),
            1
        );
        let update = output.platform_output.accesskit_update.take().unwrap();
        let findings = audit_accesskit(&snapshot, &update);
        assert!(findings.is_empty(), "{findings:?}");
        output.textures_delta.clear();
    }

    #[test]
    fn a_narrow_column_at_larger_text_keeps_every_label_whole() {
        let (snapshot, mut output) = probe(
            ThemeVariant::LightHighContrast,
            DensityVariant::Comfortable,
            1.5,
            egui::vec2(296.0, 2_000.0),
        );
        let findings = audit_text_layouts(&snapshot.text);
        assert!(findings.is_empty(), "{findings:#?}");
        assert!(snapshot.text.iter().all(|text| !text.truncated));
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        output.textures_delta.clear();
    }
}
