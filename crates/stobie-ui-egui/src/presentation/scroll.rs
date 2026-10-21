//! Scrolling recipes: a vertical scroll area that keeps the reader's place
//! when rows arrive above the ones in view, and a footer that sits at the
//! foot of a short pane and after the last row of a long one.

use std::fmt::Debug;
use std::hash::Hash;

use egui::Ui;
use egui::scroll_area::ScrollAreaOutput;

/// The row a reader is at: the first row in view, where it was in the
/// content, and whether the view was at the content's end.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Anchor {
    key: egui::Id,
    /// The row's top, from the top of the content.
    top: f32,
    /// Whether the view showed the end of the content.
    at_end: bool,
}

/// A vertical [`egui::ScrollArea`] that keeps the reader's place when rows
/// are inserted, removed or resized above the ones in view, such as older
/// messages a "Load older" control brings in above a conversation.
///
/// egui keeps a scroll area's offset from the top of its content, so rows
/// inserted above the view push what the reader was reading down, out of
/// view; `stick_to_bottom` covers only the content's end. This area
/// remembers the first row in view, by the stable key the application
/// gives it with [`ScrollRows::row`], and where that row was. When the row
/// has moved in the content, it moves the offset by as much, in the same
/// frame: it asks egui to lay the pass out again (`request_discard`), so
/// the reader never sees the jump. A reader at the content's end stays at
/// the end: with `stick_to_bottom`, rows that arrive at the end scroll into
/// view as egui scrolls them, and rows that arrive above leave the view
/// where it was.
///
/// Give each row a stable key from the application's own data, such as a
/// message ID, never its position. Content presented outside a row, such
/// as a "Load older" control or a status line, is never the anchor, but
/// rows below it keep their place when it changes. Only the content's own
/// changes move the offset: a wheel, a drag or a scroll to a control
/// scrolls as before.
///
/// ```
/// use stobie_ui_egui::{AnchoredScrollArea, MessageBubbleSpec, MessageSide,
///     PresentationContext, PresentationScope, ReadingTextSpec, SemanticUiId,
///     DesignTokens};
///
/// fn messages(ui: &mut egui::Ui, tokens: DesignTokens, messages: &[(u64, String)]) {
///     let area = egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink(false);
///     AnchoredScrollArea::new("messages", area).show(ui, |ui, rows| {
///         let mut p = PresentationContext::new(ui, tokens, 1.0,
///             PresentationScope::new("messages"), SemanticUiId::root());
///         for (id, text) in messages {
///             rows.row(ui, id, |ui| {
///                 let spec = MessageBubbleSpec { side: MessageSide::Other, sender: "Ada" };
///                 p.message_bubble(ui, ("message", id), spec, |ui, p| {
///                     p.reading_text(ui, "text", text, ReadingTextSpec::default());
///                 });
///             });
///         }
///         p.finish(ui);
///     });
/// }
/// ```
#[derive(Debug)]
#[must_use = "an anchored scroll area presents nothing until it is shown"]
pub struct AnchoredScrollArea {
    area: egui::ScrollArea,
    salt: egui::Id,
}

impl AnchoredScrollArea {
    /// Wrap a vertical scroll area configured as the application wants it,
    /// such as with a height, `stick_to_bottom` or `auto_shrink`. `id_salt`
    /// names the area among its siblings and replaces any salt it had.
    pub fn new(id_salt: impl Hash + Debug, area: egui::ScrollArea) -> Self {
        let salt = egui::Id::new(("stobie.anchored-scroll", id_salt));
        Self {
            area: area.id_salt(salt),
            salt,
        }
    }

