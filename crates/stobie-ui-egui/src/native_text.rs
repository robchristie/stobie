//! Native browser text fields for egui text edits.
//!
//! On touch devices a browser's own text field gives the platform caret,
//! loupe, selection, autocorrect context, dictation and composition, which an
//! egui text edit drawn on the canvas cannot. [`NativeTextEdit`] is an
//! ordinary egui text edit that the browser host may present as the ARIA
//! layer's own field while it is being edited: a tap on the field moves that
//! field out of the transparent layer, over the egui field, styled from the
//! `Ui`'s visuals. egui keeps the text, the frame and the actions, and stops
//! drawing the text, caret and selection while the native field is active.
//! Natively, and in a browser until a tap activates it, it is an ordinary
//! egui text edit.
//!
//! The registration made here each pass is all the host needs: the field's
//! geometry in points and the style its native field takes.

use std::sync::Arc;

use egui::{
    Color32, FontFamily, FontId, Galley, Id, KeyboardShortcut, Margin, Pos2, Rect, Response, Ui,
};

/// A single-line or multiline text edit that the browser host presents as a
/// native text field while a tap has it active. Use it where the application
/// would add an [`egui::TextEdit`], inside `PresentationContext::native` with
/// `NativeTextControlKind::TextEdit` so the host pairs it with its observed
/// text input node.
#[must_use = "call `show` to add the text edit"]
pub struct NativeTextEdit<'t> {
    text: &'t mut String,
    id: Id,
    multiline: bool,
    hint: String,
    font: Option<FontId>,
    rows: Option<usize>,
    width: Option<f32>,
    margin: Margin,
    spellcheck: bool,
    frame: bool,
    /// `None` keeps egui's own return key.
    return_key: Option<Option<KeyboardShortcut>>,
    max_rows: Option<usize>,
}

impl<'t> NativeTextEdit<'t> {
    /// A multiline field, such as a message composer. Return types a newline.
    pub fn multiline(text: &'t mut String, id: impl Into<Id>) -> Self {
        Self::new(text, id.into(), true)
    }

    /// A single-line field.
    pub fn singleline(text: &'t mut String, id: impl Into<Id>) -> Self {
        Self::new(text, id.into(), false)
    }

    fn new(text: &'t mut String, id: Id, multiline: bool) -> Self {
        Self {
            text,
            id,
            multiline,
            hint: String::new(),
            font: None,
            rows: None,
            width: None,
            // egui's own default margin.
            margin: Margin::symmetric(4, 2),
            spellcheck: true,
            frame: true,
            return_key: None,
            max_rows: None,
        }
    }

    /// Text shown while the field is empty.
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// The font; by default the `Ui`'s body text.
    pub fn font(mut self, font: FontId) -> Self {
        self.font = Some(font);
        self
    }

    /// See [`egui::TextEdit::desired_rows`].
    pub fn desired_rows(mut self, rows: usize) -> Self {
        self.rows = Some(rows);
        self
    }

    /// See [`egui::TextEdit::desired_width`].
    pub fn desired_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// See [`egui::TextEdit::margin`].
    pub fn margin(mut self, margin: impl Into<Margin>) -> Self {
        self.margin = margin.into();
        self
    }

    /// Whether egui draws the field's frame (on by default). Turn it off for
    /// a field inside a surface the application draws, such as a composer.
    pub fn frame(mut self, frame: bool) -> Self {
        self.frame = frame;
        self
    }

    /// The key that types a newline in a multiline field, or ends editing in
    /// a single-line one, as [`egui::TextEdit::return_key`]: `None` for no
    /// such key. It applies to egui's keyboard input; an active native
    /// field's Return is the platform's, which types a newline in a
    /// multiline field.
    pub fn return_key(mut self, return_key: impl Into<Option<KeyboardShortcut>>) -> Self {
        self.return_key = Some(return_key.into());
        self
    }

    /// Grow a multiline field with its text up to `rows` lines, then scroll
    /// within it. The response's rectangle is then the part of the field
    /// shown, and so are its semantic bounds and its native field, which
    /// scrolls natively. Use [`Self::desired_rows`] for the height it starts
    /// at. Its hint shows on one line, elided with an ellipsis; assistive
    /// technology and the native field keep the whole hint.
    pub fn max_rows(mut self, rows: usize) -> Self {
        self.max_rows = Some(rows.max(1));
        self
    }

    /// Whether the native field offers spelling, autocorrect and sentence
    /// capitalisation (on by default). Turn it off for names, codes and
    /// other text that is not prose.
    pub fn spellcheck(mut self, spellcheck: bool) -> Self {
        self.spellcheck = spellcheck;
        self
    }

