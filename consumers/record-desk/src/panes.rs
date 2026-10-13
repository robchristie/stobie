use egui::Ui;
use stobie_core::PaneId;
use stobie_ui_egui::*;

use crate::{actions::Action, app::Intent, model::*};

type PaneFrame<'f, 'a> = &'f mut Frame<'a, Action, Intent>;

// These are the entire pane inputs: read views and resolved appearance. Panes
// emit intents and actions through the frame; they never see the application.
pub struct RecordPanes<'a> {
    pub records: Vec<&'a Record>,
    pub total: usize,
    pub selected: Option<RecordId>,
    pub draft: Option<&'a Record>,
    pub filters: &'a Filters,
    pub dirty: bool,
    pub tokens: DesignTokens,
    pub focus_search: bool,
}

pub fn content_spec(role: TextRole, lines: u8) -> ContentTextSpec {
    ContentTextSpec {
        role,
        overflow: TextOverflow::Wrap,
        max_lines: lines,
        interaction: TextInteraction::Selectable,
    }
}

impl RecordPanes<'_> {
    fn list(&mut self, ui: &mut Ui, p: &mut PresentationContext, frame: PaneFrame) {
        let mut filters = self.filters.clone();
        p.content(
            ui,
            "search-label",
            "Search titles and notes",
            content_spec(TextRole::Secondary, 1),
        );
        let search = text_input(
            ui,
            p,
            "record-desk.search",
            "Search records",
            &mut filters.query,
            false,
            None,
        );
        if self.focus_search {
            search.request_focus();
            self.focus_search = false;
        }
        ui.horizontal_wrapped(|ui| {
            let mut options = vec![(None, "All categories")];
            options.extend(Category::ALL.iter().map(|c| (Some(*c), c.label())));
            // Choice recipes observe their control and popup options themselves.
            choice_control_with_options(
                ui,
                SemanticUiId::new("record-desk.filter-category"),
                SemanticUiId::pane(LIST_PANE),
                "Category filter",
                &mut filters.category,
                &options,
                Action::FilterCategory,
                &self.tokens,
            );
            let options: Vec<_> = ReviewFilter::ALL.iter().map(|r| (*r, r.label())).collect();
            choice_control_with_options(
                ui,
                SemanticUiId::new("record-desk.filter-review"),
                SemanticUiId::pane(LIST_PANE),
                "Review filter",
                &mut filters.review,
                &options,
                Action::FilterReview,
                &self.tokens,
            );
        });
        if filters != *self.filters {
            frame.emit(Intent::Filters(filters));
        }
        ui.horizontal(|ui| {
            frame.action(ui, p, Action::ResetFilters, ActionEmphasis::Normal);
            p.content(
                ui,
                "count",
                &format!("{} of {} records", self.records.len(), self.total),
                content_spec(TextRole::Secondary, 1),
            );
        });
        if self.records.is_empty() {
            p.heading(
                ui,
                "empty",
                if self.total == 0 {
                    "No records"
                } else {
                    "No matching records"
                },
            );
            p.content(
                ui,
                "empty-help",
                if self.total == 0 {
                    "The saved collection is empty. Move the saved state outside the app, then use Restore to start the synthetic collection."
                } else {
                    "Use Reset filters to show all records. A hidden selection and its editing draft stay in the editor."
                },
                content_spec(TextRole::Body, 4),
            );
        }
        for record in &self.records {
            let selected = self.selected == Some(record.id);
            let (response, node, observation) = p.raw(
                ui,
                record.id,
                "Application-owned record row with measured title and stable domain identity",
                |ui| record_row(ui, record, selected, &self.tokens),
            );
            p.observe_node(ui, node);
            p.observe_text(ui, observation);
            if response.clicked() {
                frame.emit(Intent::Select(record.id));
            }
        }
    }

    fn detail(&mut self, ui: &mut Ui, p: &mut PresentationContext, frame: PaneFrame) {
        let Some(record) = self.draft else {
            p.heading(ui, "empty", "Choose a record");
            p.content(
                ui,
                "empty-help",
                "Select a record in the list to inspect and edit it.",
                content_spec(TextRole::Body, 2),
            );
            return;
        };
        let mut draft = record.clone();
        p.heading(ui, "heading", "Record detail");
        p.property_row(ui, "identity", "Stable ID", &record.id.0.to_string());
        if !self.records.iter().any(|r| r.id == record.id) {
            p.content(
                ui,
                "hidden",
                "This selected record is hidden by the current filters. Your draft stays here.",
                content_spec(TextRole::Status, 2),
            );
        }
        p.content(
            ui,
            "title-label",
            "Title",
            content_spec(TextRole::Secondary, 1),
        );
        text_input(
            ui,
            p,
            &format!("record-desk.title.{}", record.id.0),
            "Record title",
            &mut draft.title,
            false,
            Some(record.id),
        );
        p.content(
            ui,
            "category-label",
            "Category",
            content_spec(TextRole::Secondary, 1),
        );
        let options: Vec<_> = Category::ALL.iter().map(|c| (*c, c.label())).collect();
        choice_control_with_options(
            ui,
            SemanticUiId::new(format!("record-desk.category.{}", record.id.0)),
            SemanticUiId::pane(DETAIL_PANE),
            "Record category",
            &mut draft.category,
            &options,
            Action::EditCategory,
            &self.tokens,
        );
        frame.toggle(ui, p, "reviewed", Action::ToggleReviewed, draft.reviewed);
        p.content(
            ui,
            "notes-label",
            "Notes",
            content_spec(TextRole::Secondary, 1),
        );
        text_input(
            ui,
            p,
            &format!("record-desk.notes.{}", record.id.0),
            "Record notes",
            &mut draft.notes,
            true,
            Some(record.id),
        );
        if let Err(error) = validate_record(&draft) {
            p.content(ui, "validation", &error, content_spec(TextRole::Error, 2));
        }
        p.content(
            ui,
            "draft-state",
            if self.dirty {
                "Draft changes · Apply validates and commits"
            } else {
                "Committed record · edit to create a draft"
            },
            content_spec(TextRole::Secondary, 2),
        );
        ui.horizontal_wrapped(|ui| {
            frame.action(ui, p, Action::Apply, ActionEmphasis::Primary);
            frame.action(ui, p, Action::Cancel, ActionEmphasis::Normal);
        });
        p.content(
            ui,
            "keyboard",
            "Ctrl/Cmd+Enter Apply · Ctrl/Cmd+Shift+Enter Cancel · Ctrl/Cmd+F Search",
            content_spec(TextRole::Caption, 3),
        );
        if draft != *record {
            frame.emit(Intent::Draft(draft));
        }
    }
}

