//! Markdown on the reading-text path: CommonMark blocks and inline styles,
//! each block a measured reading surface, with links inside the text.

mod parse;

use std::ops::Range;

use egui::{
    Color32, CursorIcon, FontFamily, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke,
    TextFormat, text::LayoutJob,
};

use self::parse::{Block, Document, Inline};
use crate::{
    DesignTokens, HorizontalTextAlignment, INVALID_LINK_URL, MeasuredText, PresentationId,
    SemanticUiId, TextComponentId, TextComponentKind, TextInteraction, TextLayoutObservation,
    TextOverflow, TextRole, TextRoleStyles, TextSpec, UiNode, UiRole, VerticalTextAlignment,
    link_colour, link_url_allowed, measure_component_reading, measure_component_text,
    present_accessible_measured_text, present_measured_text,
};

/// A bullet's radius as a share of its text's font size.
const BULLET_RADIUS: f32 = 0.18;
/// The widest ordered-list number every list reserves, so a list growing
/// from nine items to ten keeps its alignment while a reply streams in.
const RESERVED_NUMBER: &str = "00.";
/// The marker column of a bulleted list.
const RESERVED_BULLET: &str = "0.";

/// How a Markdown presentation behaves. Its typography comes from the tokens
/// (the reading profile for a reading surface) and its colours from the
/// theme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownSpec<'a> {
    /// Selectable by default, as reading text is. Choose Inert only where a
    /// parent owns the pointer; links stay activatable either way.
    pub interaction: TextInteraction,
    /// Context for assistive technology on every link, such as "Opens in a
    /// new tab", in the application's language.
    pub link_description: Option<&'a str>,
}

impl Default for MarkdownSpec<'_> {
    fn default() -> Self {
        Self {
            interaction: TextInteraction::Selectable,
            link_description: None,
        }
    }
}

/// What [`markdown`] presented.
pub struct MarkdownResponse {
    /// The whole presentation's allocation, sensing hover only.
    pub response: Response,
    /// One [`UiRole::Link`] node for each link at least partly visible, in
    /// document order. [`crate::PresentationContext::markdown`] records them;
    /// low-level callers publish them.
    pub link_nodes: Vec<UiNode>,
}

/// The heading role for a heading level.
fn heading_role(level: u8) -> TextRole {
    match level {
        1 => TextRole::ApplicationTitle,
        2 => TextRole::PaneTitle,
        _ => TextRole::SectionHeading,
    }
}

struct TextPiece {
    identity: PresentationId,
    rect: Rect,
    job: LayoutJob,
    /// The document link each section of `job` belongs to.
    section_links: Vec<Option<usize>>,
    /// Each link in the text, by byte range.
    links: Vec<(usize, Range<usize>)>,
    measured: MeasuredText,
    heading: Option<u8>,
}

/// A block's styled text before measurement.
struct Styled {
    job: LayoutJob,
    /// The document link each section of `job` belongs to.
    section_links: Vec<Option<usize>>,
    /// Each link in the text, by byte range.
    links: Vec<(usize, Range<usize>)>,
    heading: Option<u8>,
}

enum Piece {
    Text(TextPiece),
    Marker {
        identity: PresentationId,
        rect: Rect,
        measured: MeasuredText,
    },
    Bullet {
        centre: Pos2,
        radius: f32,
        depth: usize,
        colour: Color32,
    },
    /// A code block's surface, painted before its text.
    Fill(Rect),
    /// A block quote's bar, or a horizontal rule.
    Line(Rect),
}

impl Piece {
    fn translate(&mut self, offset: egui::Vec2) {
        match self {
            Self::Text(text) => text.rect = text.rect.translate(offset),
            Self::Marker { rect, .. } | Self::Fill(rect) | Self::Line(rect) => {
                *rect = rect.translate(offset);
            }
            Self::Bullet { centre, .. } => *centre += offset,
        }
    }
}

