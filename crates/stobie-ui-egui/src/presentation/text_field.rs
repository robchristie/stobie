//! The text field recipe: a labelled single-line or multiline field in a
//! form, with an optional description and error line.

use std::hash::Hash;

use egui::{Key, Response, Stroke, Ui};

use super::{ContentTextSpec, PresentationContext};
use crate::{
    NativeTextControlKind, NativeTextEdit, SemanticUiId, TextInteraction, TextOverflow, TextRole,
    TextRoleStyles, UiNode, UiRole, components::action_hit_height,
};

/// The caption above a field: one measured, inert line that wraps to a
/// second before it is elided.
const CAPTION: ContentTextSpec = ContentTextSpec {
    role: TextRole::Caption,
    overflow: TextOverflow::Wrap,
    max_lines: 2,
    interaction: TextInteraction::Inert,
};

/// A field's description beneath it.
const DESCRIPTION: ContentTextSpec = ContentTextSpec {
    role: TextRole::Secondary,
    overflow: TextOverflow::Wrap,
    max_lines: 4,
    interaction: TextInteraction::Selectable,
};

/// A field's error beneath it, selectable as status and error text is.
const ERROR: ContentTextSpec = ContentTextSpec {
    role: TextRole::Error,
    overflow: TextOverflow::Wrap,
    max_lines: 4,
    interaction: TextInteraction::Selectable,
};

/// A labelled text field in a form: its visible caption, which also names
/// it, and how it edits. Build one with [`TextFieldSpec::new`] and the
/// builder methods; every word is the application's.
///
/// ```
/// use stobie_ui_egui::TextFieldSpec;
///
/// let name = TextFieldSpec::new("Name").hint("Morning briefing");
/// let prompt = TextFieldSpec::new("Prompt")
///     .multiline(3)
///     .description("What the routine asks for each morning.");
/// let code = TextFieldSpec::new("Invitation code")
///     .spellcheck(false)
///     .error(Some("That code has expired."));
/// # let _ = (name, prompt, code);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextFieldSpec<'a> {
    label: &'a str,
    rows: Option<usize>,
    max_rows: Option<usize>,
    hint: &'a str,
    description: Option<&'a str>,
    error: Option<&'a str>,
    password: bool,
    spellcheck: bool,
    id: Option<&'a str>,
}

impl<'a> TextFieldSpec<'a> {
    /// A single-line field captioned and named `label`.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            rows: None,
            max_rows: None,
            hint: "",
            description: None,
            error: None,
            password: false,
            spellcheck: true,
            id: None,
        }
    }

    /// A multiline field that starts `rows` lines tall (at least one) and
    /// grows with its text. Return types a newline.
    pub fn multiline(mut self, rows: usize) -> Self {
        self.rows = Some(rows.max(1));
        self
    }

    /// Let a multiline field grow to `rows` lines, then scroll within it.
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = Some(rows.max(1));
        self
    }

    /// Text shown while the field is empty.
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    /// A line beneath the field saying what it is for; assistive
    /// technology reads it as the field's description.
    pub fn description(mut self, description: &'a str) -> Self {
        self.description = Some(description).filter(|text| !text.is_empty());
        self
    }

    /// An error beneath the field, such as why its text cannot be saved, or
    /// `None` for none. The field is marked invalid, its border takes the
    /// error colour, and assistive technology announces the error when it
    /// appears and reads it with the field's description.
    pub fn error(mut self, error: impl Into<Option<&'a str>>) -> Self {
        self.error = error.into().filter(|text| !text.is_empty());
        self
    }

    /// A single-line password field: its text is masked, never published
    /// to observation, never set by an agent and never presented as a
    /// browser's native field. It ignores [`Self::multiline`].
    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    /// Whether a touch device's native field offers spelling, autocorrect
    /// and capitalisation (on by default). Turn it off for names, codes and
    /// addresses.
    pub fn spellcheck(mut self, spellcheck: bool) -> Self {
        self.spellcheck = spellcheck;
        self
    }

    /// Give the field this semantic ID, and an egui ID made from it, rather
    /// than one derived from the context's scope and the key: for a field
    /// that agents, tests or documentation already address by a published
    /// ID. Keep it stable and unique.
    pub fn id(mut self, id: &'a str) -> Self {
        self.id = Some(id);
        self
    }

    /// The caption, which is also the field's accessible name.
    pub fn label(&self) -> &'a str {
        self.label
    }

    fn multiline_rows(&self) -> Option<usize> {
        self.rows.filter(|_| !self.password)
    }

    /// The field's description for assistive technology: the error, then
    /// the description, each on its own line.
    fn accessible_description(&self) -> Option<String> {
        match (self.error, self.description) {
            (None, None) => None,
            (Some(text), None) | (None, Some(text)) => Some(text.to_owned()),
            (Some(error), Some(description)) => Some(format!("{error}\n{description}")),
        }
    }
}

