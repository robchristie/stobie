use egui::Ui;
use polyorama_core::{DockNode, SplitAxis, Workspace};
use polyorama_ui_egui::*;
use serde::Serialize;

use crate::{actions::Action, model::*, panes::*, store::Store};

#[path = "inspection.rs"]
mod inspection;

pub struct RecordDeskApp {
    pub(crate) desk: Desk,
    pub(crate) workspace: Workspace,
    dock: DockBehaviour,
    store: Store,
    persisted: Option<SavedState>,
    load_error: bool,
    message: String,
    error: bool,
    preferences: UiPreferences,
    narrow_layout: bool,
    focus_search: bool,
    frame: u64,
    snapshot: Snapshot,
    inspection: Option<Inspection>,
    desk_epoch: u64,
    #[cfg(not(target_arch = "wasm32"))]
    _inspection_host: Option<NativeInspectionHost>,
    #[cfg(target_arch = "wasm32")]
    inspection_context: egui::Context,
}

#[derive(Clone, Default, Serialize)]
pub struct Snapshot {
    pub ui: UiSnapshot,
    pub controls: Vec<Control>,
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

impl RecordDeskApp {
    pub fn new(creation: &eframe::CreationContext<'_>) -> Self {
        let app = Self::with_store(&creation.egui_ctx, Store::new());
        if let Some(render_state) = &creation.wgpu_render_state {
            eprintln!("Record Desk adapter: {:?}", render_state.adapter.get_info());
        }
        app
    }

    pub fn with_store(context: &egui::Context, store: Store) -> Self {
        let preferences = UiPreferences::default();
        apply_design_system(context, preferences);
        let mut app = Self {
            desk: Desk::default(),
            workspace: default_workspace(),
            dock: DockBehaviour::default(),
            store,
            persisted: None,
            load_error: false,
            message:
                "Synthetic records ready. Apply commits a draft; Save persists records and layout."
                    .into(),
            error: false,
            preferences,
            narrow_layout: false,
            focus_search: false,
            frame: 0,
            snapshot: Snapshot::default(),
            inspection: None,
            desk_epoch: 0,
            #[cfg(not(target_arch = "wasm32"))]
            _inspection_host: None,
            #[cfg(target_arch = "wasm32")]
            inspection_context: context.clone(),
        };
        app.load();
        #[cfg(not(target_arch = "wasm32"))]
        app.start_inspection(context);
        app
    }

    fn state(&self) -> SavedState {
        SavedState {
            schema_version: VERSION,
            records: self.desk.records().to_vec(),
            workspace: self.workspace.clone(),
        }
    }

    fn unsaved(&self) -> bool {
        self.persisted.as_ref() != Some(&self.state())
    }

    fn load(&mut self) {
        match self.store.load() {
            Ok(Some(saved)) => match Desk::new(saved.records.clone()) {
                Ok(desk) => {
                    self.desk = desk;
                    self.desk_epoch += 1;
                    self.workspace = saved.workspace.clone();
                    self.persisted = Some(saved);
                    self.load_error = false;
                    self.message =
                        "Restored committed records and layout. Undo history starts empty.".into();
                    self.error = false;
                }
                Err(error) => self.failure(error),
            },
            Ok(None) => {
                self.desk = Desk::default();
                self.desk_epoch += 1;
                self.workspace = default_workspace();
                self.persisted = None;
                self.load_error = false;
                self.message = "No saved state yet. Synthetic records are ready to review.".into();
                self.error = false;
            }
            Err(error) => {
                self.load_error = true;
                self.failure(format!("Restore failed: {error}. Saved data is preserved. Repair or move it outside the app, then use Restore."));
            }
        }
    }

    fn failure(&mut self, error: String) {
        self.message = error;
        self.error = true;
    }

    pub(crate) fn availability(&self, action: Action) -> Availability {
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

    fn action(&mut self, action: Action) -> Result<(), String> {
        if let Availability::Disabled { reason } = self.availability(action) {
            return Err(reason.into_owned());
        }
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
                self.persisted = Some(saved);
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
                self.focus_search = true;
            }
            Action::ResetFilters => {
                self.desk.set_filters(Filters::default());
                self.message = "Filters reset. Selection and editing draft are unchanged.".into();
            }
            Action::Arrange => {
                if let DockNode::Split { axis, .. } = &mut self.workspace.root {
                    *axis = if *axis == SplitAxis::Horizontal {
                        SplitAxis::Vertical
                    } else {
                        SplitAxis::Horizontal
                    };
                } else {
                    self.workspace = default_workspace();
                }
                self.message = "Pane arrangement changed. Save retains the layout.".into();
            }
            _ => {}
        }
        if action != Action::Restore {
            self.error = false;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.clone()
    }

