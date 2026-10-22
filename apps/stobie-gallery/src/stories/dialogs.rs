use eframe::egui;
use serde::Serialize;
use stobie_ui_egui::{
    ActionArgument, ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionScope,
    ActionSpec, ActionTarget, Availability, ConfirmSpec, ContentTextSpec, DesignTokens, DialogSpec,
    PresentationContext, PresentationScope, SemanticUiId, SwitchSpec, TextFieldSpec,
    TextInteraction, TextOverflow, TextRole,
};

use super::StoryReport;
use crate::catalogue::StoryId;

/// Consumer-owned dialog state: which dialog is open and the routine it
/// edits. The story opens its dialog when it is first shown.
#[derive(Clone, Debug, Serialize)]
pub struct DialogFixtureState {
    pub seeded: bool,
    pub open: bool,
    pub name: String,
    pub prompt: String,
    pub notify: bool,
    pub saved: u32,
    pub deleted: bool,
}

impl Default for DialogFixtureState {
    fn default() -> Self {
        Self {
            seeded: false,
            open: false,
            name: "Morning briefing".into(),
            prompt: "Summarise the weather, my calendar and the overnight news.".into(),
            notify: true,
            saved: 0,
            deleted: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
enum RoutineAction {
    Edit,
    Close,
    Save,
    Notify,
    Delete,
    ConfirmDelete,
    Keep,
}

impl ActionKey for RoutineAction {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Edit => "gallery.routine.edit",
            Self::Close => "gallery.routine.close",
            Self::Save => "gallery.routine.save",
            Self::Notify => "gallery.routine.notify",
            Self::Delete => "gallery.routine.delete",
            Self::ConfirmDelete => "gallery.routine.confirm_delete",
            Self::Keep => "gallery.routine.keep",
        }
    }
    fn specification(self) -> ActionSpec<Self> {
        ActionSpec {
            id: self,
            label: match self {
                Self::Edit => "Edit routine",
                Self::Close => "Close",
                Self::Save => "Save",
                Self::Notify => "Send a notification",
                Self::Delete => "Delete routine",
                Self::ConfirmDelete => "Delete",
                Self::Keep => "Keep routine",
            },
            description: match self {
                Self::Edit => "Edit the routine's name, prompt and notification",
                Self::Close => "Close the editor without saving",
                Self::Save => "Save the routine and close the editor",
                Self::Notify => "Send a notification when the routine runs",
                Self::Delete => "Delete the routine",
                Self::ConfirmDelete => "Delete the routine and its history",
                Self::Keep => "Keep the routine",
            },
            compact_label: None,
            shortcut: None,
            scope: ActionScope::Application,
        }
    }
}

const HEADING: ContentTextSpec = ContentTextSpec {
    role: TextRole::SectionHeading,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

const DETAIL: ContentTextSpec = ContentTextSpec {
    role: TextRole::Secondary,
    overflow: TextOverflow::Wrap,
    max_lines: 2,
    interaction: TextInteraction::Inert,
};

const CAPTION: ContentTextSpec = ContentTextSpec {
    role: TextRole::Caption,
    overflow: TextOverflow::Ellipsis,
    max_lines: 1,
    interaction: TextInteraction::Inert,
};

const SCHEDULE: &str = "Weekdays at 7:00";

/// The field IDs, which agents use with `stobie set-text`.
pub(crate) const NAME_FIELD: &str = "gallery.routine.name";
pub(crate) const PROMPT_FIELD: &str = "gallery.routine.prompt";

/// A routine behind an editor sheet, or behind a confirm to delete it.
pub(super) fn story(
    ui: &mut egui::Ui,
    story: StoryId,
    tokens: &DesignTokens,
    scale: f32,
    state: &mut DialogFixtureState,
    report: &mut StoryReport,
) {
    if !state.seeded {
        state.seeded = true;
        state.open = true;
    }
    let mut p = PresentationContext::new(
        ui,
        *tokens,
        scale,
        PresentationScope::new("gallery.routine"),
        SemanticUiId::new("gallery.story"),
    );
    let opener = if story == StoryId::DialogSheet {
        RoutineAction::Edit
    } else {
        RoutineAction::Delete
    };
    p.content(ui, "name", &state.name, HEADING);
    p.content(ui, "schedule", SCHEDULE, DETAIL);
    if state.deleted {
        p.content(ui, "deleted", "The routine was deleted.", DETAIL);
    }
    ui.add_space(tokens.spacing.block.0);
    if button(ui, &mut p, opener, ActionEmphasis::Normal, !state.deleted).clicked() {
        state.open = true;
        report.repaint = Some("dialog opened");
    }
    if state.open {
        let changed = if story == StoryId::DialogSheet {
            editor(ui, &mut p, state)
        } else {
            confirm(ui, &mut p, state)
        };
        if changed {
            report.repaint = Some("dialog changed");
        }
    }
    report.presented(&p.finish(ui));
}

fn button(
    ui: &mut egui::Ui,
    p: &mut PresentationContext,
    action: RoutineAction,
    emphasis: ActionEmphasis,
    enabled: bool,
) -> egui::Response {
    p.action(
        ui,
        action,
        ActionButtonSpec {
            target: ActionTarget::application(action),
            availability: if enabled {
                Availability::Enabled
            } else {
                Availability::Disabled {
                    reason: "The routine was deleted".into(),
                }
            },
            state: ActionButtonState::Momentary,
            emphasis,
            compact: false,
        },
    )
}

/// The routine editor: a sheet on a phone and a centred dialog on a desktop.
/// Returns whether the state changed.
fn editor(ui: &mut egui::Ui, p: &mut PresentationContext, state: &mut DialogFixtureState) -> bool {
    let mut changed = false;
    // Save sits in the sheet's fixed footer, in reach however far the form
    // scrolls.
    let spec = DialogSpec::new("Edit routine", RoutineAction::Close)
        .sheet()
        .footer(RoutineAction::Save);
    let response = p.dialog(
        ui,
        "editor",
        &spec,
        |_, _| Availability::Enabled,
        |ui, p| {
            let gap = p.tokens().spacing.block.0;
            let name = TextFieldSpec::new("Name").id(NAME_FIELD);
            changed |= p.text_field(ui, NAME_FIELD, &name, &mut state.name).changed;
            ui.add_space(gap);
            let prompt = TextFieldSpec::new("Prompt").id(PROMPT_FIELD).multiline(3);
            changed |= p
                .text_field(ui, PROMPT_FIELD, &prompt, &mut state.prompt)
                .changed;
            ui.add_space(gap);
            p.content(ui, "schedule-label", "Schedule", CAPTION);
            p.content(ui, "schedule", SCHEDULE, DETAIL);
            ui.add_space(gap);
            let none = ActionArgument::None;
            if p.switch(
                ui,
                RoutineAction::Notify,
                SwitchSpec {
                    label: "Send a notification",
                    on: state.notify,
                },
                ActionTarget::application(RoutineAction::Notify),
                &Availability::Enabled,
                &none,
            )
            .clicked()
            {
                state.notify = !state.notify;
                changed = true;
            }
        },
    );
    if response.footer_pressed {
        state.saved += 1;
        state.open = false;
        changed = true;
    }
    if response.dismissed {
        state.open = false;
        changed = true;
    }
    changed
}

/// A confirm before deleting the routine. Returns whether it closed.
fn confirm(ui: &mut egui::Ui, p: &mut PresentationContext, state: &mut DialogFixtureState) -> bool {
    let title = format!("Delete \u{201c}{}\u{201d}?", state.name);
    let response = p.confirm(
        ui,
        "delete",
        &ConfirmSpec::new(
            &title,
            "The routine stops running and its history is removed. This can\u{2019}t be undone.",
            RoutineAction::ConfirmDelete,
            RoutineAction::Keep,
        )
        .confirm_emphasis(ActionEmphasis::Destructive),
        |_, _| Availability::Enabled,
    );
    if response.confirmed {
        state.deleted = true;
    }
    if response.confirmed || response.cancelled {
        state.open = false;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use stobie_ui_egui::{
        ApplicationTheme, DensityVariant, ThemeVariant, UiRole, audit_accesskit, audit_text_layouts,
    };

    use super::*;
    use crate::stories::probe::{StoryProbe, render};

    fn probe(
        story: StoryId,
        variant: ThemeVariant,
        scale: f32,
        size: egui::Vec2,
    ) -> (stobie_ui_egui::UiSnapshot, egui::FullOutput) {
        let context = egui::Context::default();
        context.enable_accesskit();
        render(
            &context,
            StoryProbe {
                story,
                tokens: DesignTokens::resolve(variant, DensityVariant::Comfortable),
                scale,
                clip: None,
                theme: ApplicationTheme::analytical(),
                preferences: Default::default(),
                heading: false,
            },
            size,
        )
    }

    fn names(snapshot: &stobie_ui_egui::UiSnapshot) -> Vec<(UiRole, &str)> {
        snapshot
            .nodes
            .iter()
            .skip(1)
            .map(|node| (node.role, node.name.as_str()))
            .collect()
    }

    #[test]
    fn the_sheet_holds_the_form_and_hides_the_routine_behind_it() {
        for size in [egui::vec2(1440.0, 900.0), egui::vec2(390.0, 844.0)] {
            let (snapshot, mut output) = probe(StoryId::DialogSheet, ThemeVariant::Dark, 1.0, size);
            assert_eq!(
                names(&snapshot),
                [
                    (UiRole::Dialog, "Edit routine"),
                    (UiRole::TextInput, "Name"),
                    (UiRole::MultilineTextInput, "Prompt"),
                    (UiRole::Switch, "Send a notification"),
                    (UiRole::Button, "Save"),
                    (UiRole::Button, "Close"),
                ],
                "{size:?}"
            );
            assert!(
                snapshot.semantic_audit.is_empty(),
                "{:?}",
                snapshot.semantic_audit
            );
            assert!(snapshot.text_audit.is_empty(), "{:#?}", snapshot.text_audit);
            let update = output.platform_output.accesskit_update.take().unwrap();
            let findings = audit_accesskit(&snapshot, &update);
            assert!(findings.is_empty(), "{findings:?}");
            output.textures_delta.clear();
        }
    }

    #[test]
    fn the_confirm_offers_keep_then_delete() {
        let (snapshot, mut output) = probe(
            StoryId::DialogConfirm,
            ThemeVariant::LightHighContrast,
            1.5,
            egui::vec2(390.0, 844.0),
        );
        assert_eq!(
            names(&snapshot),
            [
                (
                    UiRole::AlertDialog,
                    "Delete \u{201c}Morning briefing\u{201d}?"
                ),
                (UiRole::Button, "Keep routine"),
                (UiRole::Button, "Delete"),
            ]
        );
        assert!(audit_text_layouts(&snapshot.text).is_empty());
        let update = output.platform_output.accesskit_update.take().unwrap();
        assert!(audit_accesskit(&snapshot, &update).is_empty());
        output.textures_delta.clear();
    }

    /// Focus moves into the sheet's first field when it opens, the field asks
    /// the platform for text input (the browser's text agent follows it), and
    /// typed text reaches it.
    #[test]
    fn typing_reaches_the_name_field_once_the_sheet_opens() {
        let context = egui::Context::default();
        context.enable_accesskit();
        let probe = StoryProbe {
            story: StoryId::DialogSheet,
            tokens: DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable),
            scale: 1.0,
            clip: None,
            theme: ApplicationTheme::analytical(),
            preferences: Default::default(),
            heading: true,
        };
        let mut host = stobie_ui_egui::AppHost::new(&context, probe);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1440.0, 900.0));
        let mut asked_for_text = false;
        for events in [
            vec![],
            vec![],
            vec![],
            vec![egui::Event::Text(" at dawn".into())],
            vec![],
        ] {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ui| host.present(ui),
            );
            output.textures_delta.clear();
            asked_for_text |= output.platform_output.ime.is_some();
        }
        assert!(asked_for_text, "the focused field asks for text input");
        assert_eq!(host.ui_state().dialog.name, "Morning briefing at dawn");
    }
}