    /// Add the field and register it for this pass.
    pub fn show(self, ui: &mut Ui) -> Response {
        let active = native_text_active(ui.ctx()) == Some(self.id);
        let font = self
            .font
            .unwrap_or_else(|| egui::FontSelection::Default.resolve(ui.style()));
        let visuals = ui.visuals();
        // egui's text edit colours, which Stobie's theme sets from tokens.
        let text = visuals
            .override_text_color
            .unwrap_or_else(|| visuals.widgets.inactive.text_color());
        let caret = visuals.text_cursor.stroke.color;
        let selection = visuals.selection.bg_fill;
        let placeholder = visuals.weak_text_color();
        let line_height = ui.ctx().fonts_mut(|fonts| fonts.row_height(&font));
        let clip = ui.clip_rect();
        let margin = self.margin;
        let max_height = self.max_rows.filter(|_| self.multiline).map(|rows| {
            rows_height(ui, &font, rows) + f32::from(margin.top) + f32::from(margin.bottom)
        });
        // A limited field shows its hint on one line, elided; the native
        // field and assistive technology keep the whole hint.
        let shown_hint = match max_height {
            Some(_) if !self.hint.is_empty() => {
                let width = self.width.unwrap_or_else(|| ui.available_width())
                    - f32::from(margin.left)
                    - f32::from(margin.right);
                elide_to_width(ui, &self.hint, &font, width)
            }
            _ => self.hint.clone(),
        };
        let edit = |ui: &mut Ui| {
            let mut edit = if self.multiline {
                egui::TextEdit::multiline(self.text)
            } else {
                egui::TextEdit::singleline(self.text)
            }
            .id(self.id)
            .font(font.clone())
            .margin(margin);
            if !self.frame {
                // No fill or stroke, and exactly the margin.
                edit = edit.frame(egui::Frame::new().inner_margin(margin));
            }
            if let Some(return_key) = self.return_key {
                edit = edit.return_key(return_key);
            }
            if let Some(rows) = self.rows {
                edit = edit.desired_rows(rows);
            }
            if let Some(width) = self.width {
                edit = edit.desired_width(width);
            }
            if active {
                // The native field draws the text, hint, caret and
                // selection; egui keeps the frame and lays the text out,
                // so the field keeps its height.
                let visuals = ui.visuals_mut();
                visuals.text_cursor.stroke = egui::Stroke::NONE;
                visuals.text_cursor.blink = false;
                visuals.text_cursor.preview = false;
                visuals.selection.bg_fill = Color32::TRANSPARENT;
                edit = edit.text_color(Color32::TRANSPARENT);
            } else if !shown_hint.is_empty() {
                edit = edit.hint_text(shown_hint.as_str());
            }
            edit.show(ui)
        };
        let (output, viewport) = match max_height {
            None => (ui.scope(edit).inner, None),
            Some(max_height) => {
                // The scroll area grows with the text up to the limit. While
                // the native field is active it scrolls itself, so egui's
                // scroll bar stays out of its way.
                let scrolled = egui::ScrollArea::vertical()
                    .id_salt(self.id.with("stobie.native-text-scroll"))
                    .max_height(max_height)
                    .min_scrolled_height(0.0)
                    .content_margin(0)
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(if active {
                        egui::scroll_area::ScrollBarVisibility::AlwaysHidden
                    } else {
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded
                    })
                    .show(ui, edit);
                // The output's inner rectangle is the room offered; the area
                // shrinks from it to the content.
                let shown = scrolled.inner_rect.height().min(scrolled.content_size.y);
                let viewport = Rect::from_min_size(
                    scrolled.inner_rect.min,
                    egui::vec2(scrolled.inner_rect.width(), shown),
                );
                (scrolled.inner, Some(viewport))
            }
        };
        let mut response = output.response.response;
        if shown_hint != self.hint {
            let hint = self.hint.clone();
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_placeholder(hint));
        }
        // egui's text rectangle is as wide as the text laid out; the native
        // field wraps at the field's width less its margins, as egui does.
        let field = response.rect;
        let text_clip = {
            let clip = output.text_clip_rect;
            let left = clip.min.x - field.min.x;
            let right = left - f32::from(margin.left) + f32::from(margin.right);
            Rect::from_x_y_ranges(
                clip.min.x..=(field.max.x - right).max(clip.min.x),
                clip.y_range(),
            )
        };
        // The part of the field shown, padded as egui pads the text.
        let (rect, text_rect) = match viewport {
            None => (field, text_clip),
            Some(viewport) => {
                let rect = Rect::from_x_y_ranges(field.x_range(), viewport.y_range());
                let text_rect = Rect::from_min_max(
                    rect.min + (text_clip.min - field.min),
                    rect.max - (field.max - text_clip.max),
                );
                response.rect = rect;
                response.interact_rect = response.interact_rect.intersect(rect);
                ui.ctx().accesskit_node_builder(response.id, |node| {
                    node.set_bounds(egui::accesskit::Rect {
                        x0: f64::from(rect.min.x),
                        y0: f64::from(rect.min.y),
                        x1: f64::from(rect.max.x),
                        y1: f64::from(rect.max.y),
                    });
                });
                (rect, text_rect)
            }
        };
        // egui places a text edit's rows on whole points, a little closer
        // than the font's row height; the native field's lines match them,
        // so its text fits the field egui sized.
        let rows = output.galley.rows.len();
        let line_height = if self.multiline && rows > 0 && output.galley.rect.height() > 0.0 {
            output.galley.rect.height() / rows as f32
        } else {
            line_height
        };
        register(
            ui.ctx(),
            NativeTextRegistration {
                id: self.id,
                multiline: self.multiline,
                rect,
                text_rect,
                clip,
                galley: output.galley,
                galley_pos: output.galley_pos,
                family: match font.family {
                    FontFamily::Monospace => NativeFont::Monospace,
                    _ => NativeFont::Proportional,
                },
                font_size: font.size,
                line_height,
                text,
                caret,
                selection,
                placeholder,
                hint: self.hint,
                spellcheck: self.spellcheck,
            },
        );
        response
    }
}

