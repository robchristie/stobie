use std::sync::Arc;

use egui::{
    Align, Color32, FontFamily, FontId, Galley, Id, Painter, Pos2, Rect, Sense, TextFormat, Ui,
    UiBuilder,
    text::{LayoutJob, TextWrapping},
    text_selection::LabelSelectionState,
};
use serde::{Deserialize, Serialize};

use crate::{DesignTokens, FontWeight};

pub use stobie_protocol::text::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextSpec {
    pub role: TextRole,
    pub overflow: TextOverflow,
    pub interaction: TextInteraction,
    pub horizontal_alignment: HorizontalTextAlignment,
    pub vertical_alignment: VerticalTextAlignment,
    pub max_lines: u8,
}

impl TextSpec {
    pub const fn single_line(role: TextRole, overflow: TextOverflow) -> Self {
        Self {
            role,
            overflow,
            interaction: TextInteraction::Inert,
            horizontal_alignment: HorizontalTextAlignment::Start,
            vertical_alignment: VerticalTextAlignment::Centre,
            max_lines: 1,
        }
    }

    /// Bounded recipes accept 1 to [`MAX_TEXT_LINES`] lines. Unbounded
    /// wrapping belongs to [`crate::reading_text`] and [`crate::link`].
    pub fn validate(self) -> Result<Self, TextLayoutError> {
        if self.overflow == TextOverflow::WrapUnbounded {
            return Err(TextLayoutError::UnboundedOverflow);
        }
        if !(1..=MAX_TEXT_LINES).contains(&self.max_lines) {
            return Err(TextLayoutError::InvalidMaxLines(self.max_lines));
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextRoleStyle {
    pub font_id: FontId,
    pub weight: FontWeight,
    /// Logical line height, including the current font scale.
    pub line_height: f32,
    pub colour: Color32,
}

/// The design system's style for a text role.
pub trait TextRoleStyles {
    /// Font, line height and colour for this role at `font_scale`.
    fn style(self, tokens: &DesignTokens, font_scale: f32) -> TextRoleStyle;
}

impl TextRoleStyles for TextRole {
    fn style(self, tokens: &DesignTokens, font_scale: f32) -> TextRoleStyle {
        let scale = if font_scale.is_finite() {
            font_scale.clamp(1.0, 1.5)
        } else {
            1.0
        };
        let typography = tokens.typography;
        let size = match self {
            Self::ApplicationTitle => typography.application_title_size.0,
            Self::PaneTitle => typography.pane_title_size.0,
            Self::SectionHeading => typography.section_heading_size.0,
            Self::ButtonLabel | Self::TabLabel => typography.label_size.0,
            Self::Secondary | Self::Status => typography.secondary_size.0,
            Self::Caption => typography.caption_size.0,
            _ => typography.body_size.0,
        };
        let semibold = matches!(
            self,
            Self::ApplicationTitle
                | Self::PaneTitle
                | Self::SectionHeading
                | Self::ButtonLabel
                | Self::TabLabel
        );
        let weight = if semibold {
            typography.label_weight
        } else {
            typography.body_weight
        };
        let colour = match self {
            Self::Secondary | Self::Caption | Self::Status => tokens.colours.text_muted,
            Self::Error => tokens.colours.status_error,
            _ => tokens.colours.text_primary,
        };
        let family = if self == Self::MonospaceTechnical {
            FontFamily::Monospace
        } else if semibold {
            FontFamily::Name(crate::SEMIBOLD_FONT_FAMILY.into())
        } else {
            FontFamily::Name(crate::REGULAR_FONT_FAMILY.into())
        };
        let line_height = if matches!(
            self,
            Self::ApplicationTitle | Self::PaneTitle | Self::SectionHeading
        ) {
            typography.heading_line_height.0
        } else {
            typography.line_height.0
        };
        TextRoleStyle {
            font_id: FontId::new(size * scale, family),
            weight,
            line_height: size * scale * line_height,
            colour: colour.into(),
        }
    }
}

impl TextRoleStyle {
    /// Apply the identical font, line height and emphasis to a native egui widget.
    pub fn rich_text(&self, text: impl Into<String>) -> egui::RichText {
        egui::RichText::new(text)
            .font(self.font_id.clone())
            .line_height(Some(self.line_height))
            .color(self.colour)
    }
}

#[derive(Clone)]
pub struct MeasuredText {
    pub galley: Arc<Galley>,
    pub spec: TextSpec,
    pub colour: Color32,
    pub layout_error: Option<TextLayoutError>,
}

impl MeasuredText {
    pub fn size(&self) -> egui::Vec2 {
        self.galley.size()
    }

    pub fn truncated(&self) -> bool {
        self.galley.elided
    }
}

pub fn measure_text(
    painter: &Painter,
    text: &str,
    spec: TextSpec,
    tokens: &DesignTokens,
    font_scale: f32,
    max_width: f32,
) -> Result<MeasuredText, TextLayoutError> {
    let spec = spec.validate()?;
    if !font_scale.is_finite() || !(1.0..=1.5).contains(&font_scale) {
        return Err(TextLayoutError::InvalidFontScale);
    }
    if !max_width.is_finite() || max_width <= 0.0 {
        return Err(TextLayoutError::InvalidWidth);
    }
    let role_style = spec.role.style(tokens, font_scale);
    if !painter.ctx().fonts(|fonts| {
        fonts
            .definitions()
            .families
            .contains_key(&role_style.font_id.family)
    }) {
        return Err(TextLayoutError::FontsNotInstalled);
    }
    let mut job =
        LayoutJob::simple_singleline(text.to_owned(), role_style.font_id, role_style.colour);
    // Position the complete galley explicitly when painting. Keeping the job
    // left-origin avoids double-applying centre/end alignment offsets.
    job.halign = Align::Min;
    job.wrap = match spec.overflow {
        TextOverflow::Clip | TextOverflow::Scroll | TextOverflow::Expand => {
            TextWrapping::no_max_width()
        }
        TextOverflow::Wrap => TextWrapping {
            max_width,
            max_rows: usize::from(spec.max_lines),
            break_anywhere: false,
            overflow_character: Some('…'),
        },
        TextOverflow::Ellipsis => TextWrapping {
            max_width,
            max_rows: usize::from(spec.max_lines),
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        TextOverflow::WrapUnbounded => unreachable!("validation rejects unbounded text"),
    };
    job.break_on_newline = spec.overflow == TextOverflow::Wrap;
    job.sections[0].format = TextFormat {
        line_height: Some(role_style.line_height),
        ..job.sections[0].format.clone()
    };
    let galley = painter.layout_job(job);
    Ok(MeasuredText {
        galley,
        spec,
        colour: role_style.colour,
        layout_error: None,
    })
}

/// Measure reading text without a line limit: `job` carries the text and its
/// styled sections, and this sets the wrapping width, start alignment, line
/// breaks at newlines and each section's role line height. Plain text and
/// later rich text (styled runs, links, code) share this one measured path, so
/// every reading surface is measured, observed and audited alike.
pub(crate) fn measure_reading_job(
    painter: &Painter,
    mut job: LayoutJob,
    role: TextRole,
    interaction: TextInteraction,
    tokens: &DesignTokens,
    font_scale: f32,
    max_width: f32,
) -> Result<MeasuredText, TextLayoutError> {
    if !font_scale.is_finite() || !(1.0..=1.5).contains(&font_scale) {
        return Err(TextLayoutError::InvalidFontScale);
    }
    if !max_width.is_finite() || max_width <= 0.0 {
        return Err(TextLayoutError::InvalidWidth);
    }
    let role_style = role.style(tokens, font_scale);
    let installed = painter.ctx().fonts(|fonts| {
        job.sections.iter().all(|section| {
            fonts
                .definitions()
                .families
                .contains_key(&section.format.font_id.family)
        })
    });
    if !installed {
        return Err(TextLayoutError::FontsNotInstalled);
    }
    job.halign = Align::Min;
    job.wrap = TextWrapping {
        max_width,
        max_rows: usize::MAX,
        break_anywhere: false,
        overflow_character: None,
    };
    job.break_on_newline = true;
    for section in &mut job.sections {
        section.format.line_height = Some(role_style.line_height);
    }
    let galley = painter.layout_job(job);
    Ok(MeasuredText {
        galley,
        spec: TextSpec {
            role,
            overflow: TextOverflow::WrapUnbounded,
            interaction,
            horizontal_alignment: HorizontalTextAlignment::Start,
            vertical_alignment: VerticalTextAlignment::Top,
            max_lines: 0,
        },
        colour: role_style.colour,
        layout_error: None,
    })
}

/// A one-section job in `role`'s font and `colour`, ready for
/// [`measure_reading_job`].
pub(crate) fn reading_job(
    text: &str,
    role: TextRole,
    colour: Color32,
    tokens: &DesignTokens,
    font_scale: f32,
) -> LayoutJob {
    let style = role.style(tokens, font_scale);
    LayoutJob::single_section(
        text.to_owned(),
        TextFormat {
            line_height: Some(style.line_height),
            ..TextFormat::simple(style.font_id, colour)
        },
    )
}

/// The failure fallback shared by component measurement: a visible one-line
/// diagnostic that keeps its typed error for the audit.
pub(crate) fn failed_measurement(
    painter: &Painter,
    text: &str,
    interaction: TextInteraction,
    error: TextLayoutError,
    tokens: &DesignTokens,
    max_width: f32,
) -> MeasuredText {
    let colour: Color32 = tokens.colours.status_error.into();
    let fallback = format!("Text unavailable ({error:?}): {text}");
    let mut job = LayoutJob::simple_singleline(
        fallback,
        FontId::new(tokens.typography.body_size.0, FontFamily::Proportional),
        colour,
    );
    job.wrap = TextWrapping {
        max_width: if max_width.is_finite() {
            max_width.max(1.0)
        } else {
            200.0
        },
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    MeasuredText {
        galley: painter.layout_job(job),
        spec: TextSpec {
            interaction,
            ..TextSpec::single_line(TextRole::Error, TextOverflow::Ellipsis)
        },
        colour,
        layout_error: Some(error),
    }
}

/// [`measure_reading_job`] at a component boundary: a failed request paints
/// an explicit diagnostic and keeps a failing observation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn measure_component_reading(
    painter: &Painter,
    job: LayoutJob,
    role: TextRole,
    interaction: TextInteraction,
    tokens: &DesignTokens,
    font_scale: f32,
    max_width: f32,
) -> MeasuredText {
    let text = job.text.clone();
    measure_reading_job(
        painter,
        job,
        role,
        interaction,
        tokens,
        font_scale,
        max_width,
    )
    .unwrap_or_else(|error| {
        failed_measurement(painter, &text, interaction, error, tokens, max_width)
    })
}

/// Component boundary: invalid requests paint an explicit error and emit a
/// failing observation instead of disappearing. Low-level callers can still
/// use `measure_text` to handle the typed error themselves.
pub fn measure_component_text(
    painter: &Painter,
    text: &str,
    spec: TextSpec,
    tokens: &DesignTokens,
    font_scale: f32,
    max_width: f32,
) -> MeasuredText {
    measure_text(painter, text, spec, tokens, font_scale, max_width).unwrap_or_else(|error| {
        failed_measurement(painter, text, spec.interaction, error, tokens, max_width)
    })
}

pub fn paint_measured_text(
    painter: &Painter,
    measured: &MeasuredText,
    allocated_rect: Rect,
    component_id: TextComponentId,
    parent_id: Option<TextComponentId>,
) -> TextLayoutObservation {
    debug_assert_eq!(
        measured.spec.interaction,
        TextInteraction::Inert,
        "selectable measured text must be presented through present_measured_text"
    );
    let (galley_position, observation) =
        position_measured_text(painter, measured, allocated_rect, component_id, parent_id);
    painter.with_clip_rect(allocated_rect).galley(
        galley_position,
        measured.galley.clone(),
        measured.colour,
    );
    observation
}

/// Present inert measured content as one accessible label while preserving
/// Stobie's allocation, alignment and clipping. Use this only when the text
/// itself owns its semantics; interactive chrome should retain its aggregate
/// widget semantics and call [`paint_measured_text`] instead.
pub fn present_accessible_measured_text(
    ui: &mut Ui,
    measured: &MeasuredText,
    allocated_rect: Rect,
    component_id: TextComponentId,
    parent_id: Option<TextComponentId>,
) -> (egui::Response, TextLayoutObservation) {
    debug_assert_eq!(
        measured.spec.interaction,
        TextInteraction::Inert,
        "selectable measured text must be presented through present_measured_text"
    );
    let (galley_position, observation) = position_measured_text(
        ui.painter(),
        measured,
        allocated_rect,
        component_id,
        parent_id,
    );
    let mut label_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("stobie.accessible-measured-text", component_id, parent_id))
            .max_rect(allocated_rect),
    );
    label_ui.set_clip_rect(ui.clip_rect().intersect(allocated_rect));
    let response = label_ui.interact(
        allocated_rect,
        Id::new(("stobie.accessible-measured-text", component_id, parent_id)),
        Sense::hover(),
    );
    if response.interact_rect.is_positive() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Label,
                label_ui.is_enabled(),
                measured.galley.text(),
            )
        });
    }
    label_ui
        .painter()
        .galley(galley_position, measured.galley.clone(), measured.colour);
    (response, observation)
}