struct Layout<'a> {
    painter: &'a Painter,
    tokens: &'a DesignTokens,
    scale: f32,
    interaction: TextInteraction,
    enabled: bool,
    links: &'a [String],
    identity: PresentationId,
    pieces: Vec<Piece>,
    y: f32,
    /// How many lists enclose the current block.
    list_depth: usize,
    /// The top of the first text laid out since the marker of the current
    /// list item was reserved.
    first_text_top: Option<f32>,
    /// The height of the strip above each code block's text that holds its
    /// copy control, when the caller presents one.
    code_header: Option<f32>,
    /// Each code block laid out, in document order.
    code_blocks: Vec<CodeBlock>,
}

/// A code block as laid out: its text exactly as parsed and the strip above
/// that text where its copy control goes.
pub(crate) struct CodeBlock {
    pub(crate) text: String,
    pub(crate) header: Rect,
}

impl Layout<'_> {
    fn style(&self, role: TextRole) -> crate::TextRoleStyle {
        role.style(self.tokens, self.scale)
    }

    fn measure(&self, text: &str, role: TextRole, width: f32) -> MeasuredText {
        measure_component_text(
            self.painter,
            text,
            TextSpec {
                horizontal_alignment: HorizontalTextAlignment::End,
                vertical_alignment: VerticalTextAlignment::Top,
                ..TextSpec::single_line(role, TextOverflow::Expand)
            },
            self.tokens,
            self.scale,
            width.max(1.0),
        )
    }

    /// The narrowest content a nested list or quote may leave; below it,
    /// nesting stops indenting.
    fn minimum_content_width(&self) -> f32 {
        self.tokens.geometry.minimum_hit_size.0 * 2.0 * self.scale
    }

    fn inline_job(
        &self,
        inline: &Inline,
        role: TextRole,
        quoted: bool,
        hovered: Option<usize>,
    ) -> (LayoutJob, Vec<Option<usize>>) {
        let style = self.style(role);
        let base = if quoted {
            self.tokens.colours.text_muted.into()
        } else {
            style.colour
        };
        let mut job = LayoutJob {
            text: inline.text.clone(),
            ..Default::default()
        };
        let mut section_links = Vec::new();
        for run in &inline.runs {
            let family = if run.style.code {
                FontFamily::Monospace
            } else if run.style.strong {
                FontFamily::Name(crate::SEMIBOLD_FONT_FAMILY.into())
            } else {
                style.font_id.family.clone()
            };
            let valid = run
                .link
                .is_some_and(|link| link_url_allowed(&self.links[link]));
            let colour = match run.link {
                Some(_) if valid && self.enabled => link_colour(self.tokens),
                Some(_) if valid => self.tokens.colours.text_muted.into(),
                _ => base,
            };
            let mut format = TextFormat {
                line_height: Some(style.line_height),
                italics: run.style.emphasis,
                ..TextFormat::simple(FontId::new(style.font_id.size, family), colour)
            };
            if run.style.code {
                format.background = self.tokens.colours.surface_raised.into();
            }
            if run.style.strikethrough {
                format.strikethrough = Stroke::new(1.0, colour);
            }
            if valid {
                let width = if run.link == hovered { 2.0 } else { 1.0 };
                format.underline = Stroke::new(width, colour);
            }
            job.sections.push(egui::text::LayoutSection {
                leading_space: 0.0,
                byte_range: egui::text::ByteIndex(run.bytes.start)
                    ..egui::text::ByteIndex(run.bytes.end),
                format,
            });
            section_links.push(run.link);
        }
        (job, section_links)
    }

    fn text(&mut self, path: &[u32], x: f32, width: f32, role: TextRole, styled: Styled) -> Rect {
        let Styled {
            job,
            section_links,
            links,
            heading,
        } = styled;
        let measured = measure_component_reading(
            self.painter,
            job.clone(),
            role,
            self.interaction,
            self.tokens,
            self.scale,
            width.max(1.0),
        );
        let rect = Rect::from_min_size(
            egui::pos2(x, self.y),
            egui::vec2(width.max(1.0), measured.size().y.max(1.0)),
        );
        self.first_text_top.get_or_insert(rect.top());
        self.pieces.push(Piece::Text(TextPiece {
            identity: self.identity.child(("block", path)),
            rect,
            job,
            section_links,
            links,
            measured,
            heading,
        }));
        self.y = rect.bottom();
        rect
    }

    /// A paragraph, or a heading of `heading`'s level.
    fn inline(
        &mut self,
        path: &[u32],
        x: f32,
        width: f32,
        inline: &Inline,
        quoted: bool,
        heading: Option<u8>,
    ) {
        let role = heading.map_or(TextRole::Body, heading_role);
        let (job, section_links) = self.inline_job(inline, role, quoted, None);
        let styled = Styled {
            job,
            section_links,
            links: inline.link_bytes(),
            heading,
        };
        self.text(path, x, width, role, styled);
    }

    /// Lay out blocks one below another. Blocks in a list item are spaced
    /// like the items themselves, whether or not the source separates them
    /// with blank lines, so a list keeps its layout as it streams in.
    fn blocks(
        &mut self,
        blocks: &[Block],
        path: &mut Vec<u32>,
        x: f32,
        width: f32,
        quoted: bool,
        in_item: bool,
    ) {
        let mut previous: Option<&Block> = None;
        for (index, block) in blocks.iter().enumerate() {
            if let Some(previous) = previous {
                self.y += if in_item || matches!(previous, Block::Heading { .. }) {
                    self.tokens.spacing.block.0
                } else {
                    self.tokens.spacing.section.0
                };
            }
            path.push(u32::try_from(index).unwrap_or(u32::MAX));
            self.block(block, path, x, width, quoted);
            path.pop();
            previous = Some(block);
        }
    }

    fn block(&mut self, block: &Block, path: &mut Vec<u32>, x: f32, width: f32, quoted: bool) {
        match block {
            Block::Paragraph(inline) => {
                self.inline(path, x, width, inline, quoted, None);
            }
            Block::Heading { level, inline } => {
                self.inline(path, x, width, inline, quoted, Some(*level));
            }
            Block::Code(code) => {
                let pad = egui::vec2(self.tokens.spacing.inline.0, self.tokens.spacing.block.0);
                let top = self.y;
                let fill = self.pieces.len();
                self.pieces.push(Piece::Fill(Rect::NOTHING));
                match self.code_header {
                    // The strip takes the place of the top padding.
                    Some(height) => {
                        self.code_blocks.push(CodeBlock {
                            text: code.clone(),
                            header: Rect::from_min_size(
                                egui::pos2(x, top),
                                egui::vec2(width, height),
                            ),
                        });
                        self.y += height;
                    }
                    None => self.y += pad.y,
                }
                let style = self.style(TextRole::MonospaceTechnical);
                let job = LayoutJob::single_section(
                    code.clone(),
                    TextFormat {
                        line_height: Some(style.line_height),
                        ..TextFormat::simple(style.font_id, style.colour)
                    },
                );
                let styled = Styled {
                    job,
                    section_links: vec![None],
                    links: Vec::new(),
                    heading: None,
                };
                self.text(
                    path,
                    x + pad.x,
                    width - pad.x * 2.0,
                    TextRole::MonospaceTechnical,
                    styled,
                );
                self.y += pad.y;
                self.pieces[fill] = Piece::Fill(Rect::from_min_max(
                    egui::pos2(x, top),
                    egui::pos2(x + width, self.y),
                ));
            }
            Block::Quote(blocks) => {
                let bar = self.tokens.spacing.unit.0 * 0.5;
                let indent = bar + self.tokens.spacing.section.0;
                if width - indent < self.minimum_content_width() {
                    self.blocks(blocks, path, x, width, true, false);
                    return;
                }
                let top = self.y;
                let line = self.pieces.len();
                self.pieces.push(Piece::Line(Rect::NOTHING));
                self.blocks(blocks, path, x + indent, width - indent, true, false);
                self.pieces[line] = Piece::Line(Rect::from_min_max(
                    egui::pos2(x, top),
                    egui::pos2(x + bar, self.y.max(top + 1.0)),
                ));
            }
            Block::List { start, items } => {
                self.list(*start, items, path, x, width, quoted);
            }
            Block::Rule => {
                self.pieces.push(Piece::Line(Rect::from_min_size(
                    egui::pos2(x, self.y),
                    egui::vec2(width, 1.0),
                )));
                self.y += 1.0;
            }
        }
    }

    fn list(
        &mut self,
        start: Option<u64>,
        items: &[Vec<Block>],
        path: &mut Vec<u32>,
        x: f32,
        width: f32,
        quoted: bool,
    ) {
        let number = |index: usize| {
            start.map(|first| first.saturating_add(u64::try_from(index).unwrap_or(u64::MAX)))
        };
        let marker_text = |index: usize| match number(index) {
            Some(n) => format!("{n}."),
            None => "•".to_owned(),
        };
        let mut column = self
            .measure(
                if start.is_some() {
                    RESERVED_NUMBER
                } else {
                    RESERVED_BULLET
                },
                TextRole::Body,
                width,
            )
            .size()
            .x;
        if start.is_some() {
            for index in 0..items.len() {
                column = column.max(
                    self.measure(&marker_text(index), TextRole::Body, width)
                        .size()
                        .x,
                );
            }
        }
        let gutter = column + self.tokens.spacing.inline.0;
        let flat = width - gutter < self.minimum_content_width();
        let colour: Color32 = if quoted {
            self.tokens.colours.text_muted.into()
        } else {
            self.style(TextRole::Body).colour
        };
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                self.y += self.tokens.spacing.block.0;
            }
            path.push(u32::try_from(index).unwrap_or(u32::MAX));
            if flat {
                // Too narrow to indent: the marker leads the item's text.
                let prefix = format!("{} ", marker_text(index));
                let mut blocks = item.clone();
                match blocks.first_mut() {
                    Some(Block::Paragraph(inline) | Block::Heading { inline, .. }) => {
                        *inline = inline.with_prefix(&prefix);
                    }
                    _ => blocks.insert(
                        0,
                        Block::Paragraph(Inline::default().with_prefix(prefix.trim_end())),
                    ),
                }
                self.blocks(&blocks, path, x, width, quoted, true);
            } else {
                let top = self.y;
                let outer = self.first_text_top.take();
                self.list_depth += 1;
                self.blocks(item, path, x + gutter, width - gutter, quoted, true);
                self.list_depth -= 1;
                let line_top = self.first_text_top.unwrap_or(top);
                self.first_text_top = outer.or(Some(line_top));
                let line_height = self.style(TextRole::Body).line_height;
                if start.is_some() {
                    let measured = self.measure(&marker_text(index), TextRole::Body, width);
                    self.pieces.push(Piece::Marker {
                        identity: self.identity.child(("marker", path.as_slice())),
                        rect: Rect::from_min_size(
                            egui::pos2(x, line_top),
                            egui::vec2(column, measured.size().y.max(line_height)),
                        ),
                        measured,
                    });
                } else {
                    let size = self.style(TextRole::Body).font_id.size;
                    self.pieces.push(Piece::Bullet {
                        centre: egui::pos2(x + column * 0.5, line_top + line_height * 0.5),
                        radius: size * BULLET_RADIUS,
                        depth: self.list_depth,
                        colour,
                    });
                }
                // An empty item still occupies a line beside its marker.
                self.y = self.y.max(top + line_height);
            }
            path.pop();
        }
    }
}

