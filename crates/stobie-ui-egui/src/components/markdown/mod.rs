//! Markdown on the reading-text path: CommonMark blocks and inline styles,
//! each block a measured reading surface, with links inside the text.

mod parse;

use std::ops::Range;

use egui::{
    Color32, CursorIcon, FontFamily, FontId, Galley, Painter, Pos2, Rect, Response, Sense, Stroke,
    TextFormat, text::LayoutJob,
};

use self::parse::{Block, Document, Inline, Item, LinkTarget};
pub use self::parse::{
    MarkdownBlock, MarkdownBlockKind, MarkdownFootnote, MarkdownHeading, MarkdownLink,
    MarkdownSourceMap,
};
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
/// The narrowest marker column of a footnote definition.
const RESERVED_FOOTNOTE: &str = "[00]";
/// A task-list item's box's side as a share of its text's font size.
const TASK_BOX: f32 = 0.85;
/// The name of a task-list item without text of its own to name it.
const UNNAMED_TASK: &str = "Task";
/// A footnote reference's size as a share of its text's.
const SUPERSCRIPT: f32 = 0.75;
/// The reason a footnote reference gives when its definition is not
/// presented, such as one nested deeper than the recipe keeps.
const UNPRESENTED_FOOTNOTE: &str = "The footnote is not shown";

/// pulldown-cmark's parser options, which choose the CommonMark extensions a
/// Markdown presentation parses. An application that parses the same source
/// itself passes the options it parses with, so both parses find the same
/// blocks; see [`markdown_source_map`].
pub use pulldown_cmark::Options as MarkdownOptions;

/// The parser options a [`MarkdownSpec`] has by default: CommonMark with
/// strikethrough.
pub const DEFAULT_MARKDOWN_OPTIONS: MarkdownOptions = MarkdownOptions::ENABLE_STRIKETHROUGH;

/// How a Markdown presentation behaves. Its typography comes from the tokens
/// (the reading profile for a reading surface) and its colours from the
/// theme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownSpec<'a> {
    /// Selectable by default, as reading text is. Choose Inert only where a
    /// parent owns the pointer; links stay activatable either way.
    pub interaction: TextInteraction,
    /// Context for assistive technology on every external link, such as
    /// "Opens in a new tab", in the application's language. In-application
    /// links and links the application makes unavailable
    /// ([`MarkdownSpec::link_kinds`]) do not carry it.
    pub link_description: Option<&'a str>,
    /// The extensions to parse with, [`DEFAULT_MARKDOWN_OPTIONS`] by
    /// default. Raw HTML stays text and nesting stays bounded whatever they
    /// are. With `ENABLE_TASKLISTS`, a task-list item shows a read-only box,
    /// checked when the task is done, in place of its bullet or number. With
    /// `ENABLE_FOOTNOTES`, a footnote reference is a link shown as a raised
    /// `[n]` that scrolls its definition into view, and each definition is a
    /// muted block numbered `[n]` after a short rule. With `ENABLE_MATH`,
    /// maths keeps its TeX source, never typeset, in the italic monospace
    /// face: inline maths within its text, and display maths alone in a
    /// paragraph as a block of its own. A table's cells keep their source
    /// text, a row to a line between `|` separators.
    pub options: MarkdownOptions,
    /// The application's ID for each heading, in the order
    /// [`markdown_source_map`] lists them for the same source and options;
    /// none by default. A heading with an ID is published as a
    /// [`UiRole::Heading`] node with that ID, named by its text, so an agent
    /// (`stobie reveal ID`) or the application ([`crate::Frame::reveal`]) can
    /// scroll it into view. The ID belongs to the heading, not to its place
    /// in the layout: it is the same at every width and text size. IDs must
    /// be unique among the application's nodes, for example by naming the
    /// document in each. A heading past the end of the list, or given an
    /// empty ID, publishes no node. Stobie derives no IDs: the application
    /// chooses them, for example from each heading's text or source range.
    pub heading_ids: &'a [SemanticUiId],
    /// How each link behaves, as the application classifies its destination,
    /// in the order [`markdown_source_map`] lists links for the same source
    /// and options; every link is [`MarkdownLinkKind::External`] by default
    /// and past the end of the list. Stobie resolves nothing itself: the
    /// application decides, for example, that a relative path names one of
    /// its own documents.
    pub link_kinds: &'a [MarkdownLinkKind<'a>],
    /// The application's ID for each footnote definition, in the order
    /// [`markdown_source_map`] lists them ([`MarkdownSourceMap::footnotes`])
    /// for the same source and options; none by default. A definition with
    /// an ID is published as a [`UiRole::Section`] node with that ID, named
    /// by its number as shown (`[1]`), which an agent or the application
    /// reveals as it does a heading. Footnote references reach their
    /// definitions with or without IDs.
    pub footnote_ids: &'a [SemanticUiId],
}

