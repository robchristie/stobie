//! Chat recipes on the presentation context: a message bubble that holds
//! any content, a conversation row, a switch and an unread dot.

use std::hash::{DefaultHasher, Hash, Hasher};

use egui::{Color32, Response, Shape, Ui, UiBuilder, pos2, vec2};

use super::PresentationContext;
use crate::{
    ActionArgument, ActionKey, ActionTarget, Availability, ConversationRowSpec, DesignTokens,
    SwitchSpec, UiNode, UiRole,
};

/// Which side of a conversation a message is from.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MessageSide {
    /// The person using the application: at the end of the line, on an
    /// accented fill.
    Own,
    /// Anyone else, such as another person or an assistant: at the start of
    /// the line, on a quiet raised fill.
    Other,
}

/// One message's bubble. The content is the caller's, presented inside it.
#[derive(Clone, Copy, Debug)]
pub struct MessageBubbleSpec<'a> {
    pub side: MessageSide,
    /// Who sent the message, in the application's words, such as "You" or
    /// a name. It names the bubble for assistive technology and agents; the
    /// application shows a name or avatar beside the bubble when it wants
    /// one in view.
    pub sender: &'a str,
}

/// The bubble's fill: the selection tint for the person's own messages, the
/// raised surface for anyone else's. Primary text is validated on both;
/// [`message_bubble_tokens`] keeps muted text legible on them.
pub fn message_bubble_fill(tokens: &DesignTokens, side: MessageSide) -> Color32 {
    match side {
        MessageSide::Own => tokens.colours.selection_background.into(),
        MessageSide::Other => tokens.colours.surface_raised.into(),
    }
}

/// The tokens for a bubble's content: its surfaces are the bubble's fill,
/// so a link's colour is checked against the fill, and the raised and hover
/// surfaces step back to the canvas so a code block or a hovered control
/// still stands apart from the fill. Muted text becomes primary text on a
/// fill too close to it.
pub fn message_bubble_tokens(tokens: &DesignTokens, side: MessageSide) -> DesignTokens {
    let mut inner = *tokens;
    inner.colours.text_muted =
        crate::components::muted_text_on(tokens, message_bubble_fill(tokens, side));
    let colours = &mut inner.colours;
    let canvas = tokens.colours.surface_canvas;
    match side {
        MessageSide::Own => {
            let fill = tokens.colours.selection_background;
            colours.surface_canvas = fill;
            colours.surface_panel = fill;
            colours.surface_raised = canvas;
            colours.surface_hover = canvas;
            colours.action_quiet_hover = canvas;
        }
        MessageSide::Other => {
            let fill = tokens.colours.surface_raised;
            colours.surface_canvas = fill;
            colours.surface_panel = fill;
            colours.surface_raised = canvas;
        }
    }
    inner
}

/// The widest a bubble may be in a column `available` points wide: 85% of
/// the column, at most the token width grown with text to 150%, and never
/// wider than the column.
pub fn message_bubble_max_width(tokens: &DesignTokens, font_scale: f32, available: f32) -> f32 {
    let scale = if font_scale.is_finite() {
        font_scale.clamp(1.0, 1.5)
    } else {
        1.0
    };
    let minimum = tokens.geometry.minimum_hit_size.0 * 2.0;
    (available * 0.85)
        .min(tokens.chat_bubble_max_width().0 * scale)
        .max(minimum)
        .min(available.max(0.0))
}

/// The width a bubble's content was last laid out at, kept between passes
/// so a short message hugs its text.
#[derive(Clone, Copy, Debug, PartialEq)]
struct BubbleFit {
    /// The widest content width the bubble allowed when it was measured.
    max: f32,
    /// The content's natural width at that maximum.
    fit: f32,
    /// The content's text and how it was set: its lines and their height.
    signature: u64,
}

