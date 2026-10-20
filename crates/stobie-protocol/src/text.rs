//! Measured text observations and the structural text audit, as inspection
//! publishes them with each snapshot.

use serde::{Deserialize, Serialize};

/// The largest line limit a bounded text recipe accepts. Reading text, under
/// [`TextOverflow::WrapUnbounded`], has no limit.
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
    /// Text wraps at the declared width to as many lines as it needs, and the
    /// allocation grows to the measured height; it is never truncated. Only the
    /// reading-text and link recipes use it, and they declare a line limit of 0
    /// (none). A parent scroll surface may clip the allocation.
    WrapUnbounded,
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

#[cfg(feature = "emath")]
impl From<emath::Rect> for TextRect {
    fn from(rect: emath::Rect) -> Self {
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
    NavigationItem,
    NavigationLabel,
    NavigationBadge,
    PropertyRow,
    ResultRow,
    StatusBadge,
    StatusChip,
    ThumbnailCell,
    DiagnosticRow,
    SectionHeading,
    ContentLabel,
    ReadingText,
    Link,
    /// A numbered list item's number, beside its reading text.
    ListMarker,
    ApplicationBarLabel,
    ImageStatus,
    GalleryHeading,
    TextSample,
    /// A list row that activates its action; the parent of its title and
    /// detail.
    ListRow,
    ListRowTitle,
    ListRowDetail,
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

impl TextComponentKind {
    /// Whether text of this kind is page content, whose words observation
    /// publishes as a [`TextContent`]: content labels, section headings,
    /// reading text and Markdown blocks, links and Markdown list numbers.
    /// Other kinds are the text of controls and chrome, such as a button's
    /// label, which the control's semantic node already names.
    pub const fn presents_content(self) -> bool {
        matches!(
            self,
            Self::ContentLabel
                | Self::SectionHeading
                | Self::ReadingText
                | Self::Link
                | Self::ListMarker
        )
    }
}

/// The most characters of one text's content that observation publishes.
/// Longer text is cut to this many characters and marked with
/// [`CONTENT_TRUNCATION_MARKER`].
pub const CONTENT_EXCERPT_CHARS: usize = 256;

/// Follows an excerpt cut at [`CONTENT_EXCERPT_CHARS`].
pub const CONTENT_TRUNCATION_MARKER: char = '…';

/// The presented words of one content text, as observation publishes them.
/// Only kinds that [`TextComponentKind::presents_content`] carry it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextContent {
    /// The text as presented, or, when it is longer than
    /// [`CONTENT_EXCERPT_CHARS`] characters, its first
    /// [`CONTENT_EXCERPT_CHARS`] characters followed by
    /// [`CONTENT_TRUNCATION_MARKER`], with `truncated` set.
    Excerpt { text: String, truncated: bool },
    /// The application withheld the text from observation, as it does for
    /// sensitive text; nothing about it, not even its length, is published.
    Withheld,
}

impl TextContent {
    /// A bounded excerpt of `text`.
    pub fn excerpt(text: &str) -> Self {
        match text.char_indices().nth(CONTENT_EXCERPT_CHARS) {
            Some((end, _)) => {
                let mut excerpt = String::with_capacity(end + CONTENT_TRUNCATION_MARKER.len_utf8());
                excerpt.push_str(&text[..end]);
                excerpt.push(CONTENT_TRUNCATION_MARKER);
                Self::Excerpt {
                    text: excerpt,
                    truncated: true,
                }
            }
            None => Self::Excerpt {
                text: text.to_owned(),
                truncated: false,
            },
        }
    }

    /// The published excerpt, or `None` when the text was withheld.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Excerpt { text, .. } => Some(text),
            Self::Withheld => None,
        }
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
    /// The declared line limit; 0 under [`TextOverflow::WrapUnbounded`], which
    /// has none.
    pub declared_max_lines: u8,
    /// Laid-out lines, counted in full however many there are.
    pub line_count: u32,
    pub truncated: bool,
    /// Failed requests retain their diagnostic even when the fallback is clipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_error: Option<TextLayoutError>,
    /// The presented words, bounded, for kinds that
    /// [`TextComponentKind::presents_content`]; `None` for other kinds. A
    /// failed layout publishes its visible diagnostic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<TextContent>,
}