impl Panes<Action, Intent> for RecordPanes<'_> {
    fn title(&self, pane: PaneId) -> &'static str {
        if pane == LIST_PANE {
            "Records"
        } else {
            "Detail"
        }
    }

    fn pane_ui(&mut self, ui: &mut Ui, pane: PaneId, frame: PaneFrame) {
        let record = if pane == DETAIL_PANE {
            self.selected
        } else {
            None
        };
        let scope = PresentationScope::new(("record-desk.pane", pane)).child(record);
        let mut p = frame.pane_presentation(ui, scope, pane);
        if pane == DETAIL_PANE
            && let Some(id) = self.selected
        {
            p = p.with_domain_reference(domain(id));
        }
        egui::ScrollArea::vertical()
            .id_salt(("record-desk.scroll", pane))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if pane == LIST_PANE {
                    self.list(ui, &mut p, frame);
                } else {
                    self.detail(ui, &mut p, frame);
                }
            });
    }
}

fn domain(id: RecordId) -> DomainReference {
    DomainReference::External {
        namespace: "record-desk.record".into(),
        id: id.0.to_string(),
    }
}

fn set_visible_bounds(node: &mut egui::accesskit::Node, response: &egui::Response) {
    if response.interact_rect.is_positive() {
        node.set_bounds(egui::accesskit::Rect {
            x0: f64::from(response.interact_rect.min.x),
            y0: f64::from(response.interact_rect.min.y),
            x1: f64::from(response.interact_rect.max.x),
            y1: f64::from(response.interact_rect.max.y),
        });
    }
}