    /// Present the content. `add_contents` presents each row through
    /// [`ScrollRows::row`], and may end with [`ScrollRows::footer`].
    pub fn show<R>(
        self,
        ui: &mut Ui,
        add_contents: impl FnOnce(&mut Ui, &mut ScrollRows) -> R,
    ) -> ScrollAreaOutput<R> {
        let id = ui.make_persistent_id(self.salt);
        let mut rows = None;
        let mut output = self.area.show(ui, |ui| {
            let mut scroll_rows = ScrollRows {
                id,
                origin: ui.max_rect().top(),
                view: ui.max_rect().height(),
                rows: Vec::new(),
                footer_shift: 0.0,
            };
            let inner = add_contents(ui, &mut scroll_rows);
            rows = Some(scroll_rows);
            inner
        });
        let Some(rows) = rows else {
            return output;
        };
        let context = ui.ctx().clone();
        let memory = id.with("stobie.scroll-anchor");
        let remembered = context.data(|data| data.get_temp::<Anchor>(memory));
        let view = output.inner_rect.height();
        // A footer that moves changes the content's height in the pass laid
        // out again: the end is where that pass will end, so a footer that
        // grew past the view does not scroll it.
        let end = (output.content_size.y + rows.footer_shift - view).max(0.0);
        let mut offset = output.state.offset.y;
        if offset > end + 0.5 {
            offset = end;
            output.state.offset.y = end;
            output.state.store(&context, output.id);
        }
        let at_end = offset >= end - 0.5;
        if let Some(anchor) = remembered
            && !(anchor.at_end && at_end)
            && let Some(row) = rows.rows.iter().find(|row| row.key == anchor.key)
        {
            let moved = row.top - anchor.top;
            let kept = (offset + moved).clamp(0.0, end);
            if moved.abs() >= 0.5 && (kept - offset).abs() >= 0.5 {
                offset = kept;
                output.state.offset.y = kept;
                output.state.store(&context, output.id);
                context.request_discard("an anchored scroll area keeps the reader's place");
                if !context.will_discard() {
                    context.request_repaint();
                }
            }
        }
        // The first row in view is the reader's place for the next pass.
        let next = rows
            .rows
            .iter()
            .find(|row| row.bottom > offset + 0.5)
            .map(|row| Anchor {
                key: row.key,
                top: row.top,
                at_end: offset >= end - 0.5,
            });
        if next != remembered {
            context.data_mut(|data| match next {
                Some(anchor) => {
                    data.insert_temp(memory, anchor);
                }
                None => {
                    data.remove::<Anchor>(memory);
                }
            });
        }
        output
    }
}

#[derive(Clone, Copy, Debug)]
struct Row {
    key: egui::Id,
    top: f32,
    bottom: f32,
}

/// The rows of an [`AnchoredScrollArea`], presented in order.
#[derive(Debug)]
pub struct ScrollRows {
    id: egui::Id,
    /// The top of the content, on screen, in this pass.
    origin: f32,
    /// The height of the view.
    view: f32,
    rows: Vec<Row>,
    /// How far the footer moves in the pass laid out again.
    footer_shift: f32,
}

impl ScrollRows {
    /// Present one row, named by `key`, a stable key from the
    /// application's data such as a message ID. Each row's key must be
    /// unique in the area.
    pub fn row<R>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + Debug,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> R {
        let top = ui.cursor().top() - self.origin;
        let result = add(ui);
        let bottom = ui.min_rect().bottom() - self.origin;
        self.rows.push(Row {
            key: egui::Id::new(key),
            top,
            bottom: bottom.max(top),
        });
        result
    }

    /// Present `add` as the content's footer, such as an account block
    /// under a list: at the foot of the view while the content above it is
    /// short, and after the last row once it is long. Call it last, directly
    /// in the area's content.
    ///
    /// The footer's height is measured as it is presented and kept for
    /// later passes; when it changes where the footer goes, the area lays
    /// the pass out again (`request_discard`), so it never shows out of
    /// place and needs no repaint from the application.
    pub fn footer<R>(&mut self, ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
        let memory = self.id.with("stobie.scroll-footer");
        let remembered = ui.ctx().data(|data| data.get_temp::<f32>(memory));
        let used = ui.cursor().top() - self.origin;
        // Until the footer has been measured, it follows the content.
        let spare = |height: f32| (self.view - used - height).max(0.0);
        let before = spare(remembered.unwrap_or(self.view));
        if before > 0.0 {
            ui.add_space(before);
        }
        let top = ui.cursor().top();
        let result = add(ui);
        let height = (ui.min_rect().bottom() - top).max(0.0);
        if remembered.is_none_or(|remembered| (remembered - height).abs() >= 0.5) {
            ui.ctx().data_mut(|data| data.insert_temp(memory, height));
            let shift = spare(height) - before;
            if shift.abs() >= 0.5 {
                self.footer_shift = shift;
                ui.ctx()
                    .request_discard("a footer keeps to the foot of its pane");
                if !ui.ctx().will_discard() {
                    ui.ctx().request_repaint();
                }
            }
        }
        result
    }
}
