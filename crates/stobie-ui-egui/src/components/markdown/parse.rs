//! CommonMark source to Stobie's block model, through pulldown-cmark.
//!
//! The model keeps only what the recipe presents: paragraphs, headings,
//! lists (with task-list items' states), block quotes, code blocks, display
//! maths, footnote definitions and rules, with inline emphasis, strong,
//! strikethrough, code, maths, links and footnote references. Everything
//! else keeps its text: raw HTML is shown as written and never interpreted,
//! an image shows its alternative text, and tables stay paragraphs of their
//! source (see [`parse`]).
//!
//! The parse also maps each presented block to its source: its kind, its
//! nesting depth and its byte range as pulldown-cmark reports it.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

/// The kind of source a presented Markdown block comes from.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum MarkdownBlockKind {
    /// A paragraph, including the text of a tight list item, a definition
    /// list's term or definition, or display maths alone in a paragraph.
    Paragraph,
    /// An ATX or setext heading.
    Heading,
    /// A fenced or indented code block.
    CodeBlock,
    /// An HTML block, shown as written.
    HtmlBlock,
    /// A block quote.
    BlockQuote,
    /// A bulleted or numbered list.
    List,
    /// One item of a list.
    ListItem,
    /// A horizontal rule.
    Rule,
    /// A table, with `ENABLE_TABLES`.
    Table,
    /// A metadata block, with `ENABLE_YAML_STYLE_METADATA_BLOCKS` or
    /// `ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS`.
    MetadataBlock,
    /// A footnote definition, with `ENABLE_FOOTNOTES`: a container of the
    /// blocks after its label.
    FootnoteDefinition,
}

/// A presented Markdown block and where its source is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct MarkdownBlock {
    /// What the block is.
    pub kind: MarkdownBlockKind,
    /// How many presented quotes, lists, list items and footnote definitions
    /// enclose it: 0 at the top level.
    pub depth: usize,
    /// The block's source, in bytes, as pulldown-cmark's offset iterator
    /// reports its start tag, plus the caller's base. A paragraph without its
    /// own tag (a tight list item's text) spans its first to its last inline
    /// event.
    pub source: Range<usize>,
}

/// A presented heading and where its source is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct MarkdownHeading {
    /// Its level, 1 to 6.
    pub level: u8,
    /// Its text as presented, without markup.
    pub text: String,
    /// Its source, in bytes, plus the caller's base, as for
    /// [`MarkdownBlock::source`].
    pub source: Range<usize>,
    /// Its index in [`MarkdownSourceMap::blocks`].
    pub block: usize,
}

/// A link in Markdown source and where it is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct MarkdownLink {
    /// Where the link leads, as the recipe uses it: pulldown-cmark's
    /// destination after Markdown unescaping, with `mailto:` before an email
    /// autolink's address.
    pub destination: String,
    /// Its source, in bytes, as pulldown-cmark's offset iterator reports the
    /// link's start tag (the whole link, text and destination), plus the
    /// caller's base.
    pub source: Range<usize>,
}

/// A presented footnote definition and where its source is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct MarkdownFootnote {
    /// Its label as written, without `[^` and `]`.
    pub label: String,
    /// The number its references and its definition show, from 1: footnotes
    /// are numbered in the order their labels first appear, as a reference
    /// or a definition, matching labels without regard to case.
    pub number: usize,
    /// Its source, in bytes, as pulldown-cmark's offset iterator reports its
    /// start tag (the whole definition), plus the caller's base.
    pub source: Range<usize>,
    /// Its index in [`MarkdownSourceMap::blocks`].
    pub block: usize,
}

/// Every block a Markdown presentation shows, in document order (each
/// container before what it contains), every heading and footnote
/// definition among them and every link.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct MarkdownSourceMap {
    /// Every presented block. Empty paragraphs and headings are not
    /// presented, and containers nested deeper than the model keeps are not
    /// presented although their contents are.
    pub blocks: Vec<MarkdownBlock>,
    /// Every presented heading, in document order.
    pub headings: Vec<MarkdownHeading>,
    /// Every link, in document order: the order
    /// [`super::MarkdownSpec::link_kinds`] follows. A link whose text is
    /// empty is listed although it presents nothing.
    pub links: Vec<MarkdownLink>,
    /// Every presented footnote definition, in document order: the order
    /// [`super::MarkdownSpec::footnote_ids`] follows.
    pub footnotes: Vec<MarkdownFootnote>,
}

impl MarkdownSourceMap {
    fn offset(mut self, base: usize) -> Self {
        let shift = |range: &mut Range<usize>| {
            *range = range.start.saturating_add(base)..range.end.saturating_add(base);
        };
        for block in &mut self.blocks {
            shift(&mut block.source);
        }
        for heading in &mut self.headings {
            shift(&mut heading.source);
        }
        for link in &mut self.links {
            shift(&mut link.source);
        }
        for footnote in &mut self.footnotes {
            shift(&mut footnote.source);
        }
        self
    }
}

/// The deepest nesting of lists, block quotes and footnote definitions the
/// model keeps. Deeper
/// containers continue at this depth, so hostile input cannot exhaust the
/// stack.
pub(super) const MAX_NESTING: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct InlineStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub code: bool,
    /// Maths source, inline or display maths within other text.
    pub math: bool,
    /// Raised and smaller, as a footnote reference is.
    pub superscript: bool,
}

/// A styled stretch of an inline's text, in bytes. `link` indexes
/// [`Document::links`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Run {
    pub bytes: Range<usize>,
    pub style: InlineStyle,
    pub link: Option<usize>,
}

/// Text with its styled runs, which cover the text in order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Inline {
    pub text: String,
    pub runs: Vec<Run>,
}

impl Inline {
    fn push(&mut self, text: &str, style: InlineStyle, link: Option<usize>) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        match self.runs.last_mut() {
            Some(run) if run.style == style && run.link == link && run.bytes.end == start => {
                run.bytes.end = end;
            }
            _ => self.runs.push(Run {
                bytes: start..end,
                style,
                link,
            }),
        }
    }

    /// The same inline after `prefix` in plain text.
    pub fn with_prefix(&self, prefix: &str) -> Self {
        let mut inline = Self::default();
        inline.push(prefix, InlineStyle::default(), None);
        for run in &self.runs {
            inline.push(&self.text[run.bytes.clone()], run.style, run.link);
        }
        inline
    }

    /// Each link's byte range in the text, in order of appearance.
    pub fn link_bytes(&self) -> Vec<(usize, Range<usize>)> {
        let mut links: Vec<(usize, Range<usize>)> = Vec::new();
        for run in &self.runs {
            let Some(link) = run.link else {
                continue;
            };
            match links.last_mut() {
                Some((last, range)) if *last == link && range.end == run.bytes.start => {
                    range.end = run.bytes.end;
                }
                _ => links.push((link, run.bytes.clone())),
            }
        }
        links
    }
}

