//! CommonMark source to Stobie's block model, through pulldown-cmark.
//!
//! The model keeps only what the recipe presents: paragraphs, headings,
//! lists, block quotes, code blocks and rules, with inline emphasis, strong,
//! strikethrough, code and links. Everything else keeps its text: raw HTML is
//! shown as written and never interpreted, an image shows its alternative
//! text, tables not enabled stay paragraphs of their source, and the
//! extensions a caller enables keep their source form (see [`parse`]).
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
    /// list's term or definition, or a footnote definition's text.
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
}

/// A presented Markdown block and where its source is.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct MarkdownBlock {
    /// What the block is.
    pub kind: MarkdownBlockKind,
    /// How many presented quotes, lists and list items enclose it: 0 at the
    /// top level.
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

/// Every block a Markdown presentation shows, in document order (each
/// container before what it contains), every heading among them and every
/// link.
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
        self
    }
}

/// The deepest nesting of lists and block quotes the model keeps. Deeper
/// containers continue at this depth, so hostile input cannot exhaust the
/// stack.
pub(super) const MAX_NESTING: usize = 16;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct InlineStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub code: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Block {
    Paragraph(Inline),
    Heading {
        level: u8,
        inline: Inline,
    },
    Code(String),
    Quote(Vec<Block>),
    List {
        /// The first number of an ordered list; `None` for bullets.
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Rule,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct Document {
    pub blocks: Vec<Block>,
    /// Every link's URL in order of appearance; an email autolink gains
    /// `mailto:`.
    pub links: Vec<String>,
    /// Where each presented block's source is, relative to the parsed text.
    pub map: MarkdownSourceMap,
}

enum Container {
    Root(Vec<Block>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Item(Vec<Block>),
}

enum Leaf {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code(String),
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
    /// Text that leads the next inline text, such as a footnote
    /// definition's label.
    prefix: Option<String>,
    links: Vec<String>,
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
        }
    }

    fn blocks(&mut self) -> &mut Vec<Block> {
        let top = self.containers.last_mut().expect("the root stays open");
        match top {
            Container::Root(blocks) | Container::Quote(blocks) | Container::Item(blocks) => blocks,
            // Content directly in a list belongs to its last item.
            Container::List { items, .. } => {
                if items.is_empty() {
                    items.push(Vec::new());
                }
                items.last_mut().expect("an item exists")
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
            Some(Leaf::Paragraph(inline)) => Block::Paragraph(inline),
            Some(Leaf::Heading(level, inline)) => Block::Heading { level, inline },
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
        if matches!(self.leaf, Some(Leaf::Code(_))) {
            return;
        }
        self.inline();
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
        if let Some(prefix) = self.prefix.take() {
            self.literal(&prefix);
        }
        let style = self.style(code);
        let link = self.link;
        self.inline().push(text, style, link);
    }

    /// Plain text outside any style or link, such as a table's separators.
    fn literal(&mut self, text: &str) {
        self.inline().push(text, InlineStyle::default(), None);
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
            Container::Item(blocks) => match self.containers.last_mut() {
                Some(Container::List { items, .. }) => items.push(blocks),
                _ => self.blocks().extend(blocks),
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
            // Extensions the recipe does not present yet keep their source
            // form.
            Event::InlineMath(text) => self.text(&format!("${text}$"), false),
            Event::DisplayMath(text) => self.text(&format!("$${text}$$"), false),
            Event::FootnoteReference(label) => self.text(&format!("[^{label}]"), false),
            Event::TaskListMarker(done) => self.text(if done { "[x] " } else { "[ ] " }, false),
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
            // A footnote definition's text follows its label.
            Tag::FootnoteDefinition(label) => {
                self.close_leaf();
                self.prefix = Some(format!("[^{label}]: "));
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
            Tag::Item => self.open(Container::Item(Vec::new())),
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
                self.links.push(url);
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
            TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item => self.close(),
            TagEnd::TableCell => self.literal(" |"),
            TagEnd::FootnoteDefinition => {
                self.close_leaf();
                self.prefix = None;
            }
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
        | Event::FootnoteReference(_)
        | Event::TaskListMarker(_) => true,
    }
}

/// Parse CommonMark with pulldown-cmark's `options`. Any source, including an
/// incomplete one still arriving, parses without panicking, and nesting stays
/// within [`MAX_NESTING`] whatever the options.
///
/// Extensions the recipe does not present yet keep their source form:
/// inline maths as `$…$`, display maths as `$$…$$`, a footnote reference as
/// `[^label]`, a footnote definition's text after `[^label]: `, a task-list
/// item after `[ ] ` or `[x] `, and a table's cells a row to a line between
/// `|` separators. A metadata block shows its text, as an HTML block does.
pub(super) fn parse(source: &str, options: Options) -> Document {
    let mut builder = Builder {
        containers: vec![Container::Root(Vec::new())],
        kept: Vec::new(),
        leaf: None,
        leaf_source: None,
        current: 0..0,
        map: MarkdownSourceMap::default(),
        prefix: None,
        links: Vec::new(),
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
                Block::Quote(blocks) => 1 + depth(blocks),
                Block::List { items, .. } => {
                    2 + items.iter().map(|item| depth(item)).max().unwrap_or(0)
                }
                _ => 0,
            })
            .max()
            .unwrap_or(0)
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
            document.links,
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
        assert_eq!(items[0], [Block::Paragraph(plain("one"))]);
        assert!(matches!(&items[1][1], Block::List { start: Some(1), items } if items.len() == 2));
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
                Block::Code(code) => vec![code.clone()],
                Block::Quote(blocks) => texts(blocks),
                Block::List { items, .. } => items.iter().flat_map(|item| texts(item)).collect(),
                Block::Rule => vec!["---".to_owned()],
            })
            .collect()
    }

    #[test]
    fn enabled_extensions_keep_their_source_form() {
        let source = "Maths $a+b$ and $$c$$.\n\n- [x] done\n- [ ] open\n\nNote[^1].\n\n\
                      [^1]: The *note*.\n\n| a | b |\n| - | - |\n| 1 | **2** |\n";
        assert_eq!(
            texts(&parse(source, DOCUMENT).blocks),
            [
                "Maths $a+b$ and $$c$$.",
                "[x] done",
                "[ ] open",
                "Note[^1].",
                "[^1]: The note.",
                "| a | b |\n| 1 | 2 |",
            ]
        );
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
                Event::Start(Tag::BlockQuote(_) | Tag::List(_) | Tag::Item) => depth += 1,
                Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item) => depth -= 1,
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
        // task items' text, which has no tag of its own.
        let (matching, untagged): (Vec<_>, Vec<_>) =
            map.blocks.iter().partition(|block| tagged.contains(block));
        assert_eq!(matching.into_iter().cloned().collect::<Vec<_>>(), tagged);
        assert_eq!(
            untagged
                .iter()
                .map(|block| (block.kind, block.depth, &EXTENDED[block.source.clone()]))
                .collect::<Vec<_>>(),
            [
                (MarkdownBlockKind::Paragraph, 2, "[x] done ✓"),
                (MarkdownBlockKind::Paragraph, 2, "[ ] öffen"),
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
        ] {
            assert!(kinds.contains(&kind), "{kind:?} in {kinds:?}");
        }
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
            document.links
        );
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
                items: vec![vec![Block::Paragraph(plain("a"))], vec![]],
            }]
        );
    }
}