// Keep the native control's identity and observation inputs explicit.
fn text_input(
    ui: &mut Ui,
    p: &mut PresentationContext,
    id: &str,
    name: &str,
    value: &mut String,
    multiline: bool,
    record: Option<RecordId>,
) -> egui::Response {
    let (response, ()) = p.native(ui, NativeTextControlKind::TextEdit, |ui| {
        let edit = if multiline {
            egui::TextEdit::multiline(value).desired_rows(4)
        } else {
            egui::TextEdit::singleline(value)
        };
        let response = ui.add(
            edit.id(egui::Id::new(id))
                .desired_width(ui.available_width()),
        );
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_label(name);
            node.set_author_id(id.to_owned());
            set_visible_bounds(node, &response);
        });
        (response, ())
    });
    let mut node = UiNode::container(
        SemanticUiId::new(id),
        Some(if record.is_some() {
            SemanticUiId::pane(DETAIL_PANE)
        } else {
            SemanticUiId::pane(LIST_PANE)
        }),
        if multiline {
            UiRole::MultilineTextInput
        } else {
            UiRole::TextInput
        },
        response.rect.into(),
    );
    node.name = name.into();
    node.focused = response.has_focus();
    node.text_selectable = true;
    node.domain_reference = record.map(domain);
    p.observe_node(ui, node);
    response
}

fn record_row(
    ui: &mut Ui,
    record: &Record,
    selected: bool,
    tokens: &DesignTokens,
) -> (egui::Response, UiNode, TextLayoutObservation) {
    let label = format!(
        "{} · {} · {}",
        record.title,
        record.category.label(),
        if record.reviewed {
            "Reviewed"
        } else {
            "Unreviewed"
        }
    );
    let measured = measure_text(
        ui.painter(),
        &label,
        TextSpec {
            max_lines: 2,
            ..TextSpec::single_line(TextRole::Body, TextOverflow::Wrap)
        },
        tokens,
        1.0,
        ui.available_width() - tokens.geometry.control_padding_x.0 * 2.0,
    )
    .expect("valid record row text contract");
    let height = (measured.size().y + tokens.geometry.control_padding_y.0 * 2.0)
        .max(tokens.geometry.minimum_hit_size.0);
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), height));
    let id = format!("record-desk.record.{}", record.id.0);
    let response = ui.interact(rect, egui::Id::new(&id), egui::Sense::click());
    if response.clicked() {
        response.request_focus();
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, &label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::ListBoxOption);
        node.set_label(label.as_str());
        node.set_author_id(id.clone());
        node.clear_toggled();
        node.set_selected(selected);
        node.add_action(egui::accesskit::Action::Click);
        set_visible_bounds(node, &response);
    });
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.0,
            if selected {
                tokens.colours.selection_background
            } else {
                tokens.colours.surface_hover
            },
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, tokens.colours.focus_ring),
            egui::StrokeKind::Inside,
        );
    }
    let text_rect = rect.shrink2(egui::vec2(
        tokens.geometry.control_padding_x.0,
        tokens.geometry.control_padding_y.0,
    ));
    let observation = paint_measured_text(
        &ui.painter_at(ui.clip_rect()),
        &measured,
        text_rect,
        TextComponentId::new(TextComponentKind::ResultRow, record.id.0),
        None,
    );
    let mut node = UiNode::container(
        SemanticUiId::new(id),
        Some(SemanticUiId::pane(LIST_PANE)),
        UiRole::ResultRow,
        rect.into(),
    );
    node.name = label;
    node.selected = selected;
    node.focused = response.has_focus();
    node.pane = Some(LIST_PANE);
    node.domain_reference = Some(domain(record.id));
    (response, node, observation)
}