/// One item of a list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Item {
    /// A task-list item's state: `Some(true)` when done.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Block {
    Paragraph(Inline),
    Heading {
        level: u8,
        inline: Inline,
    },
    Code(String),
    /// Display maths alone in a paragraph: its source, with line endings as
    /// `\n` and without surrounding blank space.
    Math(String),
    Quote(Vec<Block>),
    List {
        /// The first number of an ordered list; `None` for bullets.
        start: Option<u64>,
        items: Vec<Item>,
    },
    /// A footnote definition and its number.
    Footnote {
        number: usize,
        blocks: Vec<Block>,
    },
    Rule,
}

/// Where a link in the text leads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LinkTarget {
    /// A URL, with its index in [`MarkdownSourceMap::links`]; an email
    /// autolink gains `mailto:`.
    Url { url: String, index: usize },
    /// A footnote reference: the footnote's number and label, and the
    /// reference's place among the document's references.
    Footnote {
        number: usize,
        label: String,
        reference: usize,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Document {
    pub blocks: Vec<Block>,
    /// Every link and footnote reference in order of appearance.
    pub links: Vec<LinkTarget>,
    /// Where each presented block's source is, relative to the parsed text.
    pub map: MarkdownSourceMap,
}

enum Container {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Item(Item),
    Footnote {
        number: usize,
        blocks: Vec<Block>,
    },
}

enum Leaf {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code(String),
    /// A paragraph that so far holds only display maths, and whether blank
    /// space followed it.
    Math(String, bool),
}

/// The source of the open leaf.
struct LeafSource {
    kind: MarkdownBlockKind,
    range: Range<usize>,
    /// Opened by inline content without a tag of its own, so it grows with
    /// each inline event.
    implicit: bool,
}

struct Builder {
    containers: Vec<Container>,
    /// For each open list, quote or item, whether it was kept as a container.
    kept: Vec<bool>,
    leaf: Option<Leaf>,
    leaf_source: Option<LeafSource>,
    /// The source range of the event being built.
    current: Range<usize>,
    map: MarkdownSourceMap,
    links: Vec<LinkTarget>,
    /// Each footnote label, folded to lower case, by number less one.
    footnotes: Vec<String>,
    /// How many footnote references have been seen.
    references: usize,
    strong: u32,
    emphasis: u32,
    strikethrough: u32,
    link: Option<usize>,
}

impl Builder {
    fn style(&self, code: bool) -> InlineStyle {
        InlineStyle {
            strong: self.strong > 0,
            emphasis: self.emphasis > 0,
            strikethrough: self.strikethrough > 0,
            code,
            math: false,
            superscript: false,
        }
    }

    /// The number of the footnote labelled `label`, numbering it if it is
    /// new.
    fn footnote(&mut self, label: &str) -> usize {
        let folded = label.to_lowercase();
        let index = match self.footnotes.iter().position(|seen| *seen == folded) {
            Some(index) => index,
            None => {
                self.footnotes.push(folded);
                self.footnotes.len() - 1
            }
        };
        index + 1
    }

    fn blocks(&mut self) -> &mut Vec<Block> {
        let top = self.containers.last_mut().expect("the root stays open");
        match top {
            Container::Root(blocks)
            | Container::Quote(blocks)
            | Container::Item(Item { blocks, .. })
            | Container::Footnote { blocks, .. } => blocks,
            // Content directly in a list belongs to its last item.
            Container::List { items, .. } => {
                if items.is_empty() {
                    items.push(Item::default());
                }
                &mut items.last_mut().expect("an item exists").blocks
            }
        }
    }

    /// The depth of a block opened now.
    fn depth(&self) -> usize {
        self.containers.len().saturating_sub(1)
    }

    /// Open `leaf`, whose source is the current event's.
    fn open_leaf(&mut self, leaf: Leaf, kind: MarkdownBlockKind) {
        self.close_leaf();
        self.leaf = Some(leaf);
        self.leaf_source = Some(LeafSource {
            kind,
            range: self.current.clone(),
            implicit: false,
        });
    }

    fn close_leaf(&mut self) {
        let source = self.leaf_source.take();
        let block = match self.leaf.take() {
            None => return,
            Some(Leaf::Paragraph(inline)) if inline.text.trim().is_empty() => return,
            Some(Leaf::Heading(_, inline)) if inline.text.trim().is_empty() => return,
            Some(Leaf::Math(text, _)) if text.trim().is_empty() => return,
            Some(Leaf::Paragraph(inline)) => Block::Paragraph(inline),
            Some(Leaf::Heading(level, inline)) => Block::Heading { level, inline },
            Some(Leaf::Math(text, _)) => Block::Math(text),
            Some(Leaf::Code(mut text)) => {
                if text.ends_with('\n') {
                    text.pop();
                }
                Block::Code(text)
            }
        };
        if let Some(LeafSource { kind, range, .. }) = source {
            if let Block::Heading { level, inline } = &block {
                self.map.headings.push(MarkdownHeading {
                    level: *level,
                    text: inline.text.clone(),
                    source: range.clone(),
                    block: self.map.blocks.len(),
                });
            }
            self.map.blocks.push(MarkdownBlock {
                kind,
                depth: self.depth(),
                source: range,
            });
        }
        self.blocks().push(block);
    }

    /// The inline text is appended to, opening a paragraph where text
    /// arrives without one, as in a tight list item.
    fn inline(&mut self) -> &mut Inline {
        // Text after display maths makes its paragraph ordinary text, the
        // maths within it.
        if matches!(self.leaf, Some(Leaf::Math(..)))
            && let Some(Leaf::Math(math, gap)) = self.leaf.take()
        {
            let mut inline = Inline::default();
            let style = InlineStyle {
                math: true,
                ..InlineStyle::default()
            };
            inline.push(&math.replace('\n', " "), style, None);
            if gap {
                inline.push(" ", InlineStyle::default(), None);
            }
            self.leaf = Some(Leaf::Paragraph(inline));
        }
        if !matches!(self.leaf, Some(Leaf::Paragraph(_) | Leaf::Heading(..))) {
            self.open_leaf(
                Leaf::Paragraph(Inline::default()),
                MarkdownBlockKind::Paragraph,
            );
            if let Some(source) = self.leaf_source.as_mut() {
                source.implicit = true;
            }
        }
        match self.leaf.as_mut() {
            Some(Leaf::Paragraph(inline) | Leaf::Heading(_, inline)) => inline,
            _ => unreachable!("an inline leaf was just opened"),
        }
    }

    /// An inline event arrived: open a paragraph for it if none is open, and
    /// grow an implicit paragraph's source to include it.
    fn inline_event(&mut self) {
        match self.leaf {
            Some(Leaf::Code(_)) => return,
            // Display maths stays alone until text other than blank space
            // arrives.
            Some(Leaf::Math(..)) => {}
            _ => {
                self.inline();
            }
        }
        if let Some(source) = self.leaf_source.as_mut()
            && source.implicit
        {
            source.range.end = source.range.end.max(self.current.end);
        }
    }

    fn text(&mut self, text: &str, code: bool) {
        if let Some(Leaf::Code(code_text)) = self.leaf.as_mut() {
            code_text.push_str(text);
            return;
        }
        // Blank space after display maths keeps it alone in its paragraph.
        if let Some(Leaf::Math(_, gap)) = self.leaf.as_mut()
            && text.trim().is_empty()
        {
            *gap |= !text.is_empty();
            return;
        }
        let style = self.style(code);
        let link = self.link;
        self.inline().push(text, style, link);
    }

    /// Plain text outside any style or link, such as a table's separators.
    fn literal(&mut self, text: &str) {
        self.inline().push(text, InlineStyle::default(), None);
    }

    /// Maths source in `text`, inline unless `display`.
    fn math(&mut self, text: &str, display: bool) {
        let text = normalise_lines(text);
        if let Some(Leaf::Code(code)) = self.leaf.as_mut() {
            code.push_str(&text);
            return;
        }
        // Display maths opening a paragraph, or following display maths in
        // it, stands alone until other text arrives.
        let fresh = matches!(
            (&self.leaf, &self.leaf_source),
            (Some(Leaf::Paragraph(inline)), Some(source))
                if inline.text.is_empty() && source.kind == MarkdownBlockKind::Paragraph
        );
        if display && fresh {
            self.leaf = Some(Leaf::Math(text.trim().to_owned(), false));
            return;
        }
        if display && let Some(Leaf::Math(math, gap)) = self.leaf.as_mut() {
            math.push('\n');
            math.push_str(text.trim());
            *gap = false;
            return;
        }
        let style = InlineStyle {
            math: true,
            ..self.style(false)
        };
        let link = self.link;
        let text = text.trim().replace('\n', " ");
        self.inline().push(&text, style, link);
    }

    /// A reference to the footnote labelled `label`.
    fn footnote_reference(&mut self, label: &str) {
        let number = self.footnote(label);
        if let Some(Leaf::Code(code)) = self.leaf.as_mut() {
            code.push_str(&format!("[^{label}]"));
            return;
        }
        self.links.push(LinkTarget::Footnote {
            number,
            label: label.to_owned(),
            reference: self.references,
        });
        self.references += 1;
        let style = InlineStyle {
            superscript: true,
            ..self.style(false)
        };
        let link = Some(self.links.len() - 1);
        self.inline().push(&format!("[{number}]"), style, link);
    }

    /// A task-list item's marker: the state of the item it opens, or its
    /// source form where no item is kept to hold it.
    fn task(&mut self, done: bool) {
        // pulldown-cmark gives the marker straight after its item opens, or
        // after the item's first paragraph opens in a loose list.
        let opening = match &self.leaf {
            None => true,
            Some(Leaf::Paragraph(inline)) => inline.text.is_empty(),
            Some(_) => false,
        };
        if opening
            && self.kept.last() == Some(&true)
            && let Some(Container::Item(item)) = self.containers.last_mut()
            && item.blocks.is_empty()
            && item.task.is_none()
        {
            item.task = Some(done);
            return;
        }
        self.inline_event();
        self.text(if done { "[x] " } else { "[ ] " }, false);
    }

    fn open(&mut self, container: Container) {
        self.close_leaf();
        let room = match container {
            Container::List { .. } => self.containers.len() + 2 <= MAX_NESTING,
            Container::Item(_) => matches!(self.containers.last(), Some(Container::List { .. })),
            _ => self.containers.len() < MAX_NESTING,
        };
        self.kept.push(room);
        if room {
            let kind = match container {
                Container::Quote(_) => MarkdownBlockKind::BlockQuote,
                Container::List { .. } => MarkdownBlockKind::List,
                Container::Footnote { .. } => MarkdownBlockKind::FootnoteDefinition,
                Container::Item(_) | Container::Root(_) => MarkdownBlockKind::ListItem,
            };
            self.map.blocks.push(MarkdownBlock {
                kind,
                depth: self.depth(),
                source: self.current.clone(),
            });
            self.containers.push(container);
        }
    }

    fn close(&mut self) {
        self.close_leaf();
        if !self.kept.pop().unwrap_or(false) {
            return;
        }
        let Some(container) = self.containers.pop() else {
            return;
        };
        match container {
            Container::Root(blocks) => self.containers.push(Container::Root(blocks)),
            Container::Quote(blocks) => self.blocks().push(Block::Quote(blocks)),
            Container::List { start, items } => self.blocks().push(Block::List { start, items }),
            Container::Footnote { number, blocks } => {
                self.blocks().push(Block::Footnote { number, blocks });
            }
            Container::Item(item) => match self.containers.last_mut() {
                Some(Container::List { items, .. }) => items.push(item),
                _ => self.blocks().extend(item.blocks),
            },
        }
    }

    fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        self.current = range;
        if is_inline(&event) {
            self.inline_event();
        }
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => self.text(&text, false),
            Event::Code(text) => self.text(&text, true),
            // Raw HTML is shown as written, never interpreted.
            Event::Html(html) | Event::InlineHtml(html) => self.text(&html, false),
            Event::SoftBreak => self.text(" ", false),
            Event::HardBreak => self.text("\n", false),
            Event::Rule => {
                self.close_leaf();
                self.map.blocks.push(MarkdownBlock {
                    kind: MarkdownBlockKind::Rule,
                    depth: self.depth(),
                    source: self.current.clone(),
                });
                self.blocks().push(Block::Rule);
            }
            Event::InlineMath(text) => self.math(&text, false),
            Event::DisplayMath(text) => self.math(&text, true),
            Event::FootnoteReference(label) => self.footnote_reference(&label),
            Event::TaskListMarker(done) => self.task(done),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.open_leaf(
                Leaf::Paragraph(Inline::default()),
                MarkdownBlockKind::Paragraph,
            ),
            Tag::Heading { level, .. } => self.open_leaf(
                Leaf::Heading(level as u8, Inline::default()),
                MarkdownBlockKind::Heading,
            ),
            Tag::CodeBlock(_) => {
                self.open_leaf(Leaf::Code(String::new()), MarkdownBlockKind::CodeBlock);
            }
            Tag::HtmlBlock => self.open_leaf(
                Leaf::Paragraph(Inline::default()),
                MarkdownBlockKind::HtmlBlock,
            ),
            Tag::MetadataBlock(_) => self.open_leaf(
                Leaf::Paragraph(Inline::default()),
                MarkdownBlockKind::MetadataBlock,
            ),
            // A table keeps its cells' text, a row to a line between `|`
            // separators.
            Tag::Table(_) => {
                self.open_leaf(Leaf::Paragraph(Inline::default()), MarkdownBlockKind::Table)
            }
            Tag::TableHead | Tag::TableRow => {
                if !self.inline().text.is_empty() {
                    self.literal("\n");
                }
                self.literal("|");
            }
            Tag::TableCell => self.literal(" "),
            Tag::FootnoteDefinition(label) => {
                let number = self.footnote(&label);
                self.open(Container::Footnote {
                    number,
                    blocks: Vec::new(),
                });
                if self.kept.last() == Some(&true) {
                    self.map.footnotes.push(MarkdownFootnote {
                        label: label.into_string(),
                        number,
                        source: self.current.clone(),
                        block: self.map.blocks.len() - 1,
                    });
                } else {
                    // Too deep to keep as a container: its label leads its
                    // blocks as text.
                    self.open_leaf(
                        Leaf::Paragraph(Inline::default()),
                        MarkdownBlockKind::FootnoteDefinition,
                    );
                    self.literal(&format!("[^{label}]:"));
                    self.close_leaf();
                }
            }
            // A definition list's terms and definitions are paragraphs.
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {
                self.close_leaf();
            }
            Tag::BlockQuote(_) => self.open(Container::Quote(Vec::new())),
            Tag::List(start) => self.open(Container::List {
                start,
                items: Vec::new(),
            }),
            Tag::Item => self.open(Container::Item(Item::default())),
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strikethrough += 1,
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                let url = if link_type == LinkType::Email {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.into_string()
                };
                self.map.links.push(MarkdownLink {
                    destination: url.clone(),
                    source: self.current.clone(),
                });
                self.links.push(LinkTarget::Url {
                    url,
                    index: self.map.links.len() - 1,
                });
                self.link = Some(self.links.len() - 1);
            }
            // An image shows its alternative text, which arrives as text.
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::MetadataBlock(_)
            | TagEnd::Table => {
                if let Some(Leaf::Paragraph(inline)) = self.leaf.as_mut()
                    && matches!(tag, TagEnd::HtmlBlock | TagEnd::MetadataBlock(_))
                {
                    while inline.text.ends_with('\n') {
                        inline.text.pop();
                        if let Some(run) = inline.runs.last_mut() {
                            run.bytes.end = inline.text.len();
                            if run.bytes.is_empty() {
                                inline.runs.pop();
                            }
                        }
                    }
                }
                self.close_leaf();
            }
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item | TagEnd::FootnoteDefinition => {
                self.close();
            }
            TagEnd::TableCell => self.literal(" |"),
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition => self.close_leaf(),
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strikethrough = self.strikethrough.saturating_sub(1),
            TagEnd::Link => self.link = None,
            _ => {}
        }
    }
}