/// The face a native field uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeFont {
    /// Stobie's regular face, which the browser host registers from the same
    /// bytes egui uses, then the platform's.
    Proportional,
    Monospace,
}

/// One native text field presented in a pass, in points.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeTextRegistration {
    pub id: Id,
    pub multiline: bool,
    /// The field's whole rectangle, its frame included.
    pub rect: Rect,
    /// Where egui lays out the text: the padding is the difference.
    pub text_rect: Rect,
    /// What the field's `Ui` shows of it.
    pub clip: Rect,
    /// The text as egui laid it out, and where.
    pub galley: Arc<Galley>,
    pub galley_pos: Pos2,
    pub family: NativeFont,
    pub font_size: f32,
    pub line_height: f32,
    pub text: Color32,
    pub caret: Color32,
    pub selection: Color32,
    pub placeholder: Color32,
    pub hint: String,
    pub spellcheck: bool,
}

impl NativeTextRegistration {
    /// The character a tap at `point` (in points) places the caret before,
    /// as egui would place it.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn char_at(&self, point: Pos2) -> usize {
        self.galley
            .cursor_from_pos(point - self.galley_pos)
            .index
            .into()
    }
}

#[derive(Clone, Default)]
struct Registry {
    pass: u64,
    fields: Vec<NativeTextRegistration>,
}

/// The height of `rows` lines of `font` as a multiline text edit lays them
/// out, with egui's line height and rounding.
fn rows_height(ui: &Ui, font: &FontId, rows: usize) -> f32 {
    let line_height =
        ui.ctx().fonts_mut(|fonts| fonts.row_height(font)) + ui.spacing().extra_text_line_spacing;
    let mut job = egui::text::LayoutJob::simple(
        "\n".repeat(rows.saturating_sub(1)),
        font.clone(),
        Color32::PLACEHOLDER,
        f32::INFINITY,
    );
    job.keep_trailing_whitespace = true;
    for section in &mut job.sections {
        section.format.line_height = Some(line_height);
    }
    ui.ctx()
        .fonts_mut(|fonts| fonts.layout_job(job))
        .rect
        .height()
}

/// `text` on one line within `width`, shortened with an ellipsis if needed.
fn elide_to_width(ui: &Ui, text: &str, font: &FontId, width: f32) -> String {
    let measure = |candidate: &str| {
        ui.ctx().fonts_mut(|fonts| {
            fonts
                .layout_no_wrap(candidate.to_owned(), font.clone(), Color32::PLACEHOLDER)
                .size()
                .x
        })
    };
    if measure(text) <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let shortened = |count: usize| {
        let mut kept: String = chars[..count].iter().collect();
        kept.truncate(kept.trim_end().len());
        kept.push('\u{2026}');
        kept
    };
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if measure(&shortened(middle)) <= width {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    shortened(low)
}

fn registry_id() -> Id {
    Id::new("stobie.native-text-fields")
}

fn active_id() -> Id {
    Id::new("stobie.native-text-active")
}

fn register(context: &egui::Context, registration: NativeTextRegistration) {
    let pass = context.cumulative_pass_nr();
    context.data_mut(|data| {
        let registry = data.get_temp_mut_or_default::<Registry>(registry_id());
        if registry.pass != pass {
            *registry = Registry {
                pass,
                fields: Vec::new(),
            };
        }
        if !registry
            .fields
            .iter()
            .any(|field| field.id == registration.id)
        {
            registry.fields.push(registration);
        }
    });
}

/// The native fields registered in the pass that just ended, read from a
/// plugin's output hook (egui counts the pass as complete by then).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn registered_in_completed_pass(context: &egui::Context) -> Vec<NativeTextRegistration> {
    let completed = context.cumulative_pass_nr().checked_sub(1);
    context.data(|data| {
        data.get_temp::<Registry>(registry_id())
            .filter(|registry| Some(registry.pass) == completed)
            .map(|registry| registry.fields)
            .unwrap_or_default()
    })
}

/// The text edit whose native field is active in the browser, if any.
pub fn native_text_active(context: &egui::Context) -> Option<Id> {
    context.data(|data| data.get_temp::<Option<Id>>(active_id()).flatten())
}

/// Record which native field the browser host has active; the next pass
/// draws it accordingly.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn set_native_text_active(context: &egui::Context, id: Option<Id>) {
    context.data_mut(|data| data.insert_temp(active_id(), id));
}

#[cfg(test)]
mod tests;
