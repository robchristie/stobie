use std::sync::Arc;

use egui::{
    Align, Color32, FontFamily, FontId, Galley, Id, Painter, Pos2, Rect, Sense, TextFormat, Ui,
    UiBuilder,
    text::{LayoutJob, TextWrapping},
    text_selection::LabelSelectionState,
};
use serde::{Deserialize, Serialize};

use crate::{DesignTokens, FontWeight};

pub const MAX_TEXT_LINES: u8 = 8;
pub const TEXT_AUDIT_TOLERANCE: f32 = 1.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextRole {
    ApplicationTitle,
    PaneTitle,
    SectionHeading,
    Body,
    Secondary,
    Caption,
    TabularValue,
    MonospaceTechnical,
    ButtonLabel,
    TabLabel,
    Status,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextOverflow {
    /// Text is elided at the declared width and line limit.
    Ellipsis,
    /// Text may wrap to the declared line limit and is then elided.
    Wrap,
    /// Text keeps its intrinsic layout and paint is clipped to its allocation.
    Clip,
    /// Text keeps its intrinsic layout inside a component-owned scroll surface.
    Scroll,
    /// The allocation must grow to the measured single-line text.
    Expand,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextInteraction {
    #[default]
    Inert,
    Selectable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HorizontalTextAlignment {
    Start,
    Centre,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalTextAlignment {
    Top,
    Centre,
    Bottom,
}

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

    pub fn validate(self) -> Result<Self, TextLayoutError> {
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
    pub colour: Color32,
}

impl TextRole {
    pub fn style(self, tokens: &DesignTokens, font_scale: f32) -> TextRoleStyle {
        let scale = if font_scale.is_finite() {
            font_scale.clamp(1.0, 1.5)
        } else {
            1.0
        };
        let (size, weight, colour) = match self {
            Self::ApplicationTitle
            | Self::PaneTitle
            | Self::SectionHeading
            | Self::ButtonLabel
            | Self::TabLabel => (
                tokens.typography.label_size.0,
                tokens.typography.label_weight,
                tokens.colours.text_primary,
            ),
            Self::Caption => (
                tokens.typography.label_size.0,
                tokens.typography.body_weight,
                tokens.colours.text_muted,
            ),
            Self::Secondary | Self::Status => (
                tokens.typography.body_size.0,
                tokens.typography.body_weight,
                tokens.colours.text_muted,
            ),
            Self::Error => (
                tokens.typography.body_size.0,
                tokens.typography.body_weight,
                tokens.colours.status_error,
            ),
            Self::Body | Self::TabularValue | Self::MonospaceTechnical => (
                tokens.typography.body_size.0,
                tokens.typography.body_weight,
                tokens.colours.text_primary,
            ),
        };
        let family = if self == Self::MonospaceTechnical {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        };
        TextRoleStyle {
            font_id: FontId::new(size * scale, family),
            weight,
            colour: colour.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl TextRect {
    fn is_finite(self) -> bool {
        [self.min_x, self.min_y, self.max_x, self.max_y]
            .into_iter()
            .all(f32::is_finite)
    }

    fn is_positive(self) -> bool {
        self.max_x > self.min_x && self.max_y > self.min_y
    }

    fn is_non_negative(self) -> bool {
        self.max_x >= self.min_x && self.max_y >= self.min_y
    }

    fn contains_with_tolerance(self, child: Self, tolerance: f32) -> bool {
        child.min_x >= self.min_x - tolerance
            && child.min_y >= self.min_y - tolerance
            && child.max_x <= self.max_x + tolerance
            && child.max_y <= self.max_y + tolerance
    }

    fn overlaps_beyond_tolerance(self, other: Self, tolerance: f32) -> bool {
        (self.max_x.min(other.max_x) - self.min_x.max(other.min_x)) > tolerance
            && (self.max_y.min(other.max_y) - self.min_y.max(other.min_y)) > tolerance
    }
}

impl From<Rect> for TextRect {
    fn from(rect: Rect) -> Self {
        Self {
            min_x: rect.min.x,
            min_y: rect.min.y,
            max_x: rect.max.x,
            max_y: rect.max.y,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextComponentKind {
    DockTabStrip,
    DockTab,
    ActionButton,
    PropertyRow,
    ResultRow,
    StatusBadge,
    ThumbnailCell,
    DiagnosticRow,
    SectionHeading,
    ContentLabel,
    ApplicationBarLabel,
    ImageStatus,
    GalleryHeading,
    TextSample,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct TextComponentId {
    pub kind: TextComponentKind,
    pub instance: u64,
}

impl TextComponentId {
    pub const fn new(kind: TextComponentKind, instance: u64) -> Self {
        Self { kind, instance }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextLayoutObservation {
    pub component_id: TextComponentId,
    pub parent_id: Option<TextComponentId>,
    pub role: TextRole,
    pub horizontal_alignment: HorizontalTextAlignment,
    pub vertical_alignment: VerticalTextAlignment,
    pub allocated_rect: TextRect,
    pub painted_rect: TextRect,
    pub clip_rect: TextRect,
    /// Egui 0.36 does not expose a reliable public baseline metric.
    pub baseline: Option<f32>,
    pub overflow: TextOverflow,
    #[serde(default)]
    pub interaction: TextInteraction,
    pub declared_max_lines: u8,
    pub line_count: u8,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextLayoutError {
    InvalidMaxLines(u8),
    InvalidWidth,
    InvalidFontScale,
}

#[derive(Clone)]
pub struct MeasuredText {
    pub galley: Arc<Galley>,
    pub spec: TextSpec,
    pub colour: Color32,
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
    let font_size = role_style.font_id.size;
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
    };
    job.break_on_newline = spec.overflow == TextOverflow::Wrap;
    if spec.overflow == TextOverflow::Wrap {
        job.sections[0].format = TextFormat {
            line_height: Some(font_size * tokens.typography.line_height.0),
            ..job.sections[0].format.clone()
        };
    }
    let galley = painter.layout_job(job);
    Ok(MeasuredText {
        galley,
        spec,
        colour: role_style.colour,
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
/// Polyorama's allocation, alignment and clipping. Use this only when the text
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
            .id_salt((
                "polyorama.accessible-measured-text",
                component_id,
                parent_id,
            ))
            .max_rect(allocated_rect),
    );
    label_ui.set_clip_rect(ui.clip_rect().intersect(allocated_rect));
    let response = label_ui.interact(
        allocated_rect,
        Id::new((
            "polyorama.accessible-measured-text",
            component_id,
            parent_id,
        )),
        Sense::hover(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            label_ui.is_enabled(),
            measured.galley.text(),
        )
    });
    label_ui
        .painter()
        .galley(galley_position, measured.galley.clone(), measured.colour);
    (response, observation)
}

/// Present measured text while preserving Polyorama's allocation and clipping.
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
            .id_salt(("polyorama.measured-text", component_id, parent_id))
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
        Id::new(("polyorama.measured-text", component_id, parent_id)),
        sense,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Label,
            selection_ui.is_enabled(),
            measured.galley.text(),
        )
    });
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
    let observation = TextLayoutObservation {
        component_id,
        parent_id,
        role: measured.spec.role,
        horizontal_alignment: measured.spec.horizontal_alignment,
        vertical_alignment: measured.spec.vertical_alignment,
        allocated_rect: allocated_rect.into(),
        painted_rect: painted.into(),
        clip_rect: painter.clip_rect().intersect(allocated_rect).into(),
        baseline: None,
        overflow: measured.spec.overflow,
        interaction: measured.spec.interaction,
        declared_max_lines: measured.spec.max_lines,
        line_count: measured.galley.rows.len().min(usize::from(u8::MAX)) as u8,
        truncated: measured.galley.elided,
    };
    (galley_position, observation)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextAuditFinding {
    InvalidUsefulBounds {
        component_id: TextComponentId,
    },
    TextOutsideAllocation {
        component_id: TextComponentId,
    },
    TextOutsideClip {
        component_id: TextComponentId,
    },
    UnexpectedLineCount {
        component_id: TextComponentId,
    },
    UndeclaredTruncation {
        component_id: TextComponentId,
    },
    AlignmentDeviation {
        component_id: TextComponentId,
    },
    OverlappingSiblingText {
        first: TextComponentId,
        second: TextComponentId,
    },
}

/// Audit only the supplied Polyorama-owned layout observations. Empty findings
/// do not certify unobserved native controls or ordinary labels; retain
/// [`crate::TextAuditCoverage`] with the findings to state the denominator.
pub fn audit_text_layouts(observations: &[TextLayoutObservation]) -> Vec<TextAuditFinding> {
    let mut findings = Vec::new();
    for observation in observations {
        if !observation.allocated_rect.is_finite()
            || !observation.allocated_rect.is_positive()
            || !observation.clip_rect.is_finite()
            || !observation.clip_rect.is_positive()
            || !observation.painted_rect.is_finite()
            || !observation.painted_rect.is_non_negative()
        {
            findings.push(TextAuditFinding::InvalidUsefulBounds {
                component_id: observation.component_id,
            });
            continue;
        }
        let outside_is_declared = matches!(
            observation.overflow,
            TextOverflow::Clip | TextOverflow::Scroll
        );
        if !outside_is_declared
            && !observation
                .allocated_rect
                .contains_with_tolerance(observation.painted_rect, TEXT_AUDIT_TOLERANCE)
        {
            findings.push(TextAuditFinding::TextOutsideAllocation {
                component_id: observation.component_id,
            });
        }
        if !outside_is_declared
            && !observation
                .clip_rect
                .contains_with_tolerance(observation.painted_rect, TEXT_AUDIT_TOLERANCE)
        {
            findings.push(TextAuditFinding::TextOutsideClip {
                component_id: observation.component_id,
            });
        }
        if observation.line_count > observation.declared_max_lines
            || (observation.line_count == 0
                && observation.painted_rect.max_x > observation.painted_rect.min_x)
        {
            findings.push(TextAuditFinding::UnexpectedLineCount {
                component_id: observation.component_id,
            });
        }
        if observation.truncated
            && !matches!(
                observation.overflow,
                TextOverflow::Ellipsis | TextOverflow::Wrap
            )
        {
            findings.push(TextAuditFinding::UndeclaredTruncation {
                component_id: observation.component_id,
            });
        }
        let horizontal_delta = match observation.horizontal_alignment {
            HorizontalTextAlignment::Start => {
                observation.painted_rect.min_x - observation.allocated_rect.min_x
            }
            HorizontalTextAlignment::Centre => {
                (observation.painted_rect.min_x + observation.painted_rect.max_x
                    - observation.allocated_rect.min_x
                    - observation.allocated_rect.max_x)
                    * 0.5
            }
            HorizontalTextAlignment::End => {
                observation.painted_rect.max_x - observation.allocated_rect.max_x
            }
        };
        let vertical_delta = match observation.vertical_alignment {
            VerticalTextAlignment::Top => {
                observation.painted_rect.min_y - observation.allocated_rect.min_y
            }
            VerticalTextAlignment::Centre => {
                (observation.painted_rect.min_y + observation.painted_rect.max_y
                    - observation.allocated_rect.min_y
                    - observation.allocated_rect.max_y)
                    * 0.5
            }
            VerticalTextAlignment::Bottom => {
                observation.painted_rect.max_y - observation.allocated_rect.max_y
            }
        };
        let has_painted_extent = observation.painted_rect.max_x > observation.painted_rect.min_x
            || observation.painted_rect.max_y > observation.painted_rect.min_y;
        if has_painted_extent
            && (horizontal_delta.abs() > TEXT_AUDIT_TOLERANCE
                || vertical_delta.abs() > TEXT_AUDIT_TOLERANCE)
        {
            findings.push(TextAuditFinding::AlignmentDeviation {
                component_id: observation.component_id,
            });
        }
    }
    for (index, first) in observations.iter().enumerate() {
        for second in &observations[index + 1..] {
            if first.parent_id.is_some()
                && first.parent_id == second.parent_id
                && first
                    .painted_rect
                    .overlaps_beyond_tolerance(second.painted_rect, TEXT_AUDIT_TOLERANCE)
            {
                findings.push(TextAuditFinding::OverlappingSiblingText {
                    first: first.component_id,
                    second: second.component_id,
                });
            }
        }
    }
    findings
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