/// What a text field did in one pass.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TextFieldResponse {
    /// The field's response; request focus on it to focus the field.
    pub field: Response,
    /// The caption's response.
    pub caption: Response,
    /// Whether this pass's typing or pasting changed the text.
    pub changed: bool,
    /// Whether Return was pressed in a focused single-line field, to submit
    /// the form. A multiline field's Return types a newline instead.
    pub submitted: bool,
}

impl PresentationContext {
    /// Present a labelled text field editing `text`, and report what it did;
    /// hosted applications use [`Frame::text_field`](crate::Frame::text_field),
    /// which emits the edit as an intent.
    ///
    /// - A measured caption sits above the field and names it: the field's
    ///   AccessKit node is labelled by the caption, with the caption as its
    ///   label too, so assistive technology, the snapshot and agents all
    ///   call it by the visible words.
    /// - The field is a [`NativeTextEdit`] in body text on the text-field
    ///   ground, at least the token hit height, as wide as the space offered.
    ///   On a touch device a tap makes it the platform's own field.
    /// - Its border is the control border, the focus ring while it has
    ///   focus, and the error colour while it shows an error.
    /// - An error, then a description, follow beneath it as measured,
    ///   selectable text. The field is marked invalid while it has an error,
    ///   and a polite live status announces the error when it appears.
    /// - Return in a focused single-line field reports `submitted` and keeps
    ///   focus; in a multiline field it types a newline.
    /// - The field is observed as a text input, or a multiline text input,
    ///   named by the caption and described by its error and description,
    ///   so agents fill it with `set-text`. A password field publishes no
    ///   text and offers no `set-text`.
    /// - A disabled `Ui` disables it.
    pub fn text_field(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: &TextFieldSpec<'_>,
        text: &mut String,
    ) -> TextFieldResponse {
        self.check_pass(ui);
        let identity = self.scope.instance(key);
        let (field_id, semantic_id) = match spec.id {
            Some(id) => (egui::Id::new(id), SemanticUiId::new(id)),
            None => (identity.egui_id(), identity.semantic_id()),
        };
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let enabled = ui.is_enabled();

        let caption = self.content(
            ui,
            (field_id, "stobie.text-field.caption"),
            spec.label,
            CAPTION,
        );

        // One line fills the token hit height, as in a composer.
        let hit = action_hit_height(&tokens, font_scale);
        let body = TextRole::Body.style(&tokens, font_scale);
        let line_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&body.font_id));
        let spare = (hit - line_height).max(0.0);
        let top = (spare / 2.0).floor().min(127.0);
        let bottom = (spare - top).ceil().min(127.0);
        let horizontal = tokens
            .geometry
            .control_padding_x
            .0
            .round()
            .clamp(0.0, 127.0) as i8;
        let margin = egui::Margin {
            left: horizontal,
            right: horizontal,
            top: top as i8,
            bottom: bottom as i8,
        };
        let multiline = spec.multiline_rows();
        let had_focus = ui.memory(|memory| memory.has_focus(field_id));
        let before = text.clone();
        let width = ui.available_width();
        let background = ui.painter().add(egui::Shape::Noop);
        let (field, ()) = self.native(ui, NativeTextControlKind::TextEdit, |ui| {
            let mut edit = match multiline {
                Some(rows) => {
                    let edit = NativeTextEdit::multiline(text, field_id).desired_rows(rows);
                    match spec.max_rows {
                        Some(max_rows) => edit.max_rows(max_rows.max(rows)),
                        None => edit,
                    }
                }
                // Return submits rather than ending the edit.
                None => NativeTextEdit::singleline(text, field_id).return_key(None),
            };
            if !spec.hint.is_empty() {
                edit = edit.hint_text(spec.hint);
            }
            let response = edit
                .font(body.font_id.clone())
                .desired_width(width)
                .margin(margin)
                .frame(false)
                .spellcheck(spec.spellcheck && !spec.password)
                .password(spec.password)
                .show(ui);
            (response, ())
        });
        let changed = *text != before;
        let error = spec.error;
        let description = spec.accessible_description();
        ui.ctx().accesskit_node_builder(field.id, |node| {
            node.set_label(spec.label);
            node.push_labelled_by(caption.id.accesskit_id());
            node.set_author_id(semantic_id.0.clone());
            if let Some(description) = &description {
                node.set_description(description.clone());
            }
            if error.is_some() {
                node.set_invalid(egui::accesskit::Invalid::True);
            }
            // The part a scroll area shows.
            let shown = field.interact_rect;
            if shown.is_positive() {
                node.set_bounds(egui::accesskit::Rect {
                    x0: f64::from(shown.min.x),
                    y0: f64::from(shown.min.y),
                    x1: f64::from(shown.max.x),
                    y1: f64::from(shown.max.y),
                });
            }
        });
        let submitted = multiline.is_none() && enabled && had_focus && returned(ui);

        // The ground and border, behind the frameless field.
        let colour = if !enabled {
            tokens.colours.border_subtle
        } else if field.has_focus() {
            tokens.colours.focus_ring
        } else if error.is_some() {
            tokens.colours.status_error
        } else {
            tokens.colours.border_control
        };
        ui.painter().set(
            background,
            egui::epaint::RectShape::new(
                field.rect,
                tokens.geometry.control_radius.0,
                ui.visuals().text_edit_bg_color(),
                Stroke::new(1.0, colour),
                egui::StrokeKind::Inside,
            ),
        );

        let mut node = UiNode::container(
            semantic_id.clone(),
            Some(self.parent.clone()),
            if multiline.is_some() {
                UiRole::MultilineTextInput
            } else {
                UiRole::TextInput
            },
            field.rect.into(),
        );
        node.name = spec.label.to_owned();
        node.description = description;
        node.focused = field.has_focus();
        node.enabled = enabled;
        node.text_selectable = true;
        if self.domain_reference.is_some() {
            node.domain_reference = self.domain_reference.clone();
        }
        if field.rect.is_positive() {
            self.push_node(ui, node);
        }

        // The error, then the description.
        let error_line = error.map(|error| {
            let line = self.content(ui, (field_id, "stobie.text-field.error"), error, ERROR);
            // The status below presents it to assistive technology.
            ui.ctx()
                .accesskit_node_builder(line.id, |node| node.set_hidden());
            line
        });
        if let Some(description) = spec.description {
            self.content(
                ui,
                (field_id, "stobie.text-field.description"),
                description,
                DESCRIPTION,
            );
        }
        // A polite live region that is present, empty, before any error, so
        // assistive technology announces an error when it appears.
        let status_id = field_id.with("stobie.text-field.status");
        let status_semantic = SemanticUiId::new(format!("{}.status", semantic_id.0));
        let status_rect = error_line.as_ref().map_or(field.rect, |line| line.rect);
        let message = error.unwrap_or_default();
        ui.ctx().accesskit_node_builder(status_id, |node| {
            node.set_role(egui::accesskit::Role::Status);
            node.set_label(message);
            node.set_live(egui::accesskit::Live::Polite);
            node.set_author_id(status_semantic.0.clone());
            node.set_bounds(egui::accesskit::Rect {
                x0: f64::from(status_rect.min.x),
                y0: f64::from(status_rect.min.y),
                x1: f64::from(status_rect.max.x),
                y1: f64::from(status_rect.max.y),
            });
        });
        if !message.is_empty() && status_rect.is_positive() {
            let mut node = UiNode::container(
                status_semantic,
                Some(self.parent.clone()),
                UiRole::Status,
                status_rect.into(),
            );
            node.name = message.to_owned();
            node.domain_reference = self.domain_reference.clone();
            self.push_node(ui, node);
        }

        TextFieldResponse {
            field,
            caption,
            changed,
            submitted,
        }
    }
}

/// Whether this pass has a fresh Return press without modifiers.
fn returned(ui: &Ui) -> bool {
    ui.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: Key::Enter,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if modifiers.is_none()
            )
        })
    })
}

#[cfg(test)]
#[path = "text_field_tests.rs"]
mod tests;
