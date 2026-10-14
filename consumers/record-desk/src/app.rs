use egui::Ui;
use serde::Serialize;
use stobie_core::{DockNode, SplitAxis, Workspace, WorkspaceChange};
use stobie_ui_egui::*;

use crate::{actions::Action, model::*, panes::*, store::Store};

/// The hosted application: domain state, persistence feedback and layout.
pub struct RecordDesk {
    pub(crate) desk: Desk,
    pub(crate) workspace: Workspace,
    store: Store,
    /// The layout of the last save or restore; `None` before the first save.
    saved_layout: Option<Workspace>,
    load_error: bool,
    message: String,
    error: bool,
    narrow_layout: bool,
    desk_epoch: u64,
    snapshot: Snapshot,
}

pub type RecordDeskApp = AppHost<RecordDesk>;

/// UI-only state: a pending search-focus request.
#[derive(Default)]
pub struct View {
    focus_search: bool,
}

/// Read-only test observation for the native and browser smokes. It records
/// the state a pass presented, so its geometry and state always agree.
#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    /// Tab keydowns observed by the host, counted once per input frame.
    pub tab_input_epoch: u64,
    pub ui: UiSnapshot,
    pub records: Vec<Record>,
    pub visible_ids: Vec<RecordId>,
    pub selected: Option<RecordId>,
    pub draft: Option<Record>,
    pub filters: Filters,
    pub undo_entries: usize,
    pub redo_entries: usize,
    pub draft_dirty: bool,
    pub unsaved: bool,
    pub save_blocked: bool,
    pub message: String,
    pub error: bool,
    pub workspace: Option<Workspace>,
}

impl RecordDesk {
    pub fn new(creation: &eframe::CreationContext<'_>) -> Self {
        if let Some(render_state) = &creation.wgpu_render_state {
            eprintln!("Record Desk adapter: {:?}", render_state.adapter.get_info());
        }
        Self::with_store(Store::new())
    }

    pub fn with_store(store: Store) -> Self {
        let mut desk = Self {
            desk: Desk::default(),
            workspace: default_workspace(),
            store,
            saved_layout: None,
            load_error: false,
            message:
                "Synthetic records ready. Apply commits a draft; Save persists records and layout."
                    .into(),
            error: false,
            narrow_layout: false,
            desk_epoch: 0,
            snapshot: Snapshot::default(),
        };
        desk.load();
        desk
    }

    /// A hosted desk, as the native and browser entry points run it.
    pub fn hosted(context: &egui::Context, store: Store) -> RecordDeskApp {
        AppHost::new(context, Self::with_store(store))
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }

    fn state(&self) -> SavedState {
        SavedState::from_desk(&self.desk, &self.workspace)
    }

    fn unsaved(&self) -> bool {
        !self.desk.is_saved() || self.saved_layout.as_ref() != Some(&self.workspace)
    }

    fn load(&mut self) {
        match self.store.load() {
            Ok(Some(saved)) => match Desk::new(saved.records.clone()) {
                Ok(desk) => {
                    self.desk = desk;
                    self.desk_epoch += 1;
                    self.workspace = saved.workspace.clone();
                    self.saved_layout = Some(saved.workspace);
                    self.load_error = false;
                    self.message =
                        "Restored committed records and layout. Undo history starts empty.".into();
                    self.error = false;
                }
                Err(error) => self.report_error(error),
            },
            Ok(None) => {
                self.desk = Desk::default();
                self.desk_epoch += 1;
                self.workspace = default_workspace();
                self.saved_layout = None;
                self.load_error = false;
                self.message = "No saved state yet. Synthetic records are ready to review.".into();
                self.error = false;
            }
            Err(error) => {
                self.load_error = true;
                self.report_error(format!("Restore failed: {error}. Saved data is preserved. Repair or move it outside the app, then use Restore."));
            }
        }
    }

    fn layout_axis(&self) -> String {
        match &self.workspace.root {
            DockNode::Split { axis, .. } => format!("{axis:?}"),
            _ => "unsplit".into(),
        }
    }
}

pub enum Intent {
    Select(RecordId),
    Filters(Filters),
    Draft(Record),
    Layout(WorkspaceChange),
}

impl From<WorkspaceChange> for Intent {
    fn from(change: WorkspaceChange) -> Self {
        Self::Layout(change)
    }
}

impl Application for RecordDesk {
    type Action = Action;
    type Intent = Intent;
    type UiState = View;

    fn identity(&self) -> ApplicationIdentity {
        application_identity!("Record Desk")
    }