/// Whether `event` belongs inside a block's text rather than delimiting a
/// block.
fn is_inline(event: &Event<'_>) -> bool {
    match event {
        Event::Start(tag) => matches!(
            tag,
            Tag::Emphasis
                | Tag::Strong
                | Tag::Strikethrough
                | Tag::Superscript
                | Tag::Subscript
                | Tag::Link { .. }
                | Tag::Image { .. }
        ),
        Event::End(tag) => matches!(
            tag,
            TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Superscript
                | TagEnd::Subscript
                | TagEnd::Link
                | TagEnd::Image
        ),
        Event::Rule => false,
        Event::Text(_)
        | Event::Code(_)
        | Event::Html(_)
        | Event::InlineHtml(_)
        | Event::SoftBreak
        | Event::HardBreak
        | Event::InlineMath(_)
        | Event::DisplayMath(_)
        | Event::FootnoteReference(_) => true,
        // A task-list marker belongs to its item, not to the item's text.
        Event::TaskListMarker(_) => false,
    }
}

/// `text` with each CRLF or lone CR line ending as LF.
fn normalise_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// Parse CommonMark with pulldown-cmark's `options`. Any source, including an
/// incomplete one still arriving, parses without panicking, and nesting stays
/// within [`MAX_NESTING`] whatever the options.
///
/// With their extensions enabled, a task-list item records its state, a
/// footnote reference is a link to its footnote's number shown as `[n]`, a
/// footnote definition is a container numbered as its references are, and
/// maths keeps its source without its `$` delimiters, styled as maths:
/// display maths alone in a paragraph is a block of its own, its line
/// endings normalised to LF. A task-list marker no item can hold (one nested
/// deeper than the model keeps) keeps its source form, `[ ] ` or `[x] `. A
/// table's cells stay text, a row to a line between `|` separators. A
/// metadata block shows its text, as an HTML block does.
pub(super) fn parse(source: &str, options: Options) -> Document {
    let mut builder = Builder {
        containers: vec![Container::Root(Vec::new())],
        kept: Vec::new(),
        leaf: None,
        leaf_source: None,
        current: 0..0,
        map: MarkdownSourceMap::default(),
        links: Vec::new(),
        footnotes: Vec::new(),
        references: 0,
        strong: 0,
        emphasis: 0,
        strikethrough: 0,
        link: None,
    };
    for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
        builder.event(event, range);
    }
    builder.close_leaf();
    while builder.containers.len() > 1 {
        builder.kept.push(true);
        builder.close();
    }
    let Some(Container::Root(blocks)) = builder.containers.pop() else {
        unreachable!("the root is the last container");
    };
    Document {
        blocks,
        links: builder.links,
        map: builder.map,
    }
}

