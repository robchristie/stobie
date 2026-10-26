//! A complete hosted Stobie application: a list of notes with a draft field.
//!
//! Run it with `cargo run -p stobie-ui-egui --example notes`. Set
//! `STOBIE_AUTOMATION_SOCKET=/absolute/path.sock` to inspect and invoke it with
//! the `stobie` command; the application contains no inspection code.
//!
//! Adding, removing and clearing notes are undoable. Clear removes each note as
//! a separate step but records them as one history entry. The draft is not part
//! of history.

use serde::Serialize;
use stobie_core::{Command, History, PaneId, Workspace, WorkspaceChange};
use stobie_ui_egui::{
    Action, ActionEmphasis, ActionTarget, Application, ApplicationIdentity, Availability,
    ContentTextSpec, Frame, Observation, Panes, PresentationScope, TextFieldSpec, TextInteraction,
    TextOverflow, TextRole, application_identity,
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
        shortcut = "Cmd+Backspace"
    )]
    RemoveLast,
    #[action(
        label = "Clear",
        description = "Remove every note",
        shortcut = "Cmd+Shift+K"
    )]
    Clear,
    #[action(
        label = "Undo",
        description = "Reverse the last change to the notes",
        shortcut = "Cmd+Z"
    )]
    Undo,
    #[action(
        label = "Redo",
        description = "Reapply the last undone change",
        shortcut = "Cmd+Shift+Z"
    )]
    Redo,
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
#[derive(Clone)]
struct Note {
    id: u64,
    text: String,
}

// One reversible step on the note list. Each command carries everything it
// needs in both directions, including the note's stable ID, so redo restores
// the same note rather than a new one. Every change to `notes` goes through
// this type: an edit made outside history would shift the indices it records.
enum NoteEdit {
    Insert { index: usize, note: Note },
    Remove { index: usize, note: Note },
}

impl NoteEdit {
    fn insert(notes: &mut Vec<Note>, index: usize, note: &Note) -> Result<(), String> {
        if index > notes.len() {
            return Err(format!("Cannot restore note {} at {index}", note.id));
        }
        notes.insert(index, note.clone());
        Ok(())
    }

    fn remove(notes: &mut Vec<Note>, index: usize, note: &Note) -> Result<(), String> {
        // Check before changing anything: an error must leave the list as it was.
        if notes.get(index).map(|found| found.id) != Some(note.id) {
            return Err(format!("Note {} is no longer at {index}", note.id));
        }
        notes.remove(index);
        Ok(())
    }
}

impl Command<Vec<Note>> for NoteEdit {
    fn apply(&self, notes: &mut Vec<Note>) -> Result<bool, String> {
        match self {
            Self::Insert { index, note } => Self::insert(notes, *index, note)?,
            Self::Remove { index, note } => Self::remove(notes, *index, note)?,
        }
        Ok(true)
    }

    fn revert(&self, notes: &mut Vec<Note>) -> Result<(), String> {
        match self {
            Self::Insert { index, note } => Self::remove(notes, *index, note),
            Self::Remove { index, note } => Self::insert(notes, *index, note),
        }
    }
}

struct Notes {
    notes: Vec<Note>,
    next_id: u64,
    draft: String,
    status: String,
    workspace: Workspace,
    // Completed note edits only; the draft and layout stay out of history.
    history: History<NoteEdit>,
}

impl Notes {
    fn new() -> Self {
        let mut notes = Self {
            notes: Vec::new(),
            next_id: 1,
            draft: String::new(),
            status: String::new(),
            workspace: Workspace::with_tabs(&[NOTES_PANE]),
            history: History::default(),
        };
        for text in ["Water the plants", "Call the library"] {
            let note = notes.new_note(text.to_owned());
            notes.notes.push(note);
        }
        notes.status = format!("{} notes", notes.notes.len());
        notes
    }

    fn new_note(&mut self, text: String) -> Note {
        let note = Note {
            id: self.next_id,
            text,
        };
        self.next_id += 1;
        note
    }