impl Default for MarkdownSpec<'_> {
    fn default() -> Self {
        Self {
            interaction: TextInteraction::Selectable,
            link_description: None,
            options: DEFAULT_MARKDOWN_OPTIONS,
            heading_ids: &[],
            link_kinds: &[],
            footnote_ids: &[],
        }
    }
}

/// How a Markdown link behaves, as the application classifies its
/// destination ([`MarkdownSpec::link_kinds`]). Every kind keeps the link's
/// text, colour, underline and place in the text; an available link keeps
/// the link recipe's pointer, keyboard and assistive-technology behaviour
/// and its [`UiRole::Link`] node, which agents operate through
/// `stobie.open-link`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum MarkdownLinkKind<'a> {
    /// Opens in a new browser tab, or the platform's default browser
    /// natively, under the link recipe's rules: a destination
    /// [`link_url_allowed`] rejects stays an unavailable link whatever the
    /// application says.
    #[default]
    External,
    /// Navigates within the application. Following it opens nothing: the
    /// recipe reports it ([`MarkdownResponse::followed`],
    /// [`crate::PresentationContext::followed_link`]) and the application
    /// navigates, for example by showing another document or revealing a
    /// heading ([`crate::Frame::reveal`]).
    InApplication,
    /// Keeps its text, opens nothing and is published as an unavailable link
    /// with this reason, in the application's language, such as "No document
    /// has this name".
    Unavailable(&'a str),
}

/// An in-application link ([`MarkdownLinkKind::InApplication`]) the reader
/// followed, by pointer, keyboard, assistive technology or an agent.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct FollowedMarkdownLink {
    /// Its index in [`MarkdownSourceMap::links`] and
    /// [`MarkdownSpec::link_kinds`].
    pub index: usize,
    /// Where it leads, as [`MarkdownLink::destination`] gives it.
    pub destination: String,
}

/// How a presented link behaves, resolved from the application's
/// classification and the link recipe's rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinkRoute<'a> {
    /// Opens its URL in a new tab.
    Open,
    /// Reported to the application as followed.
    Follow,
    /// Opens nothing, for this reason.
    Unavailable(&'a str),
    /// Scrolls the definition of the footnote of this number into view.
    Footnote(usize),
}

/// A document link or footnote reference with its resolved behaviour.
struct ResolvedLink<'a> {
    /// A URL, or `#` and a footnote's label.
    destination: String,
    route: LinkRoute<'a>,
    /// Whether it carries no link description: the application made it
    /// unavailable, or it is a footnote reference.
    withheld: bool,
    /// A URL link's index in [`MarkdownSourceMap::links`].
    index: Option<usize>,
    /// Its identity's key within the presentation: a URL link's index, or a
    /// footnote reference's place among the references.
    key: (&'static str, usize),
}