/// The source map of `source` parsed with `options`, its ranges shifted by
/// `base`.
pub(super) fn source_map(source: &str, options: Options, base: usize) -> MarkdownSourceMap {
    parse(source, options).map.offset(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_default(source: &str) -> Document {
        parse(source, Options::ENABLE_STRIKETHROUGH)
    }

    fn plain(text: &str) -> Inline {
        let mut inline = Inline::default();
        inline.push(text, InlineStyle::default(), None);
        inline
    }

    fn depth(blocks: &[Block]) -> usize {
        blocks
            .iter()
            .map(|block| match block {
                Block::Quote(blocks) | Block::Footnote { blocks, .. } => 1 + depth(blocks),
                Block::List { items, .. } => {
                    2 + items
                        .iter()
                        .map(|item| depth(&item.blocks))
                        .max()
                        .unwrap_or(0)
                }
                _ => 0,
            })
            .max()
            .unwrap_or(0)
    }

    /// Each URL link's destination, in order.
    fn urls(document: &Document) -> Vec<&str> {
        document
            .links
            .iter()
            .filter_map(|link| match link {
                LinkTarget::Url { url, .. } => Some(url.as_str()),
                LinkTarget::Footnote { .. } => None,
            })
            .collect()
    }

    fn item(blocks: Vec<Block>) -> Item {
        Item { task: None, blocks }
    }

    #[test]
    fn blocks_and_inline_styles_follow_commonmark() {
        let document = parse_default(
            "# Title\n\n## Part\n\n###### Small\n\nA *quiet* **bold** ~~gone~~ `code` \
             [link](https://example.org/a) <https://example.org/b> <me@example.org>.\n\n\
             - one\n- two\n  1. nested\n  2. again\n\n3. three\n4. four\n\n\
             > quoted\n> > deeper\n\n```rust\nfn main() {\n\tprintln!();\n}\n```\n\n    indented code\n\n\
             ---\n\nline  \nbreak",
        );
        let blocks = &document.blocks;
        assert!(
            matches!(&blocks[0], Block::Heading { level: 1, inline } if inline.text == "Title")
        );
        assert!(matches!(&blocks[1], Block::Heading { level: 2, .. }));
        assert!(matches!(&blocks[2], Block::Heading { level: 6, .. }));
        let Block::Paragraph(inline) = &blocks[3] else {
            panic!("a paragraph: {:?}", blocks[3]);
        };
        assert_eq!(
            inline.text,
            "A quiet bold gone code link https://example.org/b me@example.org."
        );
        let styled = |text: &str| {
            inline
                .runs
                .iter()
                .find(|run| &inline.text[run.bytes.clone()] == text)
                .unwrap_or_else(|| panic!("a run for {text:?}: {:?}", inline.runs))
        };
        assert!(styled("quiet").style.emphasis);
        assert!(styled("bold").style.strong);
        assert!(styled("gone").style.strikethrough);
        assert!(styled("code").style.code);
        assert_eq!(styled("link").link, Some(0));
        assert_eq!(
            urls(&document),
            [
                "https://example.org/a",
                "https://example.org/b",
                "mailto:me@example.org"
            ]
        );
        assert_eq!(
            inline
                .link_bytes()
                .iter()
                .map(|(link, bytes)| (*link, inline.text[bytes.clone()].to_owned()))
                .collect::<Vec<_>>(),
            [
                (0, "link".to_owned()),
                (1, "https://example.org/b".to_owned()),
                (2, "me@example.org".to_owned())
            ]
        );
        let Block::List { start: None, items } = &blocks[4] else {
            panic!("a bulleted list: {:?}", blocks[4]);
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[0], item(vec![Block::Paragraph(plain("one"))]));
        assert!(
            matches!(&items[1].blocks[1], Block::List { start: Some(1), items } if items.len() == 2)
        );
        assert!(matches!(&blocks[5], Block::List { start: Some(3), items } if items.len() == 2));
        let Block::Quote(quoted) = &blocks[6] else {
            panic!("a quote: {:?}", blocks[6]);
        };
        assert_eq!(quoted[0], Block::Paragraph(plain("quoted")));
        assert!(
            matches!(&quoted[1], Block::Quote(deeper) if deeper[0] == Block::Paragraph(plain("deeper")))
        );
        assert_eq!(
            blocks[7],
            Block::Code("fn main() {\n\tprintln!();\n}".to_owned())
        );
        assert_eq!(blocks[8], Block::Code("indented code".to_owned()));
        assert_eq!(blocks[9], Block::Rule);
        assert_eq!(blocks[10], Block::Paragraph(plain("line\nbreak")));
        assert_eq!(blocks.len(), 11);
    }

    #[test]
    fn raw_html_images_and_tables_keep_their_text() {
        let document = parse_default(
            "<script>alert(1)</script>\n\nUse Vec<T> and <b>bold</b>.\n\n\
             ![A chart of results](https://example.org/chart.png)\n\n\
             | a | b |\n| - | - |\n| 1 | 2 |",
        );
        assert_eq!(
            document.blocks,
            [
                Block::Paragraph(plain("<script>alert(1)</script>")),
                Block::Paragraph(plain("Use Vec<T> and <b>bold</b>.")),
                Block::Paragraph(plain("A chart of results")),
                Block::Paragraph(plain(
                    "| a | b |\n| - | - |\n| 1 | 2 |"
                        .replace('\n', " ")
                        .as_str()
                )),
            ]
        );
        assert!(document.links.is_empty());
    }

    /// The extensions `knowledge-markdown-v1` enables, as a document
    /// application might parse with.
    const DOCUMENT: Options = Options::ENABLE_TABLES
        .union(Options::ENABLE_FOOTNOTES)
        .union(Options::ENABLE_STRIKETHROUGH)
        .union(Options::ENABLE_TASKLISTS)
        .union(Options::ENABLE_MATH);

    /// Every option set the bounds must hold under.
    const OPTION_SETS: [Options; 3] = [Options::ENABLE_STRIKETHROUGH, DOCUMENT, Options::all()];

    #[test]
    fn hostile_nesting_is_bounded_under_every_option_set() {
        let sources = [
            ">".repeat(100_000),
            "- ".repeat(50_000),
            (0..2_000)
                .map(|level| format!("{}- item\n", "  ".repeat(level)))
                .collect(),
            "[".repeat(50_000) + &"]".repeat(50_000),
            "*".repeat(100_000),
            "> [^a]: - ".repeat(20_000),
            "[^a]: ".repeat(20_000),
            (0..2_000)
                .map(|level| format!("{}- [x] [^n{level}]\n", "  ".repeat(level)))
                .collect(),
            "| a ".repeat(10_000) + "|\n" + &"|-".repeat(10_000) + "|\n",
            "$".repeat(100_000),
            "> | a |\n> |---|\n".repeat(5_000),
        ];
        for options in OPTION_SETS {
            for source in &sources {
                let document = parse(source, options);
                assert!(
                    depth(&document.blocks) <= MAX_NESTING,
                    "{options:?}: {}",
                    depth(&document.blocks)
                );
                assert!(
                    document
                        .map
                        .blocks
                        .iter()
                        .all(|block| block.depth < MAX_NESTING),
                    "{options:?}"
                );
            }
        }
    }

    #[test]
    fn raw_html_stays_text_under_every_option_set() {
        for options in OPTION_SETS {
            let document = parse(
                "<script>alert(1)</script>\n\nUse Vec<T> and <b>bold</b>.\n\n\
                 <img src=x onerror=alert(1)>",
                options,
            );
            assert_eq!(
                document.blocks,
                [
                    Block::Paragraph(plain("<script>alert(1)</script>")),
                    Block::Paragraph(plain("Use Vec<T> and <b>bold</b>.")),
                    Block::Paragraph(plain("<img src=x onerror=alert(1)>")),
                ],
                "{options:?}"
            );
        }
    }

    /// Each block's text, a list or quote's blocks in order.
    fn texts(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .flat_map(|block| match block {
                Block::Paragraph(inline) | Block::Heading { inline, .. } => {
                    vec![inline.text.clone()]
                }
                Block::Code(code) | Block::Math(code) => vec![code.clone()],
                Block::Quote(blocks) | Block::Footnote { blocks, .. } => texts(blocks),
                Block::List { items, .. } => {
                    items.iter().flat_map(|item| texts(&item.blocks)).collect()
                }
                Block::Rule => vec!["---".to_owned()],
            })
            .collect()
    }

    /// The text of each run of `inline` styled as `style` says.
    fn styled(inline: &Inline, style: impl Fn(&Run) -> bool) -> Vec<&str> {
        inline
            .runs
            .iter()
            .filter(|run| style(run))
            .map(|run| &inline.text[run.bytes.clone()])
            .collect()
    }

    #[test]
    fn enabled_extensions_present_their_structure() {
        let source = "Maths $a+b$ and $$c$$.\n\n- [x] done\n- [ ] open\n\nNote[^1].\n\n\
                      [^1]: The *note*.\n\n| a | b |\n| - | - |\n| 1 | **2** |\n";
        let document = parse(source, DOCUMENT);
        assert_eq!(
            texts(&document.blocks),
            [
                "Maths a+b and c.",
                "done",
                "open",
                "Note[1].",
                "The note.",
                "| a | b |\n| 1 | 2 |",
            ]
        );
        let Block::Paragraph(maths) = &document.blocks[0] else {
            panic!("a paragraph: {:?}", document.blocks[0]);
        };
        assert_eq!(styled(maths, |run| run.style.math), ["a+b", "c"]);
        let Block::List { items, .. } = &document.blocks[1] else {
            panic!("a list: {:?}", document.blocks[1]);
        };
        assert_eq!(
            items.iter().map(|item| item.task).collect::<Vec<_>>(),
            [Some(true), Some(false)]
        );
        let Block::Paragraph(note) = &document.blocks[2] else {
            panic!("a paragraph: {:?}", document.blocks[2]);
        };
        assert_eq!(styled(note, |run| run.style.superscript), ["[1]"]);
        assert_eq!(note.link_bytes(), [(0, 4..7)]);
        assert_eq!(
            document.links,
            [LinkTarget::Footnote {
                number: 1,
                label: "1".to_owned(),
                reference: 0,
            }]
        );
        assert!(matches!(
            &document.blocks[3],
            Block::Footnote { number: 1, .. }
        ));
        // The default options keep the same source as written.
        assert_eq!(
            texts(&parse(source, Options::ENABLE_STRIKETHROUGH).blocks),
            [
                "Maths $a+b$ and $$c$$.",
                "[x] done",
                "[ ] open",
                "Note[^1].",
                "[^1]: The note.",
                "| a | b | | - | - | | 1 | 2 |",
            ]
        );
    }

    /// Every extension a document application enables, with CRLF line
    /// endings and multibyte text.
    const EXTENDED: &str = "# Café 漢字\r\n\r\n\
        Setext *héading* ✓\r\n---\r\n\r\n\
        A paragraph with ~~struck~~ text, $x^2$ maths and a note[^n] 🙂.\r\n\r\n\
        - [x] done ✓\r\n- [ ] öffen\r\n\r\n\
        | Name | Wert |\r\n| :--- | ---: |\r\n| α | **1** |\r\n\r\n\
        $$\r\nE = mc^2\r\n$$\r\n\r\n\
        > Quoted 🙂\r\n>\r\n> 1. one\r\n>\r\n>    more\r\n\r\n\
        ```text\r\ncode ü\r\n```\r\n\r\n\
        <div>raw ü</div>\r\n\r\n\
        ***\r\n\r\n\
        [^n]: The note, ünïcode.\r\n";

    /// Each block pulldown-cmark starts with a tag of its own, with its
    /// depth among quotes, lists and items, straight from its offset
    /// iterator.
    fn tagged_blocks(source: &str, options: Options) -> Vec<MarkdownBlock> {
        let mut depth = 0;
        let mut blocks = Vec::new();
        for (event, range) in Parser::new_ext(source, options).into_offset_iter() {
            let kind = match &event {
                Event::Start(Tag::Paragraph) => Some(MarkdownBlockKind::Paragraph),
                Event::Start(Tag::Heading { .. }) => Some(MarkdownBlockKind::Heading),
                Event::Start(Tag::CodeBlock(_)) => Some(MarkdownBlockKind::CodeBlock),
                Event::Start(Tag::HtmlBlock) => Some(MarkdownBlockKind::HtmlBlock),
                Event::Start(Tag::BlockQuote(_)) => Some(MarkdownBlockKind::BlockQuote),
                Event::Start(Tag::List(_)) => Some(MarkdownBlockKind::List),
                Event::Start(Tag::Item) => Some(MarkdownBlockKind::ListItem),
                Event::Start(Tag::Table(_)) => Some(MarkdownBlockKind::Table),
                Event::Start(Tag::MetadataBlock(_)) => Some(MarkdownBlockKind::MetadataBlock),
                Event::Start(Tag::FootnoteDefinition(_)) => {
                    Some(MarkdownBlockKind::FootnoteDefinition)
                }
                Event::Rule => Some(MarkdownBlockKind::Rule),
                _ => None,
            };
            if let Some(kind) = kind {
                blocks.push(MarkdownBlock {
                    kind,
                    depth,
                    source: range,
                });
            }
            match event {
                Event::Start(
                    Tag::BlockQuote(_) | Tag::List(_) | Tag::Item | Tag::FootnoteDefinition(_),
                ) => depth += 1,
                Event::End(
                    TagEnd::BlockQuote(_)
                    | TagEnd::List(_)
                    | TagEnd::Item
                    | TagEnd::FootnoteDefinition,
                ) => depth -= 1,
                _ => {}
            }
        }
        blocks
    }

    #[test]
    fn block_and_heading_offsets_equal_pulldown_cmarks() {
        let map = parse(EXTENDED, DOCUMENT).map;
        let tagged = tagged_blocks(EXTENDED, DOCUMENT);
        // Every tagged block is presented, in order; the rest are the tight
        // task items' text, which has no tag of its own and does not include
        // the item's marker.
        let (matching, untagged): (Vec<_>, Vec<_>) =
            map.blocks.iter().partition(|block| tagged.contains(block));
        assert_eq!(matching.into_iter().cloned().collect::<Vec<_>>(), tagged);
        assert_eq!(
            untagged
                .iter()
                .map(|block| (block.kind, block.depth, &EXTENDED[block.source.clone()]))
                .collect::<Vec<_>>(),
            [
                (MarkdownBlockKind::Paragraph, 2, "done ✓"),
                (MarkdownBlockKind::Paragraph, 2, "öffen"),
            ]
        );
        let kinds: Vec<_> = map.blocks.iter().map(|block| block.kind).collect();
        for kind in [
            MarkdownBlockKind::Heading,
            MarkdownBlockKind::Paragraph,
            MarkdownBlockKind::List,
            MarkdownBlockKind::ListItem,
            MarkdownBlockKind::Table,
            MarkdownBlockKind::BlockQuote,
            MarkdownBlockKind::CodeBlock,
            MarkdownBlockKind::HtmlBlock,
            MarkdownBlockKind::Rule,
            MarkdownBlockKind::FootnoteDefinition,
        ] {
            assert!(kinds.contains(&kind), "{kind:?} in {kinds:?}");
        }
        assert_eq!(
            map.footnotes
                .iter()
                .map(|note| (
                    note.label.as_str(),
                    note.number,
                    &EXTENDED[note.source.clone()]
                ))
                .collect::<Vec<_>>(),
            [("n", 1, "[^n]: The note, ünïcode.\r\n")]
        );
        let definition = &map.blocks[map.footnotes[0].block];
        assert_eq!(definition.kind, MarkdownBlockKind::FootnoteDefinition);
        assert_eq!(definition.source, map.footnotes[0].source);
        for block in &map.blocks {
            assert!(EXTENDED.get(block.source.clone()).is_some(), "{block:?}");
        }
        assert_eq!(
            map.headings
                .iter()
                .map(|heading| (
                    heading.level,
                    heading.text.as_str(),
                    &EXTENDED[heading.source.clone()]
                ))
                .collect::<Vec<_>>(),
            [
                (1, "Café 漢字", "# Café 漢字\r\n"),
                (2, "Setext héading ✓", "Setext *héading* ✓\r\n---\r\n"),
            ]
        );
        for heading in &map.headings {
            let block = &map.blocks[heading.block];
            assert_eq!(block.kind, MarkdownBlockKind::Heading);
            assert_eq!(block.source, heading.source);
        }
    }

    #[test]
    fn a_caller_supplied_base_maps_a_body_into_its_file() {
        // A byte order mark and CRLF frontmatter precede the body; the
        // caller passes the body alone and where it starts.
        let file = format!("{}---\r\ntitle: Synthetic\r\n---\r\n{EXTENDED}", '\u{feff}');
        let base = file.len() - EXTENDED.len();
        let body = &file[base..];
        assert!(!body.starts_with('\u{feff}'));
        let relative = source_map(body, DOCUMENT, 0);
        let mapped = source_map(body, DOCUMENT, base);
        assert_eq!(relative, parse(EXTENDED, DOCUMENT).map);
        assert_eq!(relative.blocks.len(), mapped.blocks.len());
        for (relative, mapped) in relative.blocks.iter().zip(&mapped.blocks) {
            assert_eq!(
                mapped.source,
                relative.source.start + base..relative.source.end + base
            );
            assert_eq!(&file[mapped.source.clone()], &body[relative.source.clone()]);
        }
        assert_eq!(&file[mapped.headings[0].source.clone()], "# Café 漢字\r\n");
        assert_eq!(
            &file[mapped.footnotes[0].source.clone()],
            "[^n]: The note, ünïcode.\r\n"
        );
        assert_eq!(
            source_map(body, DOCUMENT, usize::MAX).blocks[0].source,
            usize::MAX..usize::MAX
        );
    }

    /// Links of every kind, in every place a document puts them, with CRLF
    /// line endings and multibyte text.
    const LINKED: &str = "# See [the guide](guide.md#set-up) ✓\r\n\r\n\
        Read [café notes](../notes/café.md \"Notes\"), [a reference][ref], \
        <https://example.org/a> and <someone@example.org>.\r\n\
        Then [empty]() and [](https://example.org/blank) and \
        ![a chart of [the data](data.md)](chart.png).\r\n\r\n\
        | Page | Link |\r\n| --- | --- |\r\n| α | [table](#table) |\r\n\r\n\
        - [x] [task](tasks.md)\r\n\r\n\
        > [quoted \\[escaped\\]](quoted%20page.md)\r\n\r\n\
        Note[^n].\r\n\r\n\
        [^n]: In a [footnote](#notes).\r\n\r\n\
        [ref]: https://example.org/ref\r\n";

    #[test]
    fn link_offsets_and_destinations_equal_pulldown_cmarks() {
        let mut expected = Vec::new();
        for (event, range) in Parser::new_ext(LINKED, DOCUMENT).into_offset_iter() {
            if let Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) = event
            {
                let destination = if link_type == LinkType::Email {
                    format!("mailto:{dest_url}")
                } else {
                    dest_url.into_string()
                };
                expected.push(MarkdownLink {
                    destination,
                    source: range,
                });
            }
        }
        let document = parse(LINKED, DOCUMENT);
        assert_eq!(document.map.links, expected);
        // The map lists the links the recipe presents, in the same order.
        assert_eq!(
            document
                .map
                .links
                .iter()
                .map(|link| link.destination.as_str())
                .collect::<Vec<_>>(),
            urls(&document)
        );
        // Each URL link knows its place in the map; the footnote reference
        // is not among them.
        let mut places = Vec::new();
        for link in &document.links {
            if let LinkTarget::Url { index, .. } = link {
                places.push(*index);
            }
        }
        assert_eq!(places, (0..expected.len()).collect::<Vec<_>>());
        assert_eq!(document.links.len(), expected.len() + 1);
        assert_eq!(
            expected
                .iter()
                .map(|link| (link.destination.as_str(), &LINKED[link.source.clone()]))
                .collect::<Vec<_>>(),
            [
                ("guide.md#set-up", "[the guide](guide.md#set-up)"),
                (
                    "../notes/café.md",
                    "[café notes](../notes/café.md \"Notes\")"
                ),
                ("https://example.org/ref", "[a reference][ref]"),
                ("https://example.org/a", "<https://example.org/a>"),
                ("mailto:someone@example.org", "<someone@example.org>"),
                ("", "[empty]()"),
                ("https://example.org/blank", "[](https://example.org/blank)"),
                ("data.md", "[the data](data.md)"),
                ("#table", "[table](#table)"),
                ("tasks.md", "[task](tasks.md)"),
                (
                    "quoted%20page.md",
                    "[quoted \\[escaped\\]](quoted%20page.md)"
                ),
                ("#notes", "[footnote](#notes)"),
            ]
        );
        // The caller's base shifts every link's range.
        let base = 21;
        let file = format!("{}{LINKED}", "x".repeat(base));
        let mapped = source_map(&file[base..], DOCUMENT, base);
        assert_eq!(mapped.links.len(), expected.len());
        for (mapped, relative) in mapped.links.iter().zip(&expected) {
            assert_eq!(mapped.destination, relative.destination);
            assert_eq!(
                &file[mapped.source.clone()],
                &LINKED[relative.source.clone()]
            );
        }
        // The default options find the same links outside the extensions.
        let default = parse(LINKED, Options::ENABLE_STRIKETHROUGH).map.links;
        assert!(default.len() >= 10, "{default:#?}");
    }

    #[test]
    fn empty_blocks_are_dropped_and_items_keep_a_place() {
        assert!(parse("", Options::ENABLE_STRIKETHROUGH).blocks.is_empty());
        assert!(parse("#", Options::ENABLE_STRIKETHROUGH).blocks.is_empty());
        assert!(
            parse("   \n\n  ", Options::ENABLE_STRIKETHROUGH)
                .blocks
                .is_empty()
        );
        assert_eq!(
            parse("- a\n-", Options::ENABLE_STRIKETHROUGH).blocks,
            [Block::List {
                start: None,
                items: vec![item(vec![Block::Paragraph(plain("a"))]), item(vec![])],
            }]
        );
    }

    #[test]
    fn a_footnote_definition_keeps_its_label_whatever_opens_it() {
        let document = parse(
            "Text[^a] and[^q].\n\n[^a]: - one\n    - two\n\n> [^q]: ## Heading q\n",
            DOCUMENT,
        );
        let [
            Block::Paragraph(_),
            Block::Footnote { number: 1, blocks },
            Block::Quote(quoted),
        ] = document.blocks.as_slice()
        else {
            panic!(
                "a paragraph, a definition and a quote: {:?}",
                document.blocks
            );
        };
        assert!(matches!(blocks.as_slice(), [Block::List { items, .. }] if items.len() == 2));
        assert_eq!(
            quoted.as_slice(),
            [Block::Footnote {
                number: 2,
                blocks: vec![Block::Heading {
                    level: 2,
                    inline: plain("Heading q"),
                }],
            }]
        );
        assert_eq!(
            document
                .map
                .footnotes
                .iter()
                .map(|note| (
                    note.label.as_str(),
                    note.number,
                    document.map.blocks[note.block].depth
                ))
                .collect::<Vec<_>>(),
            [("a", 1, 0), ("q", 2, 1)]
        );
        // A definition nested deeper than the model keeps leads its blocks
        // with its label as text.
        let deep = format!("{}[^d]: deep\n", "> ".repeat(MAX_NESTING + 4));
        let texts = texts(&parse(&deep, DOCUMENT).blocks);
        assert_eq!(texts, ["[^d]:", "deep"]);
    }

    #[test]
    fn footnotes_are_numbered_by_first_appearance_without_regard_to_case() {
        let document = parse(
            "B[^b], A[^a] and b again[^B].\n\n[^a]: A.\n\n[^b]: B.\n",
            DOCUMENT,
        );
        let numbers: Vec<_> = document
            .links
            .iter()
            .map(|link| match link {
                LinkTarget::Footnote {
                    number,
                    label,
                    reference,
                } => (*number, label.as_str(), *reference),
                LinkTarget::Url { .. } => panic!("no URL links"),
            })
            .collect();
        assert_eq!(numbers, [(1, "b", 0), (2, "a", 1), (1, "B", 2)]);
        let Block::Paragraph(text) = &document.blocks[0] else {
            panic!("a paragraph: {:?}", document.blocks[0]);
        };
        assert_eq!(text.text, "B[1], A[2] and b again[1].");
        assert_eq!(
            document
                .map
                .footnotes
                .iter()
                .map(|note| (note.label.as_str(), note.number))
                .collect::<Vec<_>>(),
            [("a", 2), ("b", 1)]
        );
    }

    #[test]
    fn a_task_marker_belongs_to_its_item() {
        // pulldown-cmark gives the marker outside a paragraph here: the
        // item is an empty task, and its text a paragraph after the list.
        let document = parse("- [ ]\n\n  text\n", DOCUMENT);
        assert!(
            !texts(&document.blocks)
                .iter()
                .any(|text| text.contains('[')),
            "{:?}",
            document.blocks
        );
        let Block::List { items, .. } = &document.blocks[0] else {
            panic!("a list: {:?}", document.blocks);
        };
        assert_eq!(items[0].task, Some(false));
        // Loose items keep their state and their paragraphs.
        let loose = parse("1. [x] a\n\n2. [ ] b\n\n   more\n", DOCUMENT);
        let Block::List { items, .. } = &loose.blocks[0] else {
            panic!("a list: {:?}", loose.blocks);
        };
        assert_eq!(
            items
                .iter()
                .map(|item| (item.task, texts(&item.blocks)))
                .collect::<Vec<_>>(),
            [
                (Some(true), vec!["a".to_owned()]),
                (Some(false), vec!["b".to_owned(), "more".to_owned()]),
            ]
        );
        // A marker no kept item holds keeps its source form.
        let deep: String = (0..MAX_NESTING)
            .map(|level| format!("{}- [x] t{level}\n", "  ".repeat(level)))
            .collect();
        let texts = texts(&parse(&deep, DOCUMENT).blocks);
        assert!(texts.contains(&"t0".to_owned()), "{texts:?}");
        assert!(
            texts.iter().any(|text| text.starts_with("[x] t")),
            "{texts:?}"
        );
    }

    #[test]
    fn display_maths_is_a_block_with_lf_line_endings() {
        let document = parse(
            "$$\r\nE = mc^2\r\n$$\r\n\r\n$$a\rb$$\r\n$$c$$\r\n\r\nx $$d\r\ne$$ y $f\r\ng$\r\n",
            DOCUMENT,
        );
        assert_eq!(document.blocks[0], Block::Math("E = mc^2".to_owned()));
        assert_eq!(document.blocks[1], Block::Math("a\nb\nc".to_owned()));
        let Block::Paragraph(mixed) = &document.blocks[2] else {
            panic!("a paragraph: {:?}", document.blocks[2]);
        };
        assert!(!mixed.text.contains('\r'), "{:?}", mixed.text);
        assert_eq!(styled(mixed, |run| run.style.math), ["d e", "f g"]);
        assert_eq!(document.blocks.len(), 3);
        // Display maths followed by text becomes text with maths in it.
        let document = parse("$$a$$ then text\n", DOCUMENT);
        let [Block::Paragraph(inline)] = document.blocks.as_slice() else {
            panic!("a paragraph: {:?}", document.blocks);
        };
        assert_eq!(inline.text, "a then text");
        assert_eq!(styled(inline, |run| run.style.math), ["a"]);
        // Each display maths block is a paragraph in the source map.
        let map = parse("$$\r\nx\r\n$$\r\n", DOCUMENT).map;
        assert_eq!(map.blocks[0].kind, MarkdownBlockKind::Paragraph);
    }
}