/// The rightmost edge among `shapes`, and a hash of their text and how it
/// was set. Text counts to the end of its laid-out lines, not its glyphs'
/// ink, so the measured width sets the text the same way again; its line
/// count and height change when a density, contrast or text size sets it
/// afresh, so the bubble measures it again.
fn measure_shapes<'a>(
    shapes: impl Iterator<Item = &'a Shape>,
    right: &mut f32,
    hasher: &mut DefaultHasher,
) {
    for shape in shapes {
        match shape {
            Shape::Noop => {}
            Shape::Vec(shapes) => measure_shapes(shapes.iter(), right, hasher),
            Shape::Text(text) => {
                let galley = &text.galley;
                galley.text().hash(hasher);
                galley.rows.len().hash(hasher);
                galley.rect.height().to_bits().hash(hasher);
                *right = right.max(text.pos.x + galley.rect.max.x);
            }
            shape => {
                let rect = shape.visual_bounding_rect();
                if rect.is_finite() {
                    *right = right.max(rect.max.x);
                }
            }
        }
    }
}

impl PresentationContext {
    /// One message in a conversation: the person's own on the right on an
    /// accented fill, anyone else's on the left on a quiet one, holding any
    /// content `render` presents, such as [`Self::markdown`] or
    /// [`Self::reading_text`]. The bubble is at most 85% of the column and
    /// the token width, hugs shorter content and grows down with long
    /// content, which wraps. It publishes a [`UiRole::Section`] named by
    /// `spec.sender` that holds the content's nodes; the content's text
    /// keeps its own kinds.
    ///
    /// A bubble measures its content after presenting it and keeps the
    /// width for later passes; when the content's text, the way it is set
    /// (a density, contrast or text size) or the column's width changes, it
    /// lays out once more at the new width.
    ///
    /// ```
    /// use stobie_ui_egui::{MarkdownSpec, MessageBubbleSpec, MessageSide,
    ///     PresentationContext, ReadingTextSpec};
    ///
    /// // The messages and their senders belong to the application.
    /// fn messages(ui: &mut egui::Ui, p: &mut PresentationContext) {
    ///     let own = MessageBubbleSpec { side: MessageSide::Own, sender: "You" };
    ///     p.message_bubble(ui, ("message", 41), own, |ui, p| {
    ///         p.reading_text(ui, "text", "Is the hall booked for Friday?",
    ///             ReadingTextSpec::default());
    ///     });
    ///     let reply = MessageBubbleSpec { side: MessageSide::Other, sender: "Ada" };
    ///     p.message_bubble(ui, ("message", 42), reply, |ui, p| {
    ///         p.markdown(ui, "text", "Yes: **6 pm**, and the key is at the desk.",
    ///             MarkdownSpec::default());
    ///     });
    /// }
    /// ```
    pub fn message_bubble<R>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: MessageBubbleSpec<'_>,
        render: impl FnOnce(&mut Ui, &mut Self) -> R,
    ) -> R {
        self.check_pass(ui);
        let identity = self.scope.instance(&key);
        let tokens = self.tokens;
        let padding = vec2(tokens.spacing.section.0, tokens.spacing.inline.0);
        let origin = ui.cursor().min;
        let available = ui.available_width().max(0.0);
        let outer = message_bubble_max_width(&tokens, self.font_scale, available);
        let maximum = (outer - padding.x * 2.0).max(1.0);
        let minimum = (tokens.geometry.minimum_hit_size.0 - padding.x * 2.0)
            .max(1.0)
            .min(maximum);
        let fit_id = identity.egui_id().with("stobie.message-bubble.fit");
        let remembered = ui.ctx().data(|data| data.get_temp::<BubbleFit>(fit_id));
        let width = match remembered {
            Some(fit) if (fit.max - maximum).abs() < 0.5 => fit.fit.clamp(minimum, maximum),
            _ => maximum,
        };
        let left = match spec.side {
            MessageSide::Own => origin.x + available - padding.x - width,
            MessageSide::Other => origin.x + padding.x,
        };
        let layer = ui.layer_id();
        // The fill goes beneath the content, once its height is known.
        let background = ui.painter().add(Shape::Noop);
        let first = ui
            .ctx()
            .graphics(|graphics| graphics.get(layer).map_or(0, |list| list.next_idx().0));
        let mut content = ui.new_child(
            UiBuilder::new()
                .id_salt(identity.egui_id())
                .max_rect(egui::Rect::from_min_size(
                    pos2(left, origin.y + padding.y),
                    vec2(width, ui.available_height().max(1.0)),
                ))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let fill = message_bubble_fill(&tokens, spec.side);
        if spec.side == MessageSide::Own {
            // Selected text stays visible on the selection-tinted fill.
            content.visuals_mut().selection.bg_fill = tokens.colours.surface_canvas.into();
        }
        let semantic_id = identity.semantic_id();
        content
            .ctx()
            .accesskit_node_builder(content.unique_id(), |node| {
                node.set_role(egui::accesskit::Role::Group);
                node.set_label(spec.sender);
                node.set_author_id(semantic_id.0.clone());
            });
        let mut child = Self::new(
            &mut content,
            message_bubble_tokens(&tokens, spec.side),
            self.font_scale,
            self.scope.child(&key),
            semantic_id.clone(),
        );
        child.domain_reference = self.domain_reference.clone();
        child.pane = self.pane;
        child.visible_only = self.visible_only;
        let result = render(&mut content, &mut child);
        child.check_pass(&content);
        let content_rect = content.min_rect();
        let bubble = egui::Rect::from_min_max(
            pos2(left - padding.x, origin.y),
            pos2(
                left + width + padding.x,
                content_rect.bottom().max(origin.y + padding.y) + padding.y,
            ),
        );
        let radius = tokens.chat_bubble_radius().0 * self.font_scale.clamp(1.0, 1.5);
        let border: Color32 = tokens.colours.border_subtle.into();
        ui.painter().set(
            background,
            egui::epaint::RectShape::new(
                bubble,
                radius.min(bubble.height() * 0.5),
                fill,
                egui::Stroke::new(1.0, border),
                egui::StrokeKind::Inside,
            ),
        );
        let row = egui::Rect::from_min_max(origin, pos2(origin.x + available, bubble.bottom()));
        ui.allocate_rect(row, egui::Sense::hover());
        // Measure the content's natural width for later passes, while the
        // bubble is in view: content out of view may skip its paint.
        if ui.is_rect_visible(bubble) {
            let mut right = f32::NEG_INFINITY;
            let mut hasher = DefaultHasher::new();
            ui.ctx().graphics(|graphics| {
                if let Some(list) = graphics.get(layer) {
                    measure_shapes(
                        list.all_entries().skip(first).map(|clipped| &clipped.shape),
                        &mut right,
                        &mut hasher,
                    );
                }
            });
            let signature = hasher.finish();
            let natural = if right.is_finite() {
                (right - left).ceil().clamp(minimum, maximum)
            } else {
                maximum
            };
            let laid_out_at_maximum = (width - maximum).abs() < 0.5;
            let next = if laid_out_at_maximum {
                Some(BubbleFit {
                    max: maximum,
                    fit: natural,
                    signature,
                })
            } else if remembered.is_some_and(|fit| fit.signature != signature)
                || natural > width + 0.5
            {
                // The text changed while laid out narrower than the
                // maximum: measure it again at the maximum.
                None
            } else {
                remembered
            };
            if next != remembered {
                ui.ctx().data_mut(|data| {
                    if let Some(fit) = next {
                        data.insert_temp(fit_id, fit);
                    } else {
                        data.remove::<BubbleFit>(fit_id);
                    }
                });
                let next_width = next.map_or(maximum, |fit| fit.fit);
                if (next_width - width).abs() >= 0.5 {
                    ui.ctx()
                        .request_discard("a message bubble fits its content");
                    if !ui.ctx().will_discard() {
                        ui.ctx().request_repaint();
                    }
                }
            }
        }
        // The bubble's node leads its content's nodes, which it holds.
        let mut node = UiNode::container(
            semantic_id,
            Some(self.parent.clone()),
            UiRole::Section,
            bubble.into(),
        );
        node.name = spec.sender.to_owned();
        node.domain_reference = self.domain_reference.clone();
        self.push_node(ui, node);
        self.text_layouts.append(&mut child.text_layouts);
        self.semantic_nodes.append(&mut child.semantic_nodes);
        self.node_clips.append(&mut child.node_clips);
        self.raw_presentations.append(&mut child.raw_presentations);
        result
    }

    /// One conversation in a list (see [`crate::conversation_row`]), whose
    /// surface performs `target` for that conversation, carrying
    /// `argument`, such as its ID. Its identity combines `key` with the
    /// argument. It publishes a [`UiRole::NavigationItem`] named by the
    /// title, `selected` while the conversation is on show, whose
    /// description holds the unread words, preview and time. Hosted
    /// applications use [`Frame::conversation_row`](crate::Frame::conversation_row),
    /// which triggers the action. Hidden availability returns an empty
    /// hover response without observations.
    pub fn conversation_row<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: ConversationRowSpec<'_>,
        target: ActionTarget<A>,
        availability: &Availability,
        argument: &ActionArgument,
    ) -> Response {
        self.check_pass(ui);
        let value = argument.to_value();
        let identity = self.argument_identity(key, value.as_ref());
        if !availability.visible() {
            return ui.interact(
                egui::Rect::NOTHING,
                identity.egui_id(),
                egui::Sense::hover(),
            );
        }
        let response = crate::conversation_row(
            ui,
            &spec,
            target,
            availability,
            identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if response.interact_rect.is_positive() {
            let mut node = crate::conversation_row_semantic_node(
                &response,
                &spec,
                target,
                availability,
                value,
                identity,
                self.parent.clone(),
            );
            if self.domain_reference.is_some() {
                node.domain_reference = self.domain_reference.clone();
            }
            self.push_node(ui, node);
        }
        response
    }

    /// One on or off setting (see [`crate::switch`]), whose surface
    /// performs `target`, carrying `argument` when one switch stands for
    /// one of several objects. Its identity combines `key` with the
    /// argument. It publishes a [`UiRole::Switch`] named by the label whose
    /// `checked` is its state. Hosted applications use
    /// [`Frame::switch`](crate::Frame::switch), which triggers the action.
    /// Hidden availability returns an empty hover response without
    /// observations.
    pub fn switch<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        spec: SwitchSpec<'_>,
        target: ActionTarget<A>,
        availability: &Availability,
        argument: &ActionArgument,
    ) -> Response {
        self.check_pass(ui);
        let value = argument.to_value();
        let identity = self.argument_identity(key, value.as_ref());
        if !availability.visible() {
            return ui.interact(
                egui::Rect::NOTHING,
                identity.egui_id(),
                egui::Sense::hover(),
            );
        }
        let response = crate::switch(
            ui,
            &spec,
            target,
            availability,
            identity,
            &self.tokens,
            self.font_scale,
            &mut self.text_layouts,
        );
        if response.interact_rect.is_positive() {
            let mut node = crate::switch_semantic_node(
                &response,
                &spec,
                target,
                availability,
                value,
                identity,
                self.parent.clone(),
            );
            if self.domain_reference.is_some() {
                node.domain_reference = self.domain_reference.clone();
            }
            self.push_node(ui, node);
        }
        response
    }

    /// A decorative unread dot at its size (see [`crate::unread_dot`]). It
    /// adds no semantic node or text observation: say that something is
    /// unread in the control or text beside it, as a conversation row's
    /// `unread` words do.
    pub fn unread_dot(&mut self, ui: &mut Ui) -> Response {
        self.check_pass(ui);
        crate::unread_dot(ui, &self.tokens, self.font_scale)
    }

    /// The identity of a control that carries `argument`: `key` alone
    /// without one.
    fn argument_identity(
        &self,
        key: impl Hash + std::fmt::Debug,
        argument: Option<&serde_json::Value>,
    ) -> super::PresentationId {
        match argument {
            Some(value) => self
                .scope
                .instance((key, "stobie.argument", value.to_string())),
            None => self.scope.instance(key),
        }
    }
}
