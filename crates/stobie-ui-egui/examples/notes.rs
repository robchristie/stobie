//! A complete hosted Stobie application: a list of notes with a draft field.
//!
//! Run it with `cargo run -p stobie-ui-egui --example notes`. Set
//! `STOBIE_AUTOMATION_SOCKET=/absolute/path.sock` to inspect and invoke it with
//! `tools/application-cli.mjs`; the application contains no inspection code.

use serde::Serialize;
use stobie_core::{PaneId, Workspace, WorkspaceChange};
use stobie_ui_egui::{
    Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability,
    ContentTextSpec, Frame, NativeTextControlKind, Observation, Panes, PresentationScope,
    SemanticUiId, TextInteraction, TextOverflow, TextRole, UiNode, UiRole, application_identity,
};

const NOTES_PANE: PaneId = PaneId(1);

// 1. Declare every user action once. IDs default to `notes.<kebab-case>`.
#[derive(Action, Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[action(prefix = "notes")]
enum NoteAction {
    #[action(
        label = "Add note",
        description = "Add the draft as a note",
        shortcut = "Cmd+Enter"
    )]
    Add,
    #[action(
        label = "Remove last",
        description = "Remove the newest note",
        shortcut = "Cmd+Z"
    )]
    RemoveLast,
    #[action(
        label = "Clear",
        description = "Remove every note",
        shortcut = "Cmd+Shift+K"
    )]
    Clear,
}

// 2. Data changes the UI emits. Actions are not intents; they have their own route.
enum NoteIntent {
    EditDraft(String),
    Layout(WorkspaceChange),
}

// The dock emits tab, move and resize changes as intents.
impl From<WorkspaceChange> for NoteIntent {
    fn from(change: WorkspaceChange) -> Self {
        Self::Layout(change)
    }
}

// 3. Application state. The UI only reads it. Notes keep stable IDs so their
// controls keep their identity as the list changes.
struct Note {
    id: u64,
    text: String,
}

struct Notes {
    notes: Vec<Note>,
    next_id: u64,
    draft: String,
    status: String,
    workspace: Workspace,
}

impl Notes {
    fn new() -> Self {
        let mut notes = Self {
            notes: Vec::new(),
            next_id: 1,
            draft: String::new(),
            status: String::new(),
            workspace: Workspace::with_tabs(&[NOTES_PANE]),
        };
        for text in ["Water the plants", "Call the library"] {
            notes.add(text.to_owned());
        }
        notes
    }

    fn add(&mut self, text: String) {
        self.notes.push(Note {
            id: self.next_id,
            text,
        });
        self.next_id += 1;
        self.status = format!("{} notes", self.notes.len());
    }
}

impl Application for Notes {
    type Action = NoteAction;
    type Intent = NoteIntent;
    type UiState = ();

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("Notes")
    }

    // One rule serves buttons, shortcuts, discovery and semantic invocation.
    fn availability(&self, target: ActionTarget<NoteAction>) -> Availability {
        let reason = match target.action {
            NoteAction::Add if self.draft.trim().is_empty() => Some("Type a note first"),
            NoteAction::RemoveLast | NoteAction::Clear if self.notes.is_empty() => {
                Some("There are no notes")
            }
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

    // Clicks, shortcuts and agents all arrive here, after an availability check.
    fn perform(&mut self, target: ActionTarget<NoteAction>, _: &mut ()) -> Result<bool, String> {
        match target.action {
            NoteAction::Add => {
                let text = std::mem::take(&mut self.draft).trim().to_owned();
                self.add(text);
            }
            NoteAction::RemoveLast => {
                self.notes.pop();
                self.status = "Removed the newest note".into();
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

    // Removing "the last note" means a different note once the list changes.
    fn meaning(&self, target: ActionTarget<NoteAction>) -> String {
        match target.action {
            NoteAction::RemoveLast => format!("notes:{}", self.notes.len()),
            _ => "stable".into(),
        }
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("notes", self.notes.len());
        observation.fact("draft", self.draft.as_str());
    }
}

// 4. The pane: borrowed fields, the frame for output.
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
        // Recipes measure text and record semantics; the host collects them.
        let mut p = frame.pane_presentation(ui, PresentationScope::new("notes"), pane);
        p.heading(ui, "title", "Notes");
        // Edit a copy and emit it; the model changes only in `apply`.
        let mut draft = self.draft.to_owned();
        let (field, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
            (
                ui.add(egui::TextEdit::singleline(&mut draft).hint_text("New note")),
                (),
            )
        });
        // Custom widgets add their own semantics.
        let mut node = UiNode::container(
            SemanticUiId::new("notes.draft"),
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
            frame.action(ui, &mut p, NoteAction::RemoveLast, ActionEmphasis::Normal);
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
        // A status line agents can wait for with the `status` condition.
        p.status(ui, "status", self.status, false);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    stobie_ui_egui::run_native("Notes", eframe::NativeOptions::default(), |_| Notes::new())
}

// A browser build exports `stobie_ui_egui::web_handle!` from a library instead.
#[cfg(target_arch = "wasm32")]
fn main() {
    let _ = (Notes::new, stobie_ui_egui::AppHost::<Notes>::new);
}