/// Each link's fragments in a galley positioned at `origin`: one rectangle
/// per row the link's text occupies, without trailing spaces. `links` are
/// byte ranges of the galley's text. Glyphs pair with the text's characters
/// in order (egui lays out one glyph per character, and a row ending in a
/// newline omits it); a glyph's own `chr` is not used, because shaping gives
/// every glyph of a multi-character cluster the cluster's first character.
fn link_fragments(
    galley: &Galley,
    origin: Pos2,
    links: &[(usize, Range<usize>)],
) -> Vec<(usize, Vec<Rect>)> {
    let mut fragments: Vec<(usize, Vec<Rect>)> =
        links.iter().map(|(link, _)| (*link, Vec::new())).collect();
    let mut characters = galley.job.text.char_indices();
    for row in &galley.rows {
        let row_characters: Vec<(usize, char)> =
            characters.by_ref().take(row.glyphs.len()).collect();
        if row.ends_with_newline {
            characters.next();
        }
        for (slot, (_, range)) in links.iter().enumerate() {
            let mut span: Option<(f32, f32)> = None;
            for (glyph, (byte, character)) in row.glyphs.iter().zip(&row_characters) {
                if range.contains(byte) && !character.is_whitespace() {
                    let (left, right) = span.unwrap_or((glyph.pos.x, glyph.max_x()));
                    span = Some((left.min(glyph.pos.x), right.max(glyph.max_x())));
                }
            }
            if let Some((left, right)) = span {
                let top = origin.y + row.pos.y;
                fragments[slot].1.push(Rect::from_min_max(
                    egui::pos2(origin.x + row.pos.x + left, top),
                    egui::pos2(origin.x + row.pos.x + right, top + row.size.y),
                ));
            }
        }
    }
    fragments
}

