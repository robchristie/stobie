# Document Markdown

Status: active
Next action: Deliver increment 2, heading anchors with application-supplied
IDs that `stobie reveal` and the application can bring into view.

## Outcome and boundaries

A document reader, such as one for a knowledge repository, presents long
Markdown documents through the Markdown recipe. It parses the same source
itself, so it needs Stobie's parse to agree with its own and to say where
each presented block came from; it needs headings it can name and bring into
view; it decides what its own links mean; and it uses tables, task lists,
footnotes and maths. The application supplies every identifier and link
classification: Stobie learns no slug rule, link resolution policy or other
application meaning. Existing callers keep their behaviour by default.
Excluded: mapping or highlighting a source span within presented text,
laying out only what is near view in very long documents, and typesetting
maths.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| 1. Parser options and source offsets | Complete | `MarkdownSpec::options` (default strikethrough only) and `markdown_source_map`; 4 new parser regressions and 1 extended (offsets equal pulldown-cmark's for every document extension with CRLF and multibyte text, a caller-supplied base after BOM and frontmatter, extensions keeping their source form, raw HTML text and bounded nesting under every option set) and 2 recipe regressions; `cargo xtask verify` |
| 2. Heading anchors with application-supplied IDs reached by `reveal` | Pending | |
| 3. Application-resolved links: external, in-application or unavailable | Pending | |
| 4. Task lists, footnotes and maths presented as more than source text | Pending | |
| 5. Tables presented as structure; evidence update and closeout | Pending | |

## Decisions

- The caller passes pulldown-cmark's own `Options`, re-exported as
  `MarkdownOptions`, so an application parsing with the same pulldown-cmark
  release passes the very value it parses with. A Stobie option type would
  have to be translated and kept in step.
- Source offsets are a query, `markdown_source_map`, rather than part of
  `MarkdownResponse`: an application needs them without presenting, and the
  scoped `PresentationContext::markdown` returns only a `Response`. The query
  and the presentation share one parse, so they list the same blocks.
- A block's range is pulldown-cmark's start-tag range, so an application can
  compare it with its own parse directly. The caller's base shifts every
  range; Stobie does not look for a byte order mark or frontmatter.
- Until increments 4 and 5 present them, enabled extensions keep their
  source form rather than losing their markers.

## Increment 1

`MarkdownSpec` gained `options` (`DEFAULT_MARKDOWN_OPTIONS`, strikethrough
alone, by default; every caller in the repository uses `..Default`). The
parser reads pulldown-cmark's offset iterator and records each presented
block's kind (`MarkdownBlockKind`), nesting depth and source range, and each
heading's level, text, source range and block index. Inline and display
maths, footnote references and definitions, task markers and table rows keep
their source form; definition-list parts and metadata blocks stay text.
Range regressions compare with pulldown-cmark directly for a fixture using
tables, footnotes, strikethrough, task lists and maths with CRLF line endings
and multibyte text, and map a body after a byte order mark and CRLF
frontmatter into its file through the caller's base. A recipe regression
presents the fixture at phone and desktop widths at 100% and 150% text with
clean text and AccessKit audits, one reading surface for each text block the
map reports. The default presentation is unchanged: every existing Markdown
regression passes as before.
