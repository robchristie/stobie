//! A heading and the controls at its end, on one row: the heading fits the
//! room the controls leave, so the controls never squeeze out of sight.

use std::hash::Hash;

use egui::{Align, Layout, Rect, Response, Ui, UiBuilder, Vec2, pos2, vec2};

use super::{PresentationContext, PresentationScope};
use crate::{
    ActionArgument, ActionButtonContent, ActionButtonSpec, ActionButtonState, ActionEmphasis,
    ActionKey, ActionTarget, Availability, BoundAction, IconId, IntoActionTarget, TextOverflow,
    TextRole, TextRoleStyles, TextSpec,
    components::{action_button_size, action_hit_height},
};

/// One control at the end of a heading row
/// ([`PresentationContext::heading_row`]) or of a navigation header
/// ([`NavigationOptions::with_control`](crate::NavigationOptions::with_control)):
/// an action presented as a button, icon-only unless built otherwise.
///
/// Pass the action, an [`ActionTarget`] or a [`BoundAction`] carrying the
/// argument the control acts on, such as the open conversation's ID. Each
/// control's action, with its argument, appears once in a row.
#[derive(Clone, Debug, PartialEq)]
pub struct HeaderControl<A: ActionKey> {
    action: BoundAction<A>,
    content: ActionButtonContent,
    pressed: Option<bool>,
    emphasis: ActionEmphasis,
}

impl<A: ActionKey> HeaderControl<A> {
    /// An icon-only control named by its action's label.
    pub fn icon(action: impl Into<BoundAction<A>>, icon: IconId) -> Self {
        Self::new(action, ActionButtonContent::IconOnly(icon))
    }

    /// A control presented as `content`, such as its action's label.
    pub fn new(action: impl Into<BoundAction<A>>, content: ActionButtonContent) -> Self {
        Self {
            action: action.into(),
            content,
            pressed: None,
            emphasis: ActionEmphasis::QuietBorderless,
        }
    }

    /// Make the control a toggle whose pressed state the application owns,
    /// such as Pinned; its action turns the state over.
    pub fn pressed(mut self, pressed: bool) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// Present the control with `emphasis` instead of quiet and borderless.
    pub fn with_emphasis(mut self, emphasis: ActionEmphasis) -> Self {
        self.emphasis = emphasis;
        self
    }
}

/// A heading row's or navigation header's control as presented: its
/// resolved target, the argument it carries and its response. Hosted
/// applications' frames trigger it when clicked.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct HeaderControlResponse<A: ActionKey> {
    pub target: ActionTarget<A>,
    pub argument: ActionArgument,
    pub response: Response,
}

/// What [`PresentationContext::heading_row`] presented.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct HeadingRowResponse<A: ActionKey> {
    /// The heading's response.
    pub heading: Response,
    /// Each control shown, in the order given; hidden ones are absent.
    pub controls: Vec<HeaderControlResponse<A>>,
    /// Whether the controls took a line of their own below the heading
    /// because beside it they would have left it too little room.
    pub wrapped: bool,
}

/// A visible control with its target resolved and its width measured.
pub(super) struct ResolvedControl<A: ActionKey> {
    bound: BoundAction<A>,
    availability: Availability,
    content: ActionButtonContent,
    state: ActionButtonState,
    emphasis: ActionEmphasis,
    pub(super) width: f32,
}

/// Where a row's leading items, its title and its trailing controls go.
pub(super) struct RowPlan {
    pub leading: Vec<Rect>,
    pub title: Rect,
    pub trailing: Vec<Rect>,
    /// Whether the trailing controls took a line of their own.
    pub wrapped: bool,
    pub height: f32,
}

/// The geometry a row is laid out with.
pub(super) struct RowMetrics {
    /// The first line's height; everything on it is centred on it.
    pub line: f32,
    /// A control's hit height.
    pub hit: f32,
    /// The title's height.
    pub title: f32,
    /// The space between neighbouring items.
    pub gap: f32,
    /// The least width the title keeps beside the trailing controls.
    pub min_title: f32,
}