    fn availability(&self, target: ActionTarget<Action>) -> Availability {
        let action = target.action;
        if matches!(action, Action::ToggleReviewed | Action::EditCategory)
            && self.desk.draft().is_none()
        {
            return Availability::Hidden;
        }
        let reason = match action {
            Action::Apply | Action::Cancel if !self.desk.is_dirty() => {
                Some("There are no draft changes")
            }
            Action::Undo | Action::Redo if self.desk.is_dirty() => {
                Some("Apply or cancel the draft first")
            }
            Action::Undo if self.desk.undo_len() == 0 => {
                Some("There is no committed change to undo")
            }
            Action::Redo if self.desk.redo_len() == 0 => Some("There is no change to redo"),
            Action::Save if self.load_error => Some("Repair the saved data and use Restore first"),
            Action::ResetFilters if *self.desk.filters() == Filters::default() => {
                Some("There are no active filters")
            }
            Action::Arrange
                if self.narrow_layout && matches!(self.workspace.root, DockNode::Split { .. }) =>
            {
                Some("Narrow windows keep split panes stacked")
            }
            Action::Restore if self.desk.is_dirty() || (!self.load_error && self.unsaved()) => {
                Some("Save committed changes and cancel or apply the draft first")
            }
            _ => None,
        };
        reason.map_or(Availability::Enabled, |reason| Availability::Disabled {
            reason: reason.into(),
        })
    }

    // Guard assumptions that affect meaning, not focus or repaint counters.
    fn meaning(&self, target: ActionTarget<Action>) -> String {
        let epoch = self.desk_epoch;
        match target.action {
            Action::Apply | Action::Cancel | Action::ToggleReviewed => format!(
                "{epoch}:draft:{}:history:{}",
                self.desk.draft_revision(),
                self.desk.history_revision()
            ),
            Action::Undo | Action::Redo => {
                format!("{epoch}:history:{}", self.desk.history_revision())
            }
            Action::ResetFilters => format!("{epoch}:filters:{}", self.desk.filter_revision()),
            Action::Arrange => self.layout_axis(),
            _ => "stable".into(),
        }
    }

    fn prepare(&mut self, pass: &mut PassContext<'_>, _: &mut View) {
        // Adapt the authoritative tree itself; never create a second docking model.
        self.narrow_layout = pass.available_rect().width() < 640.0;
        if self.narrow_layout
            && let DockNode::Split { axis, .. } = &mut self.workspace.root
        {
            *axis = SplitAxis::Vertical;
        }
    }

    fn ui(&self, root: &mut Ui, view: &mut View, frame: &mut Frame<'_, Action, Intent>) {
        // As before hosting: publish only what is visible, including the top bar's
        // first frame, which egui clips until the panel has measured its content.
        stobie_ui_egui::observe_visible_only(root, |root| {
            egui::Panel::top("record-desk-bar").show(root, |ui| {
                let scope = PresentationScope::new("record-desk.chrome");
                let mut p = frame.presentation(ui, scope, SemanticUiId::root());
                ui.horizontal_wrapped(|ui| p.heading(ui, "title", "Record Desk"));
                ui.horizontal_wrapped(|ui| {
                    for action in [
                        Action::Undo,
                        Action::Redo,
                        Action::Save,
                        Action::Restore,
                        Action::Arrange,
                    ] {
                        frame.action(ui, &mut p, action, ActionEmphasis::Normal);
                    }
                });
                let state = format!(
                    "{} · {} · {}",
                    if self.desk.is_dirty() {
                        "Editing draft"
                    } else {
                        "No draft changes"
                    },
                    if self.unsaved() {
                        "Unsaved committed state/layout"
                    } else {
                        "Saved"
                    },
                    self.store.description()
                );
                p.content(ui, "state", &state, content_spec(TextRole::Secondary, 2));
            });
            egui::Panel::bottom("record-desk-status").show(root, |ui| {
                let scope = PresentationScope::new("record-desk.feedback");
                let mut p = frame.presentation(ui, scope, SemanticUiId::root());
                p.status(ui, "message", &self.message, self.error);
            });
            let mut panes = RecordPanes {
                records: self.desk.visible(),
                total: self.desk.records().len(),
                selected: self.desk.selected(),
                draft: self.desk.draft(),
                filters: self.desk.filters(),
                dirty: self.desk.is_dirty(),
                tokens: frame.tokens(),
                focus_search: std::mem::take(&mut view.focus_search),
            };
            egui::CentralPanel::default()
                .show(root, |ui| frame.dock(ui, &self.workspace, &mut panes));
        });
    }

    // Draft/filter outputs from a pass apply before a selection change and the
    // actions that consume them; the sort keeps emission order otherwise.
    fn order_intents(&self, intents: &mut [Intent]) {
        intents.sort_by_key(|intent| !matches!(intent, Intent::Draft(_) | Intent::Filters(_)));
    }