impl TextLayoutObservation {
    /// The published excerpt of this text's content, if it has one.
    pub fn content_text(&self) -> Option<&str> {
        self.content.as_ref().and_then(TextContent::text)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextLayoutError {
    InvalidMaxLines(u8),
    /// A bounded text recipe was asked to wrap without a line limit; only the
    /// reading-text and link recipes lay out unbounded text.
    UnboundedOverflow,
    /// A link's URL is blank or not an http, https or mailto address.
    InvalidUrl,
    InvalidWidth,
    InvalidFontScale,
    FontsNotInstalled,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextAuditFinding {
    LayoutFailed {
        component_id: TextComponentId,
        error: TextLayoutError,
    },
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

/// Audit only the supplied Stobie-owned layout observations. Empty findings
/// do not certify unobserved native controls or ordinary labels; retain
/// [`crate::TextAuditCoverage`] with the findings to state the denominator.
pub fn audit_text_layouts(observations: &[TextLayoutObservation]) -> Vec<TextAuditFinding> {
    let mut findings = Vec::new();
    for observation in observations {
        if let Some(error) = observation.layout_error {
            findings.push(TextAuditFinding::LayoutFailed {
                component_id: observation.component_id,
                error,
            });
        }
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
        // Unbounded text grows its own allocation, but a parent scroll surface
        // may clip that allocation.
        let unbounded = observation.overflow == TextOverflow::WrapUnbounded;
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
            && !unbounded
            && !observation
                .clip_rect
                .contains_with_tolerance(observation.painted_rect, TEXT_AUDIT_TOLERANCE)
        {
            findings.push(TextAuditFinding::TextOutsideClip {
                component_id: observation.component_id,
            });
        }
        let over_limit = if unbounded {
            observation.declared_max_lines != 0
        } else {
            observation.line_count > u32::from(observation.declared_max_lines)
        };
        if over_limit
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

/// Categories outside the structural text audit, not audit failures.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextExclusion {
    /// Ordinary labels, headings and hover text are not enumerated.
    OrdinaryEguiLabels,
    NativeComboBoxText,
    NativeButtonText,
    NativeRadioButtonText,
    NativeSliderText,
    NativeSelectableText,
    NativeTextEditText,
}

/// The denominator for one viewport's current UI pass, not a visible-string census.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextAuditCoverage {
    /// Distinct component IDs represented by measured text observations.
    pub measured_components: usize,
    /// Distinct submitted components, including failed visible fallbacks.
    pub attempted_components: usize,
    pub successful_components: usize,
    pub failed_components: usize,
    /// Explicitly recorded native control responses, including submitted clipped
    /// controls and open popup options. Closed popups and virtual items that were
    /// not instantiated do not contribute. A control is not a count of its labels.
    pub native_text_controls: usize,
    /// Native controls whose internal text has structural layout observations.
    /// Currently zero: AccessKit and keyboard evidence do not measure text layout.
    pub observed_native_controls: usize,
    /// Ordinary labels are always outside scope; native categories are included
    /// only when those controls were recorded in this pass.
    pub excluded_categories: Vec<TextExclusion>,
}

impl Default for TextAuditCoverage {
    fn default() -> Self {
        Self {
            measured_components: 0,
            attempted_components: 0,
            successful_components: 0,
            failed_components: 0,
            native_text_controls: 0,
            observed_native_controls: 0,
            excluded_categories: vec![TextExclusion::OrdinaryEguiLabels],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_is_published_whole() {
        let text = "a".repeat(CONTENT_EXCERPT_CHARS);
        assert_eq!(
            TextContent::excerpt(&text),
            TextContent::Excerpt {
                text: text.clone(),
                truncated: false,
            }
        );
        assert_eq!(TextContent::excerpt(&text).text(), Some(text.as_str()));
        assert_eq!(
            TextContent::excerpt(""),
            TextContent::Excerpt {
                text: String::new(),
                truncated: false,
            }
        );
    }

    #[test]
    fn long_content_is_cut_at_the_bound_on_a_character_boundary_and_marked() {
        // Multi-byte characters: the bound counts characters, not bytes.
        let text = "é".repeat(CONTENT_EXCERPT_CHARS + 1);
        let TextContent::Excerpt {
            text: excerpt,
            truncated,
        } = TextContent::excerpt(&text)
        else {
            panic!("an excerpt");
        };
        assert!(truncated);
        assert_eq!(excerpt.chars().count(), CONTENT_EXCERPT_CHARS + 1);
        assert_eq!(excerpt.chars().last(), Some(CONTENT_TRUNCATION_MARKER));
        assert!(text.starts_with(excerpt.trim_end_matches(CONTENT_TRUNCATION_MARKER)));
        let long = TextContent::excerpt(&"x".repeat(100_000));
        assert!(long.text().unwrap().len() <= CONTENT_EXCERPT_CHARS + 3);
    }

    #[test]
    fn only_content_kinds_present_content() {
        use TextComponentKind as Kind;
        for kind in [
            Kind::ContentLabel,
            Kind::SectionHeading,
            Kind::ReadingText,
            Kind::Link,
            Kind::ListMarker,
        ] {
            assert!(kind.presents_content(), "{kind:?}");
        }
        for kind in [
            Kind::ActionButton,
            Kind::DockTab,
            Kind::NavigationItem,
            Kind::ListRowTitle,
            Kind::StatusChip,
            Kind::GalleryHeading,
        ] {
            assert!(!kind.presents_content(), "{kind:?}");
        }
    }

    #[test]
    fn content_serialises_compactly_and_older_observations_still_read() {
        assert_eq!(
            serde_json::to_value(TextContent::excerpt("Hello")).unwrap(),
            serde_json::json!({"kind": "excerpt", "text": "Hello", "truncated": false})
        );
        assert_eq!(
            serde_json::to_value(TextContent::Withheld).unwrap(),
            serde_json::json!({"kind": "withheld"})
        );
        // An observation from before content text was published has none.
        let rect = serde_json::json!({"min_x": 0, "min_y": 0, "max_x": 10, "max_y": 10});
        let older: TextLayoutObservation = serde_json::from_value(serde_json::json!({
            "component_id": {"kind": "reading_text", "instance": 1},
            "parent_id": null,
            "role": "body",
            "horizontal_alignment": "start",
            "vertical_alignment": "top",
            "allocated_rect": rect,
            "painted_rect": rect,
            "clip_rect": rect,
            "baseline": null,
            "overflow": "wrap_unbounded",
            "declared_max_lines": 0,
            "line_count": 1,
            "truncated": false
        }))
        .unwrap();
        assert_eq!(older.content, None);
        assert_eq!(older.content_text(), None);
        let value = serde_json::to_value(&older).unwrap();
        assert!(value.get("content").is_none());
    }
}
