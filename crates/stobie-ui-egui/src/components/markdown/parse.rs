//! CommonMark source to Stobie's block model, through pulldown-cmark.
//!
//! The model keeps only what the recipe presents: paragraphs, headings,
//! lists, block quotes, code blocks and rules, with inline emphasis, strong,
//! strikethrough, code and links. Everything else keeps its text: raw HTML is
//! shown as written and never interpreted, an image shows its alternative
//! text, and tables (not enabled) stay paragraphs of their source.

use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};

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

struct Builder {
    containers: Vec<Container>,
    /// For each open list, quote or item, whether it was kept as a container.
    kept: Vec<bool>,
    leaf: Option<Leaf>,
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

    fn close_leaf(&mut self) {
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
                Block::Code(text.replace('\t', "    "))
            }
        };
        self.blocks().push(block);
    }

    /// The inline text is appended to, opening a paragraph where text
    /// arrives without one, as in a tight list item.
    fn inline(&mut self) -> &mut Inline {
        if !matches!(self.leaf, Some(Leaf::Paragraph(_) | Leaf::Heading(..))) {
            self.close_leaf();
            self.leaf = Some(Leaf::Paragraph(Inline::default()));
        }
        match self.leaf.as_mut() {
            Some(Leaf::Paragraph(inline) | Leaf::Heading(_, inline)) => inline,
            _ => unreachable!("an inline leaf was just opened"),
        }
    }

    fn text(&mut self, text: &str, code: bool) {
        if let Some(Leaf::Code(code_text)) = self.leaf.as_mut() {
            code_text.push_str(text);
            return;
        }
        let style = self.style(code);
        let link = self.link;
        self.inline().push(text, style, link);
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

    fn event(&mut self, event: Event<'_>) {
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
                self.blocks().push(Block::Rule);
            }
            // Extensions that are not enabled keep their source text.
            Event::InlineMath(text) | Event::DisplayMath(text) | Event::FootnoteReference(text) => {
                self.text(&text, false)
            }
            Event::TaskListMarker(done) => self.text(if done { "[x] " } else { "[ ] " }, false),
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.close_leaf();
                self.leaf = Some(Leaf::Paragraph(Inline::default()));
            }
            Tag::Heading { level, .. } => {
                self.close_leaf();
                self.leaf = Some(Leaf::Heading(level as u8, Inline::default()));
            }
            Tag::CodeBlock(_) => {
                self.close_leaf();
                self.leaf = Some(Leaf::Code(String::new()));
            }
            Tag::HtmlBlock => {
                self.close_leaf();
                self.leaf = Some(Leaf::Paragraph(Inline::default()));
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
                self.links.push(url);
                self.link = Some(self.links.len() - 1);
            }
            // An image shows its alternative text, which arrives as text.
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::HtmlBlock => {
                if let Some(Leaf::Paragraph(inline)) = self.leaf.as_mut()
                    && tag == TagEnd::HtmlBlock
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
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strikethrough = self.strikethrough.saturating_sub(1),
            TagEnd::Link => self.link = None,
            _ => {}
        }
    }
}

/// Parse CommonMark with strikethrough. Any source, including an incomplete
/// one still arriving, parses without panicking.
pub(super) fn parse(source: &str) -> Document {
    let mut builder = Builder {
        containers: vec![Container::Root(Vec::new())],
        kept: Vec::new(),
        leaf: None,
        links: Vec::new(),
        strong: 0,
        emphasis: 0,
        strikethrough: 0,
        link: None,
    };
    for event in Parser::new_ext(source, Options::ENABLE_STRIKETHROUGH) {
        builder.event(event);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let document = parse(
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
            Block::Code("fn main() {\n    println!();\n}".to_owned())
        );
        assert_eq!(blocks[8], Block::Code("indented code".to_owned()));
        assert_eq!(blocks[9], Block::Rule);
        assert_eq!(blocks[10], Block::Paragraph(plain("line\nbreak")));
        assert_eq!(blocks.len(), 11);
    }

    #[test]
    fn raw_html_images_and_tables_keep_their_text() {
        let document = parse(
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

    #[test]
    fn hostile_nesting_is_bounded() {
        for source in [
            ">".repeat(100_000),
            "- ".repeat(50_000),
            (0..2_000)
                .map(|level| format!("{}- item\n", "  ".repeat(level)))
                .collect(),
            "[".repeat(50_000) + &"]".repeat(50_000),
            "*".repeat(100_000),
        ] {
            let document = parse(&source);
            assert!(
                depth(&document.blocks) <= MAX_NESTING,
                "{}",
                depth(&document.blocks)
            );
        }
    }

    #[test]
    fn empty_blocks_are_dropped_and_items_keep_a_place() {
        assert!(parse("").blocks.is_empty());
        assert!(parse("#").blocks.is_empty());
        assert!(parse("   \n\n  ").blocks.is_empty());
        assert_eq!(
            parse("- a\n-").blocks,
            [Block::List {
                start: None,
                items: vec![vec![Block::Paragraph(plain("a"))], vec![]],
            }]
        );
    }
}