/// Lay a row out across `left..right` from `top`: `leading` items from the
/// start, `trailing` controls (by width) from the end, and the title in the
/// room between them, a gap from each. When the trailing controls would leave
/// the title less than its least width, they take a line of their own, a
/// gap below the first and end-aligned, and the title runs to `right`.
pub(super) fn plan_row(
    left: f32,
    right: f32,
    top: f32,
    metrics: &RowMetrics,
    leading: &[Vec2],
    trailing: &[f32],
) -> RowPlan {
    let RowMetrics {
        line,
        hit,
        title,
        gap,
        min_title,
    } = *metrics;
    let centre = top + line * 0.5;
    let mut start = left;
    let leading = leading
        .iter()
        .map(|size| {
            let rect = Rect::from_min_size(pos2(start, centre - size.y * 0.5), *size);
            start += size.x + gap;
            rect
        })
        .collect();
    let needed: f32 = trailing.iter().map(|width| width + gap).sum();
    let wrapped = !trailing.is_empty() && right - needed - start < min_title;
    let (trailing_centre, height) = if wrapped {
        (top + line + gap + hit * 0.5, line + gap + hit)
    } else {
        (centre, line)
    };
    let mut end = right;
    let mut rects = vec![Rect::NOTHING; trailing.len()];
    for (rect, width) in rects.iter_mut().zip(trailing).rev() {
        end -= width;
        *rect = Rect::from_min_size(pos2(end, trailing_centre - hit * 0.5), vec2(*width, hit));
        end -= gap;
    }
    let title_right = if wrapped { right } else { end };
    RowPlan {
        leading,
        title: Rect::from_min_max(
            pos2(start.min(title_right), centre - title * 0.5),
            pos2(title_right.max(start), centre + title * 0.5),
        ),
        trailing: rects,
        wrapped,
        height,
    }
}

/// The least width a heading keeps beside its controls: its whole text when
/// that is shorter, otherwise three minimum hit targets at the text scale.
pub(super) fn minimum_title_width(
    ui: &Ui,
    text: &str,
    tokens: &crate::DesignTokens,
    font_scale: f32,
) -> f32 {
    let least = tokens.geometry.minimum_hit_size.0 * 3.0 * font_scale.clamp(1.0, 1.5);
    // Expanding text measures whole whatever the width; a failed measure
    // keeps the least width, so the controls make room rather than crowd.
    crate::measure_text(
        ui.painter(),
        text,
        TextSpec::single_line(TextRole::SectionHeading, TextOverflow::Expand),
        tokens,
        font_scale,
        least,
    )
    .map_or(least, |measured| measured.size().x.min(least))
}

impl PresentationContext {
    /// A heading with controls at its end, on one row, such as a section's
    /// title with Edit, or a page's title with Pin and Share.
    ///
    /// The controls keep their whole size at the row's end, in the order
    /// given, and the heading takes the room they leave, eliding with its
    /// full text kept for assistive technology and its tooltip. When beside
    /// the controls the heading would have less than its least width (its
    /// whole text, or three minimum hit targets), the controls take a line
    /// of their own below it, end-aligned, so no control is ever squeezed or
    /// pushed out of sight. A hidden control is not presented.
    /// `availability` gives each control's availability; hosted
    /// applications use [`Frame::heading_row`](crate::Frame::heading_row),
    /// which also triggers a clicked control.
    ///
    /// Use this rather than a [`Self::heading`] followed by controls in
    /// `ui.horizontal`: a heading takes the whole width it is given, which
    /// leaves nothing for controls after it.
    ///
    /// ```
    /// use stobie_ui_egui::{
    ///     ActionKey, ActionTarget, Availability, HeaderControl, IconId, PresentationContext,
    /// };
    ///
    /// // A is the application's own ActionKey enum; edit is one of its actions.
    /// fn section<A: ActionKey>(ui: &mut egui::Ui, p: &mut PresentationContext, edit: A) -> bool {
    ///     let row = p.heading_row(
    ///         ui,
    ///         "routines",
    ///         "Morning routines",
    ///         &[HeaderControl::icon(edit, IconId::Edit)],
    ///         |_, _| Availability::Enabled,
    ///     );
    ///     row.controls.iter().any(|control| control.response.clicked())
    /// }
    /// ```
    pub fn heading_row<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        key: impl Hash + std::fmt::Debug,
        text: &str,
        controls: &[HeaderControl<A>],
        availability: impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
    ) -> HeadingRowResponse<A> {
        self.check_pass(ui);
        let tokens = self.tokens;
        let font_scale = self.font_scale;
        let scope = self.scope.child(key);
        let resolved = self.resolve_controls(ui, controls, &availability);
        let hit = action_hit_height(&tokens, font_scale);
        let heading = TextRole::SectionHeading
            .style(&tokens, font_scale)
            .line_height
            + tokens.spacing.block.0;
        let metrics = RowMetrics {
            line: hit.max(heading),
            hit,
            title: heading,
            gap: tokens.spacing.unit.0,
            min_title: minimum_title_width(ui, text, &tokens, font_scale),
        };
        let area = ui.available_rect_before_wrap();
        let widths: Vec<f32> = resolved.iter().map(|control| control.width).collect();
        let plan = plan_row(
            area.left(),
            area.right(),
            area.top(),
            &metrics,
            &[],
            &widths,
        );
        let (_, row) = ui.allocate_space(vec2(area.width(), plan.height));
        let offset = row.min - area.min;
        let heading = ui
            .scope_builder(
                UiBuilder::new()
                    .max_rect(plan.title.translate(offset))
                    .layout(Layout::top_down(Align::Min)),
                |ui| self.heading_with_identity(ui, scope.instance("heading"), text),
            )
            .inner;
        let controls = resolved
            .iter()
            .zip(&plan.trailing)
            .map(|(control, rect)| {
                self.present_header_control(ui, scope, rect.translate(offset), control)
            })
            .collect();
        HeadingRowResponse {
            heading,
            controls,
            wrapped: plan.wrapped,
        }
    }

    /// Resolve each control's target against this context's pane, keep the
    /// visible ones and measure them.
    pub(super) fn resolve_controls<A: ActionKey>(
        &self,
        ui: &Ui,
        controls: &[HeaderControl<A>],
        availability: &impl Fn(ActionTarget<A>, &ActionArgument) -> Availability,
    ) -> Vec<ResolvedControl<A>> {
        controls
            .iter()
            .filter_map(|control| {
                let bound = control.action.clone().into_bound(self.pane);
                let availability = availability(bound.target(), bound.argument());
                if !availability.visible() {
                    return None;
                }
                let label = bound.action().specification().label;
                let size =
                    action_button_size(ui, label, control.content, &self.tokens, self.font_scale);
                let width = size.intrinsic.max(size.minimum_width);
                Some(ResolvedControl {
                    bound,
                    availability,
                    content: control.content,
                    state: control
                        .pressed
                        .map_or(ActionButtonState::Momentary, |pressed| {
                            ActionButtonState::Toggle { pressed }
                        }),
                    emphasis: control.emphasis,
                    width,
                })
            })
            .collect()
    }

    /// Present one resolved control in `rect`, identified within `scope` by
    /// its action and argument.
    pub(super) fn present_header_control<A: ActionKey>(
        &mut self,
        ui: &mut Ui,
        scope: PresentationScope,
        rect: Rect,
        control: &ResolvedControl<A>,
    ) -> HeaderControlResponse<A> {
        let bound = &control.bound;
        let target = bound.target();
        let action = bound.action().stable_id();
        let value = bound.argument().to_value();
        let identity = match &value {
            Some(value) => scope
                .instance((action, "stobie.argument", value.to_string()))
                .action_identity(),
            None => scope.instance(action).action_identity(),
        };
        let name = bound.subject().map(|_| bound.accessible_name());
        let spec = ActionButtonSpec {
            target,
            availability: control.availability.clone(),
            state: control.state,
            emphasis: control.emphasis,
            compact: false,
        };
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                self.present_action(ui, &identity, spec, control.content, name.as_deref(), value)
            })
            .inner;
        HeaderControlResponse {
            target,
            argument: bound.argument().clone(),
            response,
        }
    }
}