    pub fn present(&mut self, root: &mut Ui) {
        let tokens = self.preferences.tokens(root.style().visuals.dark_mode);
        // Adapt the authoritative tree itself; never create a second docking model.
        self.narrow_layout = root.available_width() < 640.0;
        if self.narrow_layout
            && let DockNode::Split {
                axis: SplitAxis::Horizontal,
                ..
            } = &self.workspace.root
            && let DockNode::Split { axis, .. } = &mut self.workspace.root
        {
            *axis = SplitAxis::Vertical;
        }
        self.drain_inspection(root.ctx());
        let mut output = PaneOutput::default();
        let mut nodes = vec![UiNode::container(
            SemanticUiId::root(),
            None,
            UiRole::Application,
            root.max_rect().into(),
        )];
        nodes[0].name = "Record Desk".into();
        let mut text = Vec::new();

        // Check the more specific Shift shortcuts before their plain counterparts.
        for action in [
            Action::Redo,
            Action::Cancel,
            Action::Restore,
            Action::ResetFilters,
            Action::Apply,
            Action::Undo,
            Action::Save,
            Action::Search,
            Action::Arrange,
        ] {
            let shortcut = action
                .specification()
                .shortcut
                .expect("registered shortcut");
            // Egui's logical matcher accepts extra Shift/Alt. An unavailable
            // Reset shortcut must not fall through to the plain Search action.
            let exact_modifiers = root.input(|input| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key { key, pressed: true, modifiers, .. }
                        if *key == shortcut.key.egui()
                            && modifiers.shift == shortcut.shift
                            && modifiers.alt == shortcut.alt)
                })
            });
            if exact_modifiers
                && self.availability(action) == Availability::Enabled
                && consume_action_shortcut(root, action, true)
            {
                output.intents.push(Intent::Action(action));
            }
        }
        egui::Panel::top("record-desk-bar").show(root, |ui| {
            let scope = PresentationScope::new("record-desk.chrome");
            let mut clipped_actions = Vec::new();
            let mut p = PresentationContext::new(ui, tokens, 1.0, scope, SemanticUiId::root());
            ui.horizontal_wrapped(|ui| {
                p.heading(ui, "title", "Record Desk");
            });
            ui.horizontal_wrapped(|ui| {
                for action in [
                    Action::Undo,
                    Action::Redo,
                    Action::Save,
                    Action::Restore,
                    Action::Arrange,
                ] {
                    let response = p.action(
                        ui,
                        action,
                        button(
                            action,
                            None,
                            self.availability(action),
                            ActionEmphasis::Normal,
                        ),
                    );
                    if completely_clipped_action(response.rect, response.interact_rect) {
                        clipped_actions.push(scope.instance(action).semantic_id());
                    }
                    if response.clicked() {
                        output.intents.push(Intent::Action(action));
                    }
                }
            });
            p.content(
                ui,
                "state",
                &format!(
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
                ),
                content_spec(TextRole::Secondary, 2),
            );
            let observed = p.finish(ui);
            nodes.extend(
                observed
                    .semantic_nodes
                    .into_iter()
                    .filter(|node| !clipped_actions.contains(&node.id)),
            );
            text.extend(observed.text_layouts);
        });
        egui::Panel::bottom("record-desk-status").show(root, |ui| {
            let mut p = PresentationContext::new(
                ui,
                tokens,
                1.0,
                PresentationScope::new("record-desk.feedback"),
                SemanticUiId::root(),
            );
            p.content(
                ui,
                "message",
                &self.message,
                content_spec(
                    if self.error {
                        TextRole::Error
                    } else {
                        TextRole::Status
                    },
                    3,
                ),
            );
            let observed = p.finish(ui);
            nodes.extend(observed.semantic_nodes);
            text.extend(observed.text_layouts);
        });
        let mut presenter = RecordPanes {
            records: self.desk.visible(),
            total: self.desk.records().len(),
            selected: self.desk.selected(),
            draft: self.desk.draft(),
            filters: self.desk.filters(),
            dirty: self.desk.is_dirty(),
            apply: self.availability(Action::Apply),
            cancel: self.availability(Action::Cancel),
            reset_filters: self.availability(Action::ResetFilters),
            tokens,
            focus_search: std::mem::take(&mut self.focus_search),
            output: &mut output,
        };
        let resize = egui::CentralPanel::default()
            .show(root, |ui| {
                dock_workspace_layout(
                    ui,
                    &mut self.workspace,
                    &mut self.dock,
                    &mut presenter,
                    DockTextContext {
                        tokens,
                        font_scale: 1.0,
                    },
                )
            })
            .inner;
        nodes.extend(output.nodes);
        text.extend(output.text);
        // Scroll surfaces may submit offscreen/partial controls. Keep failed
        // attempts, but audit measured successes only when their complete text
        // is visible. The shared inventory still reports all submitted attempts.
        text.retain(|t| {
            t.layout_error.is_some()
                || (t.clip_rect.min_x <= t.allocated_rect.min_x + 1.0
                    && t.clip_rect.min_y <= t.allocated_rect.min_y + 1.0
                    && t.clip_rect.max_x >= t.allocated_rect.max_x - 1.0
                    && t.clip_rect.max_y >= t.allocated_rect.max_y - 1.0)
        });
        let coverage = text_audit_coverage(root.ctx(), &text);
        self.frame += 1;
        let mut ui_snapshot = UiSnapshot {
            frame: self.frame,
            pixels_per_point: root.ctx().pixels_per_point(),
            root: SemanticUiId::root(),
            nodes,
            text_audit: audit_text_layouts(&text),
            text,
            text_audit_coverage: Some(coverage),
            semantic_audit: vec![],
        };
        ui_snapshot.semantic_audit = ui_snapshot.audit();
        self.publish_inspection(root.ctx(), &ui_snapshot);
        let mut changed = false;
        if let Some(resize) = resize {
            match resize.apply(&mut self.workspace) {
                Ok(applied) => changed |= applied,
                Err(error) => self.failure(error),
            }
        }
        // Draft/filter outputs from this pass precede actions that consume them.
        output.intents.sort_by_key(|intent| match intent {
            Intent::Draft(_) | Intent::Filters(_) => 0,
            _ => 1,
        });
        for intent in output.intents {
            let result = match intent {
                Intent::Select(id) => self.desk.select(id),
                Intent::Filters(filters) => {
                    self.desk.set_filters(filters);
                    Ok(())
                }
                Intent::Draft(draft) => self.desk.edit_draft(draft),
                Intent::Action(action) => self.action(action),
            };
            if let Err(error) = result {
                self.failure(error);
            }
            changed = true;
        }
        self.snapshot = Snapshot {
            ui: ui_snapshot,
            controls: output.controls,
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
        // Outputs affect the next presentation; egui owns input/animation repaints.
        if changed || self.dock.interaction_active() {
            root.ctx().request_repaint();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = std::env::var_os("RECORD_DESK_SNAPSHOT") {
            let _ = std::fs::write(path, serde_json::to_vec(&self.snapshot).unwrap());
        }
    }
}