/// Present measured text while preserving Stobie's allocation and clipping.
/// Selectable text delegates cursor, drag, multi-label and copy behaviour to
/// egui's native label-selection state.
pub fn present_measured_text(
    ui: &mut Ui,
    measured: &MeasuredText,
    allocated_rect: Rect,
    component_id: TextComponentId,
    parent_id: Option<TextComponentId>,
) -> (Option<egui::Response>, TextLayoutObservation) {
    if measured.spec.interaction == TextInteraction::Inert {
        return (
            None,
            paint_measured_text(
                ui.painter(),
                measured,
                allocated_rect,
                component_id,
                parent_id,
            ),
        );
    }

    let (galley_position, observation) = position_measured_text(
        ui.painter(),
        measured,
        allocated_rect,
        component_id,
        parent_id,
    );
    let mut selection_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(("stobie.measured-text", component_id, parent_id))
            .max_rect(allocated_rect),
    );
    selection_ui.set_clip_rect(ui.clip_rect().intersect(allocated_rect));
    let mut sense = if selection_ui.input(|input| input.has_touch_screen()) {
        Sense::click()
    } else {
        Sense::click_and_drag()
    };
    sense -= Sense::FOCUSABLE;
    let response = selection_ui.interact(
        allocated_rect,
        Id::new(("stobie.measured-text", component_id, parent_id)),
        sense,
    );
    if response.interact_rect.is_positive() {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Label,
                selection_ui.is_enabled(),
                measured.galley.text(),
            )
        });
    }
    if selection_ui.is_rect_visible(response.rect) {
        LabelSelectionState::label_text_selection(
            &selection_ui,
            &response,
            galley_position,
            measured.galley.clone(),
            measured.colour,
            egui::Stroke::NONE,
        );
    }
    (Some(response), observation)
}