fn union(rects: impl IntoIterator<Item = Rect>) -> Rect {
    rects
        .into_iter()
        .fold(Rect::NOTHING, |all, rect| all.union(rect))
}

/// Present CommonMark `source` as reading text: paragraphs, headings (level
/// 1 as an application title, 2 as a pane title, 3 to 6 as section
/// headings), bulleted
/// and numbered lists to any depth, block quotes, fenced and indented code
/// in the monospace face, horizontal rules, emphasis, strong, strikethrough,
/// inline code and links. Raw HTML is shown as written and never
/// interpreted; an image shows its alternative text; tables are not
/// recognised and keep their source text.
///
/// Each block is one measured reading surface, wrapped at the available
/// width and never truncated, observed as [`TextComponentKind::ReadingText`]
/// and selectable by default with egui's selection and copy; a numbered
/// list's numbers are measured [`TextComponentKind::ListMarker`] labels.
/// Links inside the text open in a new browser tab through the link
/// recipe's rules ([`link_url_allowed`]): pointer, Tab then Enter or Space,
/// and assistive-technology activation share one route. A link whose URL is
/// not allowed keeps its text, opens nothing and is published as an
/// unavailable link with the reason. Any source, including an incomplete one
/// still arriving, presents without panicking; list spacing does not depend
/// on blank lines between items, so a list keeps its layout as it grows.
///
/// Put long content in a scroll area; text scrolled fully out of view adds
/// no observation, as with [`crate::reading_text`].
#[allow(clippy::too_many_arguments)]
pub fn markdown(
    ui: &mut egui::Ui,
    identity: PresentationId,
    source: &str,
    spec: MarkdownSpec<'_>,
    parent: SemanticUiId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> MarkdownResponse {
    present_markdown(
        ui,
        identity,
        source,
        spec,
        None,
        parent,
        tokens,
        font_scale,
        observations,
    )
    .0
}

/// [`markdown`], reserving a strip `code_header` high above each code
/// block's text, and returning each code block as laid out.
#[allow(clippy::too_many_arguments)]
pub(crate) fn present_markdown(
    ui: &mut egui::Ui,
    identity: PresentationId,
    source: &str,
    spec: MarkdownSpec<'_>,
    code_header: Option<f32>,
    parent: SemanticUiId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
) -> (MarkdownResponse, Vec<CodeBlock>) {
    let Document { blocks, links } = parse::parse(source);
    let width = ui.available_width().max(1.0);
    let enabled = ui.is_enabled();
    let painter = ui.painter().clone();
    let mut layout = Layout {
        painter: &painter,
        tokens,
        scale: font_scale,
        interaction: spec.interaction,
        enabled,
        links: &links,
        identity,
        pieces: Vec::new(),
        y: 0.0,
        list_depth: 0,
        first_text_top: None,
        code_header,
        code_blocks: Vec::new(),
    };
    layout.blocks(&blocks, &mut Vec::new(), 0.0, width, false, false);
    let height = layout.y.max(1.0);
    let mut pieces = layout.pieces;
    let mut code_blocks = layout.code_blocks;
    let (_, rect) = ui.allocate_space(egui::vec2(width, height));
    let response = ui.interact(rect, identity.egui_id(), Sense::hover());
    for piece in &mut pieces {
        piece.translate(rect.min.to_vec2());
    }
    for block in &mut code_blocks {
        block.header = block.header.translate(rect.min.to_vec2());
    }

    let mut link_nodes = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Fill(fill) => {
                ui.painter().rect_filled(
                    fill,
                    tokens.geometry.control_radius.0,
                    tokens.colours.surface_raised,
                );
            }
            Piece::Line(line) => {
                ui.painter()
                    .rect_filled(line, 0.0, tokens.colours.border_decorative);
            }
            Piece::Bullet {
                centre,
                radius,
                depth,
                colour,
            } => {
                // Nested lists alternate disc, circle and square.
                let painter = ui.painter();
                match depth % 3 {
                    0 => painter.circle_filled(centre, radius, colour),
                    1 => painter.circle_stroke(centre, radius, Stroke::new(1.0, colour)),
                    _ => painter.rect_filled(
                        Rect::from_center_size(centre, egui::Vec2::splat(radius * 2.0)),
                        0.0,
                        colour,
                    ),
                };
            }
            Piece::Marker {
                identity,
                rect,
                measured,
            } => {
                let component =
                    TextComponentId::new(TextComponentKind::ListMarker, identity.text_instance());
                let (_, observation) =
                    present_accessible_measured_text(ui, &measured, rect, component, None);
                if ui.clip_rect().intersect(rect).is_positive()
                    || observation.layout_error.is_some()
                {
                    observations.push(observation);
                }
            }
            Piece::Text(text) => present_text(
                ui,
                identity,
                &text,
                &links,
                spec,
                parent.clone(),
                tokens,
                font_scale,
                observations,
                &mut link_nodes,
            ),
        }
    }
    (
        MarkdownResponse {
            response,
            link_nodes,
        },
        code_blocks,
    )
}