impl eframe::App for RecordDeskApp {
    #[cfg(target_arch = "wasm32")]
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        self.present(root);
    }
}

fn completely_clipped_action(allocated: egui::Rect, interactive: egui::Rect) -> bool {
    // The first panel sizing pass can clip a valid toolbar allocation entirely.
    // Preserve invalid allocations/non-finite observations as audit failures.
    allocated.is_finite()
        && allocated.is_positive()
        && interactive.is_finite()
        && !interactive.is_positive()
}

#[cfg(test)]
mod tests {
    use super::completely_clipped_action;
    use egui::{Rect, pos2};

    #[test]
    fn visibility_requires_valid_allocation_and_complete_finite_clipping() {
        let allocated = Rect::from_min_max(pos2(8.0, 35.0), pos2(60.0, 63.0));
        let hidden = Rect::from_min_max(allocated.min, pos2(60.0, 33.0));
        let partial = Rect::from_min_max(allocated.min, pos2(60.0, 48.0));
        assert!(completely_clipped_action(allocated, hidden));
        assert!(!completely_clipped_action(allocated, partial));
        assert!(!completely_clipped_action(allocated, allocated));
        assert!(!completely_clipped_action(hidden, hidden));
        assert!(!completely_clipped_action(Rect::NOTHING, hidden));
        assert!(!completely_clipped_action(allocated, Rect::NOTHING));
        let invalid = Rect::from_min_max(pos2(f32::NAN, 35.0), allocated.max);
        assert!(!completely_clipped_action(invalid, hidden));
        assert!(!completely_clipped_action(allocated, invalid));
    }
}