    fn count_status(&self) -> String {
        format!("{} notes", self.notes.len())
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
            NoteAction::Undo if !self.history.can_undo() => Some("There is nothing to undo"),
            NoteAction::Redo if !self.history.can_redo() => Some("There is nothing to redo"),
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
                let text = self.draft.trim().to_owned();
                let note = self.new_note(text);
                let index = self.notes.len();
                self.history
                    .execute(NoteEdit::Insert { index, note }, &mut self.notes)?;
                // Clearing the draft is not an edit to the notes, so it is not undone.
                self.draft.clear();
                self.status = self.count_status();
            }
            NoteAction::RemoveLast => {
                let index = self.notes.len() - 1;
                let note = self.notes[index].clone();
                self.history
                    .execute(NoteEdit::Remove { index, note }, &mut self.notes)?;
                self.status = "Removed the newest note".into();
            }
            NoteAction::Clear => {
                // One step per note, newest first, recorded as a single entry.
                let steps: Vec<NoteEdit> = self
                    .notes
                    .iter()
                    .enumerate()
                    .rev()
                    .map(|(index, note)| NoteEdit::Remove {
                        index,
                        note: note.clone(),
                    })
                    .collect();
                self.history.execute_group(steps, &mut self.notes)?;
                self.status = "Cleared every note".into();
            }
            NoteAction::Undo => {
                if !self.history.undo(&mut self.notes)? {
                    return Ok(false);
                }
                self.status = format!("Undone: {}", self.count_status());
            }
            NoteAction::Redo => {
                if !self.history.redo(&mut self.notes)? {
                    return Ok(false);
                }
                self.status = format!("Redone: {}", self.count_status());
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
            // What undo or redo would do changes with every history change.
            NoteAction::Undo | NoteAction::Redo => format!("history:{}", self.history.revision()),
            _ => "stable".into(),
        }
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        observation.fact("notes", self.notes.len());
        observation.fact("draft", self.draft.as_str());
        observation.fact("undo_steps", self.history.undo_len());
        observation.fact("redo_steps", self.history.redo_len());
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
        // The field edits a copy and emits it; the model changes only in
        // `apply`. Its ID is fixed so `stobie set-text notes.draft TEXT`
        // addresses it.
        frame.text_field(
            ui,
            &mut p,
            "draft",
            &TextFieldSpec::new("New note")
                .hint("What to remember")
                .id("notes.draft"),
            self.draft,
            NoteIntent::EditDraft,
        );
        ui.horizontal(|ui| {
            frame.action(ui, &mut p, NoteAction::Add, ActionEmphasis::Primary);
            frame.action(ui, &mut p, NoteAction::RemoveLast, ActionEmphasis::Normal);
            frame.action(ui, &mut p, NoteAction::Clear, ActionEmphasis::Normal);
        });
        ui.horizontal(|ui| {
            frame.action(ui, &mut p, NoteAction::Undo, ActionEmphasis::Normal);
            frame.action(ui, &mut p, NoteAction::Redo, ActionEmphasis::Normal);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(notes: &Notes) -> Vec<u64> {
        notes.notes.iter().map(|note| note.id).collect()
    }

    fn run(notes: &mut Notes, action: NoteAction) {
        notes
            .perform(ActionTarget::application(action), &mut ())
            .unwrap();
    }

    #[test]
    fn clear_undoes_as_one_step_and_redo_keeps_note_identity() {
        let mut notes = Notes::new();
        let original = ids(&notes);
        run(&mut notes, NoteAction::Clear);
        assert!(notes.notes.is_empty());
        assert_eq!(notes.history.undo_len(), 1);
        run(&mut notes, NoteAction::Undo);
        assert_eq!(ids(&notes), original);
        run(&mut notes, NoteAction::Redo);
        assert!(notes.notes.is_empty());
        run(&mut notes, NoteAction::Undo);
        assert_eq!(ids(&notes), original);
        assert!(
            !notes
                .availability(ActionTarget::application(NoteAction::Undo))
                .enabled()
        );
    }

    #[test]
    fn add_and_remove_undo_without_restoring_the_draft() {
        let mut notes = Notes::new();
        let original = ids(&notes);
        notes.draft = "Buy stamps".into();
        run(&mut notes, NoteAction::Add);
        assert_eq!((notes.notes.len(), notes.draft.as_str()), (3, ""));
        run(&mut notes, NoteAction::RemoveLast);
        run(&mut notes, NoteAction::RemoveLast);
        assert_eq!(notes.notes.len(), 1);
        run(&mut notes, NoteAction::Undo);
        run(&mut notes, NoteAction::Undo);
        assert_eq!(
            notes.notes.last().map(|note| note.text.as_str()),
            Some("Buy stamps")
        );
        run(&mut notes, NoteAction::Undo);
        assert_eq!((ids(&notes), notes.draft.as_str()), (original, ""));
        let before = notes.meaning(ActionTarget::application(NoteAction::Redo));
        run(&mut notes, NoteAction::Redo);
        assert_ne!(
            notes.meaning(ActionTarget::application(NoteAction::Redo)),
            before
        );
    }
}
