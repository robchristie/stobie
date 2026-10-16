//! {{title}}: a Stobie application. It declares its state, actions, UI and how
//! it applies changes; the framework host owns observation, shortcuts, agent
//! inspection and repaint. The same code runs natively and in the browser.

use serde::Serialize;
use stobie_core::{PaneId, Workspace, WorkspaceChange};
use stobie_ui_egui::{
    Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability,
    ContentTextSpec, Frame, NativeTextControlKind, Observation, Panes, PresentationScope,
    SemanticUiId, TextInteraction, TextOverflow, TextRole, UiNode, UiRole, application_identity,
};

const NOTES_PANE: PaneId = PaneId(1);

/// Every user action, declared once. Buttons, shortcuts and agents share
/// them; IDs are `{{name}}.<kebab-case variant>`, such as `{{name}}.add`.
#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "{{name}}")]
pub enum NoteAction {
    #[action(
        label = "Add note",
        description = "Add the draft as a note",
        shortcut = "Cmd+Enter"
    )]
    Add,
    #[action(
        label = "Clear",
        description = "Remove every note",
        shortcut = "Cmd+Shift+K"
    )]
    Clear,
}

/// Data changes the UI emits. Actions have their own route.
pub enum NoteIntent {
    EditDraft(String),
    Layout(WorkspaceChange),
}

impl From<WorkspaceChange> for NoteIntent {
    fn from(change: WorkspaceChange) -> Self {
        Self::Layout(change)
    }
}

/// A note keeps a stable ID, so its controls keep their identity as the list
/// changes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Note {
    pub id: u64,
    pub text: String,
}

/// The application state. The UI only reads it; `apply` and `perform` change it.
pub struct App {
    notes: Vec<Note>,
    next_id: u64,
    draft: String,
    status: String,
    workspace: Workspace,
}

impl Default for App {
    fn default() -> Self {
        let mut app = Self {
            notes: Vec::new(),
            next_id: 1,
            draft: String::new(),
            status: String::new(),
            workspace: Workspace::with_tabs(&[NOTES_PANE]),
        };
        app.push("Try `stobie observe`".into());
        app.status = app.count_status();
        app
    }
}

impl App {
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    fn push(&mut self, text: String) {
        self.notes.push(Note {
            id: self.next_id,
            text,
        });
        self.next_id += 1;
    }

    fn count_status(&self) -> String {
        match self.notes.len() {
            1 => "1 note".into(),
            count => format!("{count} notes"),
        }
    }
}

impl Application for App {
    type Action = NoteAction;
    type Intent = NoteIntent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("{{title}}")
    }

    /// One rule serves buttons, shortcuts, discovery and agents.
    fn availability(&self, target: ActionTarget<NoteAction>) -> Availability {
        let reason = match target.action {
            NoteAction::Add if self.draft.trim().is_empty() => Some("Type a note first"),
            NoteAction::Clear if self.notes.is_empty() => Some("There are no notes"),
            _ => None,
        };
        reason.map_or(Availability::Enabled, |reason| Availability::Disabled {
            reason: reason.into(),
        })
    }

    fn ui(&self, ui: &mut egui::Ui, _: &mut (), frame: &mut Frame<'_, NoteAction, NoteIntent>) {
        // Panes get a narrow read view, never the application.
        let mut panes = NotePanes {
            notes: &self.notes,
            draft: &self.draft,
            status: &self.status,
        };
        egui::CentralPanel::default().show(ui, |ui| frame.dock(ui, &self.workspace, &mut panes));
    }

    fn apply(&mut self, intent: NoteIntent, _: &mut ()) -> Result<bool, String> {
        match intent {
            NoteIntent::EditDraft(draft) => self.draft = draft,
            NoteIntent::Layout(change) => return change.apply(&mut self.workspace),
        }
        Ok(true)
    }

    /// Clicks, shortcuts and agents all arrive here, after an availability check.
    fn perform(&mut self, target: ActionTarget<NoteAction>, _: &mut ()) -> Result<bool, String> {
        match target.action {
            NoteAction::Add => {
                let text = self.draft.trim().to_owned();
                self.push(text);
                self.draft.clear();
                self.status = self.count_status();
            }
            NoteAction::Clear => {
                self.notes.clear();
                self.status = "Cleared every note".into();
            }
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.status = message;
    }

    /// Bounded facts that agents read with `stobie observe` and wait for.
    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("notes", self.notes.len());
        observation.fact("draft", self.draft.as_str());
    }
}

/// The pane: borrowed fields in, intents and actions out through the frame.
struct NotePanes<'a> {
    notes: &'a [Note],
    draft: &'a str,
    status: &'a str,
}

impl Panes<NoteAction, NoteIntent> for NotePanes<'_> {
    fn title(&self, _pane: PaneId) -> &'static str {
        "Notes"
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        pane: PaneId,
        frame: &mut Frame<'_, NoteAction, NoteIntent>,
    ) {
        let mut p = frame.pane_presentation(ui, PresentationScope::new("notes"), pane);
        p.heading(ui, "title", "Notes");
        // Edit a copy and emit it; the state changes only in `apply`.
        let mut draft = self.draft.to_owned();
        let (field, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
            (
                ui.add(egui::TextEdit::singleline(&mut draft).hint_text("New note")),
                (),
            )
        });
        // Observed as a text input, the field accepts `stobie set-text {{name}}.draft TEXT`.
        let mut node = UiNode::container(
            SemanticUiId::new("{{name}}.draft"),
            Some(SemanticUiId::pane(pane)),
            UiRole::TextInput,
            field.rect.into(),
        );
        node.name = "New note".into();
        node.focused = field.has_focus();
        p.observe_node(ui, node);
        if draft != self.draft {
            frame.emit(NoteIntent::EditDraft(draft));
        }
        ui.horizontal(|ui| {
            frame.action(ui, &mut p, NoteAction::Add, ActionEmphasis::Primary);
            frame.action(ui, &mut p, NoteAction::Clear, ActionEmphasis::Normal);
        });
        let text = ContentTextSpec {
            role: TextRole::Body,
            overflow: TextOverflow::Wrap,
            max_lines: 3,
            interaction: TextInteraction::Selectable,
        };
        for note in self.notes {
            p.content(ui, ("note", note.id), &note.text, text);
        }
        // A status line agents can wait for with `stobie wait status`.
        p.status(ui, "status", self.status, false);
    }
}

// The browser entry point: `stobie build-web` builds this library for the web.
#[cfg(target_arch = "wasm32")]
stobie_ui_egui::web_handle!(App, |_| App::default());

#[cfg(test)]
mod tests {
    use super::*;

    fn run(app: &mut App, action: NoteAction) {
        assert!(app.availability(ActionTarget::application(action)).enabled());
        app.perform(ActionTarget::application(action), &mut ())
            .unwrap();
    }

    #[test]
    fn add_needs_a_draft_and_clears_it() {
        let mut app = App::default();
        assert!(
            !app.availability(ActionTarget::application(NoteAction::Add))
                .enabled()
        );
        app.apply(NoteIntent::EditDraft("  Buy stamps ".into()), &mut ())
            .unwrap();
        run(&mut app, NoteAction::Add);
        assert_eq!(app.notes().last().unwrap().text, "Buy stamps");
        assert_eq!((app.notes().len(), app.draft.as_str()), (2, ""));
    }

    #[test]
    fn clear_removes_every_note_and_then_is_unavailable() {
        let mut app = App::default();
        run(&mut app, NoteAction::Clear);
        assert!(app.notes().is_empty());
        assert!(
            !app.availability(ActionTarget::application(NoteAction::Clear))
                .enabled()
        );
    }
}