impl<A: ActionKey> From<A> for BoundAction<A> {
    /// The action carrying no argument, its pane resolved where it is
    /// presented.
    fn from(action: A) -> Self {
        BoundAction::new(action, ActionArgument::None)
    }
}

impl<A: ActionKey> From<ActionTarget<A>> for BoundAction<A> {
    /// The target carrying no argument.
    fn from(target: ActionTarget<A>) -> Self {
        BoundAction::for_target(target, ActionArgument::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics() -> RowMetrics {
        RowMetrics {
            line: 40.0,
            hit: 32.0,
            title: 20.0,
            gap: 4.0,
            min_title: 96.0,
        }
    }

    #[test]
    fn a_row_keeps_its_controls_whole_and_gives_the_title_the_rest() {
        let plan = plan_row(
            10.0,
            390.0,
            0.0,
            &metrics(),
            &[vec2(32.0, 32.0)],
            &[32.0, 32.0],
        );
        assert!(!plan.wrapped);
        assert_eq!(plan.height, 40.0);
        assert_eq!(
            plan.leading[0],
            Rect::from_min_size(pos2(10.0, 4.0), vec2(32.0, 32.0))
        );
        assert_eq!(plan.trailing[1].max.x, 390.0);
        assert_eq!(plan.trailing[0].max.x, plan.trailing[1].min.x - 4.0);
        assert_eq!(plan.title.min.x, 46.0);
        assert_eq!(plan.title.max.x, plan.trailing[0].min.x - 4.0);
        assert_eq!(plan.title.center().y, 20.0);
    }

    #[test]
    fn controls_that_would_crowd_the_title_take_a_line_of_their_own() {
        // Beside four controls the title would keep only 92 points.
        let plan = plan_row(0.0, 236.0, 0.0, &metrics(), &[], &[32.0; 4]);
        assert!(plan.wrapped);
        assert_eq!(plan.height, 40.0 + 4.0 + 32.0);
        assert_eq!(plan.title.max.x, 236.0);
        assert_eq!(plan.trailing[3].max.x, 236.0);
        assert!(plan.trailing.iter().all(|rect| rect.min.y == 44.0));
        // One point wider, they fit beside it.
        assert!(!plan_row(0.0, 240.0, 0.0, &metrics(), &[], &[32.0; 4]).wrapped);
    }
}