    fn apply(&mut self, intent: Intent, _: &mut View) -> Result<bool, String> {
        match intent {
            Intent::Select(id) => self.desk.select(id)?,
            Intent::Filters(filters) => self.desk.set_filters(filters),
            Intent::Draft(draft) => self.desk.edit_draft(draft)?,
            Intent::Layout(change) => return change.apply(&mut self.workspace),
        }
        Ok(true)
    }

    fn perform(&mut self, target: ActionTarget<Action>, view: &mut View) -> Result<bool, String> {
        let action = target.action;
        match action {
            Action::Apply => {
                self.message = if self.desk.apply()? {
                    "Applied one record change. Save to keep it after restart."
                } else {
                    "Draft matches the committed record. No history entry was added."
                }
                .into();
            }
            Action::Cancel => {
                self.desk.cancel();
                self.message = "Draft cancelled. Committed records are unchanged.".into();
            }
            Action::Undo => {
                self.desk.undo()?;
                self.message = "Undid one record change.".into();
            }
            Action::Redo => {
                self.desk.redo()?;
                self.message = "Redid one record change.".into();
            }
            Action::Save => {
                let saved = self.state();
                self.store.save(&saved)?;
                self.desk.mark_saved();
                self.saved_layout = Some(saved.workspace);
                self.message = if self.desk.is_dirty() {
                    "Saved committed records and layout. The editing draft was excluded."
                } else {
                    "Saved committed records and layout locally."
                }
                .into();
            }
            Action::Restore => self.load(),
            Action::Search => {
                self.workspace.activate(LIST_PANE);
                view.focus_search = true;
            }
            Action::ResetFilters => {
                self.desk.set_filters(Filters::default());
                self.message = "Filters reset. Selection and editing draft are unchanged.".into();
            }
            Action::Arrange => {
                if let DockNode::Split { axis, .. } = &mut self.workspace.root {
                    *axis = match axis {
                        SplitAxis::Horizontal => SplitAxis::Vertical,
                        SplitAxis::Vertical => SplitAxis::Horizontal,
                    };
                } else {
                    self.workspace = default_workspace();
                }
                self.message = "Pane arrangement changed. Save retains the layout.".into();
            }
            Action::ToggleReviewed => {
                let mut draft = self.desk.draft().ok_or("Choose a record first")?.clone();
                draft.reviewed = !draft.reviewed;
                self.desk.edit_draft(draft)?;
            }
            Action::FilterCategory | Action::FilterReview | Action::EditCategory => {
                return Ok(false);
            }
        }
        // Restore reports its own outcome; toggling edits only the draft.
        if !matches!(action, Action::Restore | Action::ToggleReviewed) {
            self.error = false;
        }
        Ok(true)
    }

    fn report_error(&mut self, message: String) {
        self.message = message;
        self.error = true;
    }

    fn observe(&self, observation: &mut Observation<'_>) {
        let text = |value: Option<String>| value.unwrap_or_default();
        observation.fact("undo_entries", self.desk.undo_len() as u64);
        observation.fact("redo_entries", self.desk.redo_len() as u64);
        observation.fact("draft_dirty", self.desk.is_dirty());
        observation.fact("status", self.message.as_str());
        observation.fact("error", self.error);
        let selected = self.desk.selected().map(|id| id.0.to_string());
        observation.fact("selected_record", text(selected));
        let title = self.desk.draft().map(|record| record.title.clone());
        observation.fact("draft_title", text(title));
        observation.fact("layout_axis", self.layout_axis());
        let total = self.desk.records().len() as u64;
        observation.counted_collection("records", UiRole::ResultRow, total);
    }

    fn completed(&mut self, pass: &CompletedPass<'_>, _: &View) {
        self.snapshot = Snapshot {
            tab_input_epoch: pass.tab_presses,
            ui: pass.snapshot.clone(),
            records: self.desk.records().to_vec(),
            visible_ids: self.desk.visible().iter().map(|r| r.id).collect(),
            selected: self.desk.selected(),
            draft: self.desk.draft().cloned(),
            filters: self.desk.filters().clone(),
            undo_entries: self.desk.undo_len(),
            redo_entries: self.desk.redo_len(),
            draft_dirty: self.desk.is_dirty(),
            unsaved: self.unsaved(),
            save_blocked: self.load_error,
            message: self.message.clone(),
            error: self.error,
            workspace: Some(self.workspace.clone()),
        };
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = std::env::var_os("RECORD_DESK_SNAPSHOT") {
            let _ = std::fs::write(path, serde_json::to_vec(&self.snapshot).unwrap());
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "app_tests.rs"]
mod tests;