impl<'a> ResolvedLink<'a> {
    /// `target` as the application classifies it (`kinds`), a footnote
    /// reference available when its definition is presented (`presented`).
    fn new(target: &LinkTarget, kinds: &[MarkdownLinkKind<'a>], presented: &[usize]) -> Self {
        let (destination, index, kind) = match target {
            LinkTarget::Url { url, index } => (
                url.as_str(),
                *index,
                kinds.get(*index).copied().unwrap_or_default(),
            ),
            LinkTarget::Footnote {
                number,
                label,
                reference,
            } => {
                return Self {
                    destination: format!("#{label}"),
                    route: if presented.contains(number) {
                        LinkRoute::Footnote(*number)
                    } else {
                        LinkRoute::Unavailable(UNPRESENTED_FOOTNOTE)
                    },
                    withheld: true,
                    index: None,
                    key: ("footnote-reference", *reference),
                };
            }
        };
        let (route, withheld) = match kind {
            // The application cannot loosen the link recipe's rules.
            MarkdownLinkKind::External if link_url_allowed(destination) => (LinkRoute::Open, false),
            MarkdownLinkKind::External => (LinkRoute::Unavailable(INVALID_LINK_URL), false),
            MarkdownLinkKind::InApplication => (LinkRoute::Follow, false),
            MarkdownLinkKind::Unavailable(reason) if reason.trim().is_empty() => {
                (LinkRoute::Unavailable(UNAVAILABLE_LINK), true)
            }
            MarkdownLinkKind::Unavailable(reason) => (LinkRoute::Unavailable(reason), true),
        };
        Self {
            destination: destination.to_owned(),
            route,
            withheld,
            index: Some(index),
            key: ("link", index),
        }
    }

    fn available(&self) -> bool {
        !matches!(self.route, LinkRoute::Unavailable(_))
    }
}

/// The reason an unavailable link gives when the application gave none.
const UNAVAILABLE_LINK: &str = "The link is unavailable";

/// The in-application links one pass presented, which an agent may follow.
#[derive(Clone, Default)]
struct Followable {
    pass: u64,
    links: Vec<(SemanticUiId, egui::Id)>,
}

fn followable_id(context: &egui::Context) -> egui::Id {
    egui::Id::new(("stobie.markdown.followable", context.viewport_id()))
}

/// Record that this pass presented the in-application link `id`, whose
/// focusable widget is `widget`.
fn register_followable(context: &egui::Context, id: SemanticUiId, widget: egui::Id) {
    let pass = context.cumulative_pass_nr();
    let key = followable_id(context);
    context.data_mut(|data| {
        let followable = data.get_temp_mut_or_default::<Followable>(key);
        if followable.pass != pass {
            *followable = Followable {
                pass,
                links: Vec::new(),
            };
        }
        followable.links.push((id, widget));
    });
}

/// The in-application links the current pass presented, each with its
/// focusable widget, for a host to follow when an agent opens one.
pub(crate) fn followable_links(context: &egui::Context) -> Vec<(SemanticUiId, egui::Id)> {
    let pass = context.cumulative_pass_nr();
    let key = followable_id(context);
    context.data(|data| {
        data.get_temp::<Followable>(key)
            .filter(|followable| followable.pass == pass)
            .map(|followable| followable.links)
            .unwrap_or_default()
    })
}

fn follow_request_id(widget: egui::Id) -> egui::Id {
    widget.with("stobie.markdown.follow")
}

/// Ask the in-application link whose widget is `widget` to be followed on
/// its next presentation, as activating it would.
pub(crate) fn request_follow(context: &egui::Context, widget: egui::Id) {
    context.data_mut(|data| data.insert_temp(follow_request_id(widget), true));
}

/// Withdraw a request to follow that no presentation took.
pub(crate) fn cancel_follow(context: &egui::Context, widget: egui::Id) {
    context.data_mut(|data| data.remove::<bool>(follow_request_id(widget)));
}

/// Take a request to follow the link whose widget is `widget`.
fn take_follow(context: &egui::Context, widget: egui::Id) -> bool {
    context.data_mut(|data| {
        data.remove_temp::<bool>(follow_request_id(widget))
            .unwrap_or(false)
    })
}

/// Where each block that [`markdown`] presents for `source` comes from,
/// without presenting it: every presented block's kind, nesting depth and
/// source byte range, and every heading's level, text and source byte range,
/// in document order. Pass the [`MarkdownSpec::options`] the presentation
/// uses. Ranges are the byte offsets pulldown-cmark reports in `source`, plus
/// `base`: give the offset at which `source` starts in a larger text, such as
/// a file whose frontmatter precedes the Markdown body, to receive ranges in
/// that text. Ranges count bytes, so CRLF line endings and multibyte
/// characters count as they are stored.
///
/// ```
/// use stobie_ui_egui::{MarkdownBlockKind, MarkdownOptions, markdown_source_map};
///
/// let file = "---\ntitle: Notes\n---\n# Notes\n\nA paragraph.\n";
/// let body = file.find("# ").unwrap();
/// let map = markdown_source_map(&file[body..], MarkdownOptions::ENABLE_TABLES, body);
/// assert_eq!(map.blocks[0].kind, MarkdownBlockKind::Heading);
/// assert_eq!(map.headings[0].text, "Notes");
/// assert_eq!(&file[map.headings[0].source.clone()], "# Notes\n");
/// assert_eq!(&file[map.blocks[1].source.clone()], "A paragraph.\n");
/// ```
pub fn markdown_source_map(
    source: &str,
    options: MarkdownOptions,
    base: usize,
) -> MarkdownSourceMap {
    parse::source_map(source, options, base)
}

/// What [`markdown`] presented.
#[non_exhaustive]
pub struct MarkdownResponse {
    /// The whole presentation's allocation, sensing hover only.
    pub response: Response,
    /// One [`UiRole::Link`] node for each link at least partly visible, in
    /// document order. [`crate::PresentationContext::markdown`] records them;
    /// low-level callers publish them.
    pub link_nodes: Vec<UiNode>,
    /// One [`UiRole::Heading`] node for each heading with an ID from
    /// [`MarkdownSpec::heading_ids`] at least partly visible, in document
    /// order, published as `link_nodes` are.
    pub heading_nodes: Vec<UiNode>,
    /// One [`UiRole::Checkbox`] node for each task-list item's box at least
    /// partly visible, in document order: read-only and so not enabled,
    /// checked when the task is done, named by the item's first text.
    /// Published as `link_nodes` are.
    pub task_nodes: Vec<UiNode>,
    /// One [`UiRole::Section`] node for each footnote definition with an ID
    /// from [`MarkdownSpec::footnote_ids`] at least partly visible, in
    /// document order, published as `link_nodes` are.
    pub footnote_nodes: Vec<UiNode>,
    /// The first in-application link ([`MarkdownLinkKind::InApplication`])
    /// followed in this pass: the application navigates to it. External
    /// links open in a new tab and are not reported.
    pub followed: Option<FollowedMarkdownLink>,
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
    /// The application's ID for a heading.
    anchor: Option<SemanticUiId>,
    /// How many bytes of the text lead it without being its own, such as a
    /// list item's marker where an item is too narrow to indent.
    lead: usize,
}

/// A block's styled text before measurement.
struct Styled {
    job: LayoutJob,
    /// The document link each section of `job` belongs to.
    section_links: Vec<Option<usize>>,
    /// Each link in the text, by byte range.
    links: Vec<(usize, Range<usize>)>,
    heading: Option<u8>,
    /// The application's ID for a heading.
    anchor: Option<SemanticUiId>,
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
    /// A task-list item's read-only box.
    Task {
        identity: PresentationId,
        rect: Rect,
        done: bool,
        name: String,
        /// In a quote, painted in the muted text colour.
        muted: bool,
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
            Self::Marker { rect, .. }
            | Self::Task { rect, .. }
            | Self::Fill(rect)
            | Self::Line(rect) => {
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
    links: &'a [ResolvedLink<'a>],
    /// The application's ID for each heading, in document order.
    heading_ids: &'a [SemanticUiId],
    /// How many headings have been laid out.
    headings: usize,
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
    /// The application's ID for each footnote definition, in document order.
    footnote_ids: &'a [SemanticUiId],
    /// Each footnote definition laid out, in document order.
    footnotes: Vec<FootnoteArea>,
    /// How many bytes lead the next text laid out without being its own.
    lead: usize,
}

/// A footnote definition as laid out.
struct FootnoteArea {
    number: usize,
    /// The whole definition, its number and its blocks.
    rect: Rect,
    /// The application's ID for it.
    anchor: Option<SemanticUiId>,
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
            let family = if run.style.code || run.style.math {
                FontFamily::Monospace
            } else if run.style.strong {
                FontFamily::Name(crate::SEMIBOLD_FONT_FAMILY.into())
            } else {
                style.font_id.family.clone()
            };
            let valid = run.link.is_some_and(|link| self.links[link].available());
            let colour = match run.link {
                Some(_) if valid && self.enabled => link_colour(self.tokens),
                Some(_) if valid => self.tokens.colours.text_muted.into(),
                _ => base,
            };
            let size = if run.style.superscript {
                style.font_id.size * SUPERSCRIPT
            } else {
                style.font_id.size
            };
            let mut format = TextFormat {
                line_height: Some(style.line_height),
                // Maths is source, not typeset: italic monospace, where code
                // is upright on a surface.
                italics: run.style.emphasis || run.style.math,
                ..TextFormat::simple(FontId::new(size, family), colour)
            };
            if run.style.superscript {
                format.valign = egui::Align::TOP;
            }
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
            anchor,
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
        // A heading the application named keeps an identity of its own name,
        // whatever its place.
        let identity = match &anchor {
            Some(id) => self.identity.child(("heading", id.0.as_str())),
            None => self.identity.child(("block", path)),
        };
        let lead = std::mem::take(&mut self.lead);
        self.pieces.push(Piece::Text(TextPiece {
            identity,
            rect,
            job,
            section_links,
            links,
            measured,
            heading,
            anchor,
            lead,
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
        let anchor = heading.and_then(|_| {
            let index = self.headings;
            self.headings += 1;
            self.heading_ids
                .get(index)
                .filter(|id| !id.0.is_empty())
                .cloned()
        });
        let (job, section_links) = self.inline_job(inline, role, quoted, None);
        let styled = Styled {
            job,
            section_links,
            links: inline.link_bytes(),
            heading,
            anchor,
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
            let footnote = matches!(block, Block::Footnote { .. });
            let run = footnote && matches!(previous, Some(Block::Footnote { .. }));
            if let Some(previous) = previous {
                self.y += if in_item || run || matches!(previous, Block::Heading { .. }) {
                    self.tokens.spacing.block.0
                } else {
                    self.tokens.spacing.section.0
                };
            }
            // A short rule sets a run of footnote definitions apart.
            if footnote && !run {
                self.pieces.push(Piece::Line(Rect::from_min_size(
                    egui::pos2(x, self.y),
                    egui::vec2((width / 3.0).max(1.0), 1.0),
                )));
                self.y += 1.0 + self.tokens.spacing.block.0;
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
                // Laid out with tabs as four spaces; copied as written.
                let job = LayoutJob::single_section(
                    code.replace('\t', "    "),
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
                    anchor: None,
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
            Block::Math(math) => {
                // Display maths is indented where there is room.
                let indent = self.tokens.spacing.section.0;
                let (x, width) = if width - indent < self.minimum_content_width() {
                    (x, width)
                } else {
                    (x + indent, width - indent)
                };
                let style = self.style(TextRole::MonospaceTechnical);
                let colour = if quoted {
                    self.tokens.colours.text_muted.into()
                } else {
                    style.colour
                };
                let job = LayoutJob::single_section(
                    math.clone(),
                    TextFormat {
                        line_height: Some(style.line_height),
                        italics: true,
                        ..TextFormat::simple(style.font_id, colour)
                    },
                );
                let styled = Styled {
                    job,
                    section_links: vec![None],
                    links: Vec::new(),
                    heading: None,
                    anchor: None,
                };
                self.text(path, x, width, TextRole::MonospaceTechnical, styled);
            }
            Block::List { start, items } => {
                self.list(*start, items, path, x, width, quoted);
            }
            Block::Footnote { number, blocks } => {
                self.footnote(*number, blocks, path, x, width);
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

    /// A footnote definition: its number in a column of its own, or above
    /// its blocks where it is too narrow to indent, and its blocks muted.
    fn footnote(
        &mut self,
        number: usize,
        blocks: &[Block],
        path: &mut Vec<u32>,
        x: f32,
        width: f32,
    ) {
        let index = self.footnotes.len();
        let anchor = self
            .footnote_ids
            .get(index)
            .filter(|id| !id.0.is_empty())
            .cloned();
        let marker = format!("[{number}]");
        let measured = self.measure(&marker, TextRole::Body, width);
        let column = self
            .measure(RESERVED_FOOTNOTE, TextRole::Body, width)
            .size()
            .x
            .max(measured.size().x);
        let gutter = column + self.tokens.spacing.inline.0;
        let line_height = self.style(TextRole::Body).line_height;
        let identity = self.identity.child(("footnote-marker", path.as_slice()));
        let top = self.y;
        if width - gutter < self.minimum_content_width() {
            let rect = Rect::from_min_size(
                egui::pos2(x, top),
                egui::vec2(column.min(width), measured.size().y.max(line_height)),
            );
            self.pieces.push(Piece::Marker {
                identity,
                rect,
                measured,
            });
            self.y = rect.bottom();
            if !blocks.is_empty() {
                self.y += self.tokens.spacing.block.0;
            }
            self.blocks(blocks, path, x, width, true, true);
        } else {
            let outer = self.first_text_top.take();
            self.blocks(blocks, path, x + gutter, width - gutter, true, true);
            let line_top = self.first_text_top.unwrap_or(top);
            self.first_text_top = outer.or(Some(line_top));
            self.pieces.push(Piece::Marker {
                identity,
                rect: Rect::from_min_size(
                    egui::pos2(x, line_top),
                    egui::vec2(column, measured.size().y.max(line_height)),
                ),
                measured,
            });
            self.y = self.y.max(top + line_height);
        }
        self.footnotes.push(FootnoteArea {
            number,
            rect: Rect::from_min_max(
                egui::pos2(x, top),
                egui::pos2(x + width, self.y.max(top + 1.0)),
            ),
            anchor,
        });
    }

    /// A task-list item's box, its top-left corner at `min`.
    fn task(&mut self, path: &[u32], min: Pos2, done: bool, item: &Item, quoted: bool) {
        let side = self.task_side();
        let name = match item.blocks.first() {
            Some(Block::Paragraph(inline) | Block::Heading { inline, .. })
                if !inline.text.trim().is_empty() =>
            {
                inline.text.trim().to_owned()
            }
            _ => UNNAMED_TASK.to_owned(),
        };
        self.pieces.push(Piece::Task {
            identity: self.identity.child(("task", path)),
            rect: Rect::from_min_size(min, egui::Vec2::splat(side)),
            done,
            name,
            muted: quoted,
        });
    }

    fn task_side(&self) -> f32 {
        (self.style(TextRole::Body).font_id.size * TASK_BOX).round()
    }

    fn list(
        &mut self,
        start: Option<u64>,
        items: &[Item],
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
            let line_height = self.style(TextRole::Body).line_height;
            if flat && let Some(done) = item.task {
                // Too narrow to indent: a task's box takes a line of its own,
                // keyed by the item as it is when indented.
                let side = self.task_side();
                let min = egui::pos2(x, self.y + ((line_height - side) * 0.5).max(0.0));
                self.task(path, min, done, item, quoted);
                self.y += line_height.max(side);
                if !item.blocks.is_empty() {
                    self.y += self.tokens.spacing.block.0;
                }
                self.blocks(&item.blocks, path, x, width, quoted, true);
            } else if flat {
                // Too narrow to indent: the marker leads the item's text.
                let prefix = format!("{} ", marker_text(index));
                let mut blocks = item.blocks.clone();
                match blocks.first_mut() {
                    Some(Block::Paragraph(inline) | Block::Heading { inline, .. }) => {
                        *inline = inline.with_prefix(&prefix);
                        self.lead = prefix.len();
                    }
                    // The marker takes a line of its own, keyed apart from
                    // the item's blocks so that theirs do not depend on the
                    // width.
                    _ => {
                        path.push(u32::MAX);
                        let marker = Inline::default().with_prefix(prefix.trim_end());
                        self.inline(path, x, width, &marker, quoted, None);
                        path.pop();
                        if !blocks.is_empty() {
                            self.y += self.tokens.spacing.block.0;
                        }
                    }
                }
                self.blocks(&blocks, path, x, width, quoted, true);
            } else {
                let top = self.y;
                let outer = self.first_text_top.take();
                self.list_depth += 1;
                self.blocks(&item.blocks, path, x + gutter, width - gutter, quoted, true);
                self.list_depth -= 1;
                let line_top = self.first_text_top.unwrap_or(top);
                self.first_text_top = outer.or(Some(line_top));
                if let Some(done) = item.task {
                    // A task's box takes the place of its bullet or number.
                    let side = self.task_side();
                    let min = egui::pos2(
                        x + ((column - side) * 0.5).max(0.0),
                        line_top + ((line_height - side) * 0.5).max(0.0),
                    );
                    self.task(path, min, done, item, quoted);
                } else if start.is_some() {
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
/// interpreted; an image shows its alternative text. [`MarkdownSpec::options`]
/// chooses the extensions parsed, strikethrough alone by default; tables,
/// footnotes, task lists and maths keep their source text or form.
/// [`markdown_source_map`] reports where each presented block's source is.
/// A heading the application gives an ID ([`MarkdownSpec::heading_ids`])
/// is published as a [`UiRole::Heading`] node with that ID, which a reveal
/// scrolls into view even from wholly out of view.
///
/// Each block is one measured reading surface, wrapped at the available
/// width and never truncated, observed as [`TextComponentKind::ReadingText`]
/// and selectable by default with egui's selection and copy; a numbered
/// list's numbers are measured [`TextComponentKind::ListMarker`] labels.
/// Links inside the text open in a new browser tab through the link
/// recipe's rules ([`link_url_allowed`]): pointer, Tab then Enter or Space,
/// and assistive-technology activation share one route. A link whose URL is
/// not allowed keeps its text, opens nothing and is published as an
/// unavailable link with the reason. The application may instead classify a
/// link as in-application, which reports it as followed rather than opening
/// it, or as unavailable with its own reason ([`MarkdownSpec::link_kinds`]).
/// Any source, including an incomplete one
/// still arriving, presents without panicking; list spacing does not depend
/// on blank lines between items, so a list keeps its layout as it grows.
///
/// Put long content in a scroll area; text scrolled fully out of view adds
/// no observation or node, as with [`crate::reading_text`].
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
    let Document { blocks, links, map } = parse::parse(source, spec.options);
    let presented: Vec<usize> = map.footnotes.iter().map(|note| note.number).collect();
    let links: Vec<_> = links
        .iter()
        .map(|target| ResolvedLink::new(target, spec.link_kinds, &presented))
        .collect();
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
        heading_ids: spec.heading_ids,
        headings: 0,
        identity,
        pieces: Vec::new(),
        y: 0.0,
        list_depth: 0,
        first_text_top: None,
        code_header,
        code_blocks: Vec::new(),
        footnote_ids: spec.footnote_ids,
        footnotes: Vec::new(),
        lead: 0,
    };
    layout.blocks(&blocks, &mut Vec::new(), 0.0, width, false, false);
    let height = layout.y.max(1.0);
    let mut pieces = layout.pieces;
    let mut code_blocks = layout.code_blocks;
    let mut footnotes = layout.footnotes;
    let (_, rect) = ui.allocate_space(egui::vec2(width, height));
    let response = ui.interact(rect, identity.egui_id(), Sense::hover());
    for piece in &mut pieces {
        piece.translate(rect.min.to_vec2());
    }
    for block in &mut code_blocks {
        block.header = block.header.translate(rect.min.to_vec2());
    }
    for footnote in &mut footnotes {
        footnote.rect = footnote.rect.translate(rect.min.to_vec2());
    }

    let mut nodes = Nodes::default();
    let mut followed = None;
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
            Piece::Task {
                identity,
                rect,
                done,
                name,
                muted,
            } => {
                if let Some(node) = present_task(
                    ui,
                    identity,
                    rect,
                    done,
                    &name,
                    muted,
                    parent.clone(),
                    tokens,
                ) {
                    nodes.tasks.push(node);
                }
            }
            Piece::Text(text) => present_text(
                ui,
                identity,
                &text,
                &links,
                &footnotes,
                spec,
                parent.clone(),
                tokens,
                font_scale,
                observations,
                &mut nodes,
                &mut followed,
            ),
        }
    }
    for footnote in &footnotes {
        let Some(anchor) = &footnote.anchor else {
            continue;
        };
        // A definition scrolled wholly out of view publishes no node, but a
        // reveal of its ID still finds it.
        crate::observation::reveal_at(ui, anchor, footnote.rect);
        let visible = ui.clip_rect().intersect(footnote.rect);
        if visible.is_positive() {
            let mut node = UiNode::container(
                anchor.clone(),
                Some(parent.clone()),
                UiRole::Section,
                visible.into(),
            );
            node.name = format!("[{}]", footnote.number);
            node.enabled = enabled;
            nodes.footnotes.push(node);
        }
    }
    (
        MarkdownResponse {
            response,
            link_nodes: nodes.links,
            heading_nodes: nodes.headings,
            task_nodes: nodes.tasks,
            footnote_nodes: nodes.footnotes,
            followed,
        },
        code_blocks,
    )
}

/// The nodes a presentation publishes, by kind.
#[derive(Default)]
struct Nodes {
    links: Vec<UiNode>,
    headings: Vec<UiNode>,
    tasks: Vec<UiNode>,
    footnotes: Vec<UiNode>,
}

/// Paint a task-list item's read-only box and describe it to assistive
/// technology, returning its node when it is at least partly visible.
#[allow(clippy::too_many_arguments)]
fn present_task(
    ui: &mut egui::Ui,
    identity: PresentationId,
    rect: Rect,
    done: bool,
    name: &str,
    muted: bool,
    parent: SemanticUiId,
    tokens: &DesignTokens,
) -> Option<UiNode> {
    let colours = &tokens.colours;
    let radius = tokens.geometry.control_radius.0.min(rect.width() * 0.25);
    let painter = ui.painter();
    if done {
        let fill: Color32 = if muted {
            colours.text_muted.into()
        } else {
            colours.accent_primary.into()
        };
        let mark: Color32 = if muted {
            colours.surface_canvas.into()
        } else {
            colours.accent_on_accent.into()
        };
        painter.rect_filled(rect, radius, fill);
        let points = [(0.22, 0.52), (0.42, 0.72), (0.78, 0.3)]
            .map(|(x, y)| rect.lerp_inside(egui::vec2(x, y)))
            .to_vec();
        painter.add(egui::Shape::line(
            points,
            Stroke::new((rect.width() * 0.14).max(1.5), mark),
        ));
    } else {
        let edge: Color32 = if muted {
            colours.text_muted.into()
        } else {
            colours.border_control.into()
        };
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new((rect.width() * 0.1).max(1.0), edge),
            egui::StrokeKind::Inside,
        );
    }
    let response = ui.interact(rect, identity.egui_id(), Sense::hover());
    let visible = ui.clip_rect().intersect(rect);
    if !visible.is_positive() || !response.interact_rect.is_positive() {
        return None;
    }
    // Read-only, as a rendered task list's disabled checkbox is.
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, false, done, name));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::{Action, Role, Toggled};
        node.set_role(Role::CheckBox);
        node.set_label(name);
        node.set_author_id(identity.semantic_id().0);
        node.set_toggled(if done { Toggled::True } else { Toggled::False });
        node.clear_selected();
        node.clear_description();
        node.set_disabled();
        node.remove_action(Action::Click);
        node.set_bounds(egui::accesskit::Rect {
            x0: f64::from(visible.min.x),
            y0: f64::from(visible.min.y),
            x1: f64::from(visible.max.x),
            y1: f64::from(visible.max.y),
        });
    });
    let mut node = UiNode::container(
        identity.semantic_id(),
        Some(parent),
        UiRole::Checkbox,
        visible.into(),
    );
    node.name = name.to_owned();
    node.enabled = false;
    node.checked = Some(done);
    Some(node)
}

#[allow(clippy::too_many_arguments)]
fn present_text(
    ui: &mut egui::Ui,
    document: PresentationId,
    text: &TextPiece,
    links: &[ResolvedLink<'_>],
    footnotes: &[FootnoteArea],
    spec: MarkdownSpec<'_>,
    parent: SemanticUiId,
    tokens: &DesignTokens,
    font_scale: f32,
    observations: &mut Vec<TextLayoutObservation>,
    nodes: &mut Nodes,
    followed: &mut Option<FollowedMarkdownLink>,
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
                && links[*link].available()
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
    let author = text
        .anchor
        .clone()
        .unwrap_or_else(|| text.identity.semantic_id());
    // A heading's own words, without a list item's marker leading it.
    let own = text.job.text.get(text.lead..).unwrap_or_default();
    if response.interact_rect.is_positive() {
        ui.ctx().accesskit_node_builder(response.id, |node| {
            node.set_author_id(author.0.clone());
            if let Some(level) = text.heading {
                node.set_role(egui::accesskit::Role::Heading);
                node.set_level(usize::from(level));
                if text.lead > 0 {
                    node.set_value(own);
                }
            }
        });
    }
    let visible = ui.clip_rect().intersect(text.rect).is_positive();
    if let Some(anchor) = &text.anchor {
        // A heading scrolled wholly out of view publishes no node, but a
        // reveal of its ID still finds it.
        crate::observation::reveal_at(ui, anchor, text.rect);
        if visible && response.interact_rect.is_positive() {
            let mut node = UiNode::container(
                anchor.clone(),
                Some(parent.clone()),
                UiRole::Heading,
                response.rect.into(),
            );
            node.name = own.to_owned();
            node.enabled = enabled;
            nodes.headings.push(node);
        }
    }
    if visible || observation.layout_error.is_some() {
        observations.push(observation);
    }

    for ((link, rects), (_, bytes)) in fragments.iter().zip(&text.links) {
        if rects.is_empty() {
            continue;
        }
        let resolved = &links[*link];
        let url = resolved.destination.as_str();
        let valid = resolved.available();
        let active = enabled && valid;
        // The application's own unavailable links do not carry the
        // description it gives external links, nor do in-application links
        // or footnote references.
        let description = match resolved.route {
            LinkRoute::Follow | LinkRoute::Footnote(_) => None,
            LinkRoute::Unavailable(_) if resolved.withheld => None,
            _ => spec.link_description,
        };
        let label = text.job.text[bytes.clone()].trim().to_owned();
        let identity = document.child(resolved.key);
        let follows = matches!(resolved.route, LinkRoute::Follow | LinkRoute::Footnote(_));
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
        // An agent's request to follow is a press, taken whatever the
        // pointer does.
        let mut clicked = follows && take_follow(ui.ctx(), identity.egui_id()) && active;
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
            match resolved.route {
                LinkRoute::Unavailable(reason) => {
                    response.on_hover_text(format!("Unavailable: {reason}"));
                }
                // A footnote reference's destination is within view of
                // the reader once followed.
                LinkRoute::Footnote(_) => {}
                LinkRoute::Open | LinkRoute::Follow => {
                    response.on_hover_text(url);
                }
            }
        }
        if clicked {
            match resolved.route {
                LinkRoute::Open => crate::open_new_tab(ui.ctx(), url),
                LinkRoute::Follow => {
                    followed.get_or_insert_with(|| FollowedMarkdownLink {
                        index: resolved.index.unwrap_or(*link),
                        destination: url.to_owned(),
                    });
                }
                LinkRoute::Footnote(number) => {
                    if let Some(footnote) = footnotes.iter().find(|note| note.number == number) {
                        ui.scroll_to_rect(footnote.rect, Some(egui::Align::TOP));
                    }
                }
                LinkRoute::Unavailable(_) => {}
            }
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
            match (description, resolved.route) {
                (description, LinkRoute::Unavailable(reason)) => node.set_description(
                    crate::semantics::unavailable_description(description, reason),
                ),
                (Some(description), _) => node.set_description(description),
                (None, _) => node.clear_description(),
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
        node.description = description.map(str::to_owned);
        node.enabled = active;
        node.focused = focused;
        node.value = Some(url.to_owned());
        if let LinkRoute::Unavailable(reason) = resolved.route {
            node.disabled_reason = Some(reason.to_owned());
        }
        if active && follows {
            register_followable(ui.ctx(), node.id.clone(), identity.egui_id());
        }
        nodes.links.push(node);
    }
}