fn position_measured_text(
    painter: &Painter,
    measured: &MeasuredText,
    allocated_rect: Rect,
    component_id: TextComponentId,
    parent_id: Option<TextComponentId>,
) -> (Pos2, TextLayoutObservation) {
    crate::text_coverage::record_measured_component(
        painter.ctx(),
        component_id,
        measured.layout_error.is_some(),
    );
    let size = measured.galley.size();
    let anchor_x = match measured.spec.horizontal_alignment {
        HorizontalTextAlignment::Start => allocated_rect.left(),
        HorizontalTextAlignment::Centre => allocated_rect.center().x,
        HorizontalTextAlignment::End => allocated_rect.right(),
    };
    let left = match measured.spec.horizontal_alignment {
        HorizontalTextAlignment::Start => anchor_x,
        HorizontalTextAlignment::Centre => anchor_x - size.x * 0.5,
        HorizontalTextAlignment::End => anchor_x - size.x,
    };
    let top = match measured.spec.vertical_alignment {
        VerticalTextAlignment::Top => allocated_rect.top(),
        VerticalTextAlignment::Centre => allocated_rect.center().y - size.y * 0.5,
        VerticalTextAlignment::Bottom => allocated_rect.bottom() - size.y,
    };
    let positioned = Rect::from_min_size(Pos2::new(left, top), size);
    let galley_position = positioned.min;
    let painted = if measured.galley.job.text.is_empty() {
        Rect::from_min_size(positioned.min, egui::Vec2::ZERO)
    } else {
        // Galley layout bounds remain available in headless egui even when a
        // test font atlas has no tessellated mesh bounds.
        positioned
    };
    let clip_rect = painter.clip_rect().intersect(allocated_rect);
    // Only text in view presents content: text scrolled out of view or
    // clipped away publishes none, whatever the host's visibility policy.
    let in_view = clip_rect.is_positive();
    let observation = TextLayoutObservation {
        component_id,
        parent_id,
        role: measured.spec.role,
        horizontal_alignment: measured.spec.horizontal_alignment,
        vertical_alignment: measured.spec.vertical_alignment,
        allocated_rect: allocated_rect.into(),
        painted_rect: painted.into(),
        clip_rect: clip_rect.into(),
        baseline: None,
        overflow: measured.spec.overflow,
        interaction: measured.spec.interaction,
        declared_max_lines: measured.spec.max_lines,
        line_count: u32::try_from(measured.galley.rows.len()).unwrap_or(u32::MAX),
        truncated: measured.galley.elided,
        layout_error: measured.layout_error,
        content: (in_view && component_id.kind.presents_content()).then(|| {
            if crate::observation::is_content_withheld(painter.ctx()) {
                TextContent::Withheld
            } else {
                TextContent::excerpt(measured.galley.text())
            }
        }),
    };
    (galley_position, observation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DensityVariant, ThemeVariant};

    fn run_layout(
        text: &'static str,
        spec: TextSpec,
        width: f32,
        scale: f32,
    ) -> TextLayoutObservation {
        let context = egui::Context::default();
        crate::install_typography_fonts(&context);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))),
            ..Default::default()
        };
        let mut observation = None;
        let mut output = context.run_ui(input, |ui| {
            let measured = measure_text(
                ui.painter(),
                text,
                spec,
                &DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable),
                scale,
                width,
            )
            .unwrap();
            let height = measured.size().y.max(20.0);
            let allocation = Rect::from_min_size(Pos2::ZERO, egui::vec2(width, height));
            observation = Some(paint_measured_text(
                ui.painter(),
                &measured,
                allocation,
                TextComponentId::new(TextComponentKind::TextSample, 1),
                None,
            ));
        });
        output.textures_delta.clear();
        observation.unwrap()
    }

    #[test]
    fn expand_wrap_and_truncate_have_explicit_measured_behaviour() {
        let expanded = run_layout(
            "A deliberately long expanded diagnostic label",
            TextSpec::single_line(TextRole::Secondary, TextOverflow::Expand),
            500.0,
            1.0,
        );
        assert_eq!(expanded.line_count, 1);
        assert!(!expanded.truncated);

        let wrapped = run_layout(
            "A deliberately long body label that wraps over several words",
            TextSpec {
                max_lines: 3,
                overflow: TextOverflow::Wrap,
                ..TextSpec::single_line(TextRole::Body, TextOverflow::Wrap)
            },
            180.0,
            1.0,
        );
        assert!((2..=3).contains(&wrapped.line_count));

        let truncated = run_layout(
            "unbroken_label_that_cannot_fit_inside_the_declared_width",
            TextSpec::single_line(TextRole::TabLabel, TextOverflow::Ellipsis),
            80.0,
            1.0,
        );
        assert_eq!(truncated.line_count, 1);
        assert!(truncated.truncated);
        assert!(audit_text_layouts(&[expanded, wrapped, truncated]).is_empty());
    }

    #[test]
    fn empty_numeric_and_scaled_text_remain_finite_and_auditable() {
        let empty = run_layout(
            "",
            TextSpec::single_line(TextRole::ButtonLabel, TextOverflow::Ellipsis),
            80.0,
            1.0,
        );
        let numeric = run_layout(
            "-123,456.789 µm",
            TextSpec {
                horizontal_alignment: HorizontalTextAlignment::End,
                ..TextSpec::single_line(TextRole::TabularValue, TextOverflow::Ellipsis)
            },
            180.0,
            1.25,
        );
        let scaled = run_layout(
            "Scaled text",
            TextSpec::single_line(TextRole::Body, TextOverflow::Expand),
            180.0,
            1.5,
        );
        assert_eq!(empty.line_count, 1);
        assert!(!numeric.truncated);
        assert!(scaled.painted_rect.max_y - scaled.painted_rect.min_y > 13.0);
        assert!(audit_text_layouts(&[empty, numeric, scaled]).is_empty());
    }

    #[test]
    fn clip_and_scroll_preserve_intrinsic_layout_under_an_explicit_policy() {
        let clip = run_layout(
            "A clipped technical value that remains intrinsically measured",
            TextSpec::single_line(TextRole::MonospaceTechnical, TextOverflow::Clip),
            70.0,
            1.0,
        );
        let scroll = run_layout(
            "A scroll-owned line that remains intrinsically measured",
            TextSpec::single_line(TextRole::Body, TextOverflow::Scroll),
            70.0,
            1.0,
        );
        assert!(!clip.truncated && !scroll.truncated);
        assert!(clip.painted_rect.max_x > clip.allocated_rect.max_x);
        assert!(scroll.painted_rect.max_x > scroll.allocated_rect.max_x);
        assert!(audit_text_layouts(&[clip, scroll]).is_empty());
    }

    #[test]
    fn audit_rejects_invalid_geometry_lines_overflow_and_sibling_overlap() {
        let parent = TextComponentId::new(TextComponentKind::DockTabStrip, 9);
        let base = TextLayoutObservation {
            component_id: TextComponentId::new(TextComponentKind::DockTab, 1),
            parent_id: Some(parent),
            role: TextRole::TabLabel,
            horizontal_alignment: HorizontalTextAlignment::Centre,
            vertical_alignment: VerticalTextAlignment::Centre,
            allocated_rect: TextRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 50.0,
                max_y: 20.0,
            },
            painted_rect: TextRect {
                min_x: 4.0,
                min_y: 3.0,
                max_x: 40.0,
                max_y: 17.0,
            },
            clip_rect: TextRect {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 50.0,
                max_y: 20.0,
            },
            baseline: None,
            overflow: TextOverflow::Expand,
            interaction: TextInteraction::Inert,
            declared_max_lines: 1,
            line_count: 2,
            truncated: true,
            layout_error: None,
            content: None,
        };
        let mut sibling = base.clone();
        sibling.component_id = TextComponentId::new(TextComponentKind::DockTab, 2);
        sibling.allocated_rect.min_x = 30.0;
        sibling.allocated_rect.max_x = 80.0;
        sibling.clip_rect = sibling.allocated_rect;
        sibling.painted_rect.min_x = 35.0;
        sibling.painted_rect.max_x = 70.0;
        let findings = audit_text_layouts(&[base, sibling]);
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, TextAuditFinding::UnexpectedLineCount { .. }))
        );
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, TextAuditFinding::UndeclaredTruncation { .. }))
        );
        assert!(
            findings
                .iter()
                .any(|finding| matches!(finding, TextAuditFinding::OverlappingSiblingText { .. }))
        );
    }

    #[test]
    fn selectable_measured_text_delegates_drag_selection_and_copy_to_egui() {
        use std::{cell::Cell, rc::Rc};

        use egui_kittest::{
            Harness,
            kittest::{NodeT, Queryable},
        };

        const VALUE: &str = "generation=4 epoch=18 sequence=42";
        let allocated = Rc::new(Cell::new(Rect::NOTHING));
        let observed_rect = Rc::clone(&allocated);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(420.0, 120.0))
            .build_ui(move |ui| {
                let tokens = DesignTokens::resolve(ThemeVariant::Dark, DensityVariant::Comfortable);
                let measured = measure_text(
                    ui.painter(),
                    VALUE,
                    TextSpec {
                        interaction: TextInteraction::Selectable,
                        ..TextSpec::single_line(
                            TextRole::MonospaceTechnical,
                            TextOverflow::Ellipsis,
                        )
                    },
                    &tokens,
                    1.0,
                    360.0,
                )
                .unwrap();
                let (rect, _) = ui.allocate_exact_size(egui::vec2(360.0, 32.0), Sense::hover());
                observed_rect.set(rect);
                let (_, observation) = present_measured_text(
                    ui,
                    &measured,
                    rect,
                    TextComponentId::new(TextComponentKind::TextSample, 1),
                    None,
                );
                assert_eq!(observation.interaction, TextInteraction::Selectable);
                assert!(audit_text_layouts(&[observation]).is_empty());
            });

        harness.run();
        let rect = allocated.get();
        let start = egui::pos2(rect.left() + 1.0, rect.center().y);
        let end = egui::pos2(rect.right() - 1.0, rect.center().y);
        harness.hover_at(start);
        harness.drag_at(start);
        harness.hover_at(end);
        harness.drop_at(end);
        harness.step();

        let label = harness.get_by_role_and_label(egui::accesskit::Role::Label, VALUE);
        assert!(label.accesskit_node().text_selection().is_some());

        harness.event(egui::Event::Copy);
        harness.step();
        assert!(harness.output().platform_output.commands.iter().any(
            |command| matches!(command, egui::OutputCommand::CopyText(text) if text == VALUE)
        ));
    }
}