#[allow(clippy::too_many_arguments)]
fn present_text(
    ui: &mut egui::Ui,
    document: PresentationId,
    text: &TextPiece,
    links: &[String],
    spec: MarkdownSpec<'_>,
    parent: SemanticUiId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    link_nodes: &mut Vec<UiNode>,
) {
    let enabled = ui.is_enabled();
    // A failed measurement paints a diagnostic, not the text: no link areas.
    let fragments = if text.measured.layout_error.is_some() {
        Vec::new()
    } else {
        link_fragments(&text.measured.galley, text.rect.min, &text.links)
    };
    // A hovered link's underline is heavier; the layout does not move.
    let hovered = fragments
        .iter()
        .find(|(link, rects)| {
            enabled
                && link_url_allowed(&links[*link])
                && rects.iter().any(|rect| ui.rect_contains_pointer(*rect))
        })
        .map(|(link, _)| *link);
    let hovered_measurement;
    let measured = match hovered {
        Some(link) => {
            let mut job = text.job.clone();
            for (section, owner) in job.sections.iter_mut().zip(&text.section_links) {
                if *owner == Some(link) {
                    section.format.underline.width = 2.0;
                }
            }
            hovered_measurement = measure_component_reading(
                ui.painter(),
                job,
                text.measured.spec.role,
                text.measured.spec.interaction,
                tokens,
                font_scale,
                text.rect.width(),
            );
            &hovered_measurement
        }
        None => &text.measured,
    };
    let component = TextComponentId::new(
        TextComponentKind::ReadingText,
        text.identity.text_instance(),
    );
    let (response, observation) = if measured.spec.interaction == TextInteraction::Selectable {
        let (response, observation) =
            present_measured_text(ui, measured, text.rect, component, None);
        (
            response.expect("selectable measured text returns a response"),
            observation,
        )
    } else {
        present_accessible_measured_text(ui, measured, text.rect, component, None)
    };
    if response.interact_rect.is_positive() {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(text.identity.semantic_id().0);
            if let Some(level) = text.heading {
                node.set_role(egui::accesskit::Role::Heading);
                node.set_level(usize::from(level));
            }
        });
    }
    if ui.clip_rect().intersect(text.rect).is_positive() || observation.layout_error.is_some() {
        observations.push(observation);
    }

    for ((link, rects), (_, bytes)) in fragments.iter().zip(&text.links) {
        if rects.is_empty() {
            continue;
        }
        let url = links[*link].as_str();
        let valid = link_url_allowed(url);
        let active = enabled && valid;
        let label = text.job.text[bytes.clone()].trim().to_owned();
        let identity = document.child(("link", *link));
        let clip = ui.clip_rect();
        let visible = union(
            rects
                .iter()
                .map(|rect| rect.intersect(clip))
                .filter(|rect| rect.is_positive()),
        );
        let primary = rects
            .iter()
            .position(|rect| rect.intersect(clip).is_positive())
            .unwrap_or(0);
        let mut clicked = false;
        let mut hovering = false;
        let mut focused = false;
        let mut primary_response = None;
        for (index, rect) in rects.iter().enumerate() {
            let (id, sense) = if index == primary {
                (
                    identity.egui_id(),
                    if active {
                        Sense::click()
                    } else {
                        Sense::hover()
                    },
                )
            } else {
                let mut sense = if active {
                    Sense::click()
                } else {
                    Sense::hover()
                };
                sense -= Sense::FOCUSABLE;
                (identity.egui_id().with(("fragment", index)), sense)
            };
            let response = ui.interact(*rect, id, sense);
            // A middle or modifier click opens in the background: also a new tab.
            clicked |= active && (response.clicked() || response.clicked_with_open_in_background());
            hovering |= response.hovered();
            if index == primary {
                focused = response.has_focus();
                primary_response = Some(response.clone());
            }
            let tip = if valid {
                url.to_owned()
            } else {
                format!("Unavailable: {INVALID_LINK_URL}")
            };
            response.on_hover_text(tip);
        }
        if clicked {
            crate::open_new_tab(ui.ctx(), url);
        }
        if hovering && active {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if focused {
            // Half a spacing unit either side keeps the ring off the glyphs,
            // within the spaces around the link.
            let clearance = egui::vec2(tokens.spacing.unit.0 * 0.5, 0.0);
            for rect in rects {
                ui.painter().rect_stroke(
                    rect.expand2(clearance),
                    tokens.geometry.control_radius.0,
                    Stroke::new(1.0, tokens.colours.focus_ring),
                    egui::StrokeKind::Outside,
                );
            }
        }
        let Some(response) = primary_response else {
            continue;
        };
        if !visible.is_positive() {
            continue;
        }
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, active, &label));
        ui.ctx().accesskit_node_builder(response.id, |node| {
            use egui::accesskit::{Action, Role};
            node.set_role(Role::Link);
            node.set_label(label.as_str());
            node.set_url(url);
            node.set_author_id(identity.semantic_id().0);
            match (spec.link_description, valid) {
                (description, false) => node.set_description(format!(
                    "{}; unavailable: {INVALID_LINK_URL}",
                    description.unwrap_or_default()
                )),
                (Some(description), true) => node.set_description(description),
                (None, true) => node.clear_description(),
            }
            node.set_bounds(egui::accesskit::Rect {
                x0: f64::from(visible.min.x),
                y0: f64::from(visible.min.y),
                x1: f64::from(visible.max.x),
                y1: f64::from(visible.max.y),
            });
            if active {
                node.add_action(Action::Click);
            } else {
                node.set_disabled();
                node.remove_action(Action::Click);
            }
        });
        let mut node = UiNode::container(
            identity.semantic_id(),
            Some(parent.clone()),
            UiRole::Link,
            visible.into(),
        );
        node.name = label;
        node.description = spec.link_description.map(str::to_owned);
        node.enabled = active;
        node.focused = focused;
        node.value = Some(url.to_owned());
        if !valid {
            node.disabled_reason = Some(INVALID_LINK_URL.to_owned());
        }
        link_nodes.push(node);
    }
}
