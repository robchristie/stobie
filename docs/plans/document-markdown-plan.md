# Document Markdown

Status: active
Next action: Deliver increment 5: tables presented as structure when the
options enable them, then the evidence update and closeout.

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
| 2. Heading anchors with application-supplied IDs reached by `reveal` | Complete | `MarkdownSpec::heading_ids`, `UiRole::Heading` nodes and `Frame::reveal`; 2 recipe regressions (heading nodes, names, AccessKit parity and identities at phone and desktop widths and 150% text; identities unchanged where a narrow item flattens) and 3 host regressions (an agent and the application reveal a heading below the fold at phone and desktop widths); `cargo xtask verify` |
| 3. Application-resolved links: external, in-application or unavailable | Complete | `MarkdownSpec::link_kinds`, `MarkdownSourceMap::links` and `PresentationContext::followed_link`; 1 parser regression (link destinations and ranges equal pulldown-cmark's, with CRLF, multibyte text and a base), 1 recipe regression (each kind by pointer, keyboard and AccessKit, with node and AccessKit properties and clean audits at phone and desktop widths; unsafe URLs stay blocked whatever the application says), 4 host regressions (each kind by agent, pointer and keyboard; a follow lapses when its link is not presented) and the `library` example with 2 tests (a relative link opens another document at the heading it names); `cargo xtask verify` |
| 4. Task lists, footnotes and maths presented as more than source text | Complete | Read-only task boxes, numbered footnote references and definitions (`MarkdownSpec::footnote_ids`, `MarkdownSourceMap::footnotes`) and maths as source; 4 new parser regressions and 1 rewritten (structure under every extension, a definition's label whatever opens it, numbering by first appearance without regard to case, a marker outside its paragraph, display maths with CRLF), 5 new recipe regressions (each presentation with node and AccessKit parity and clean audits at phone and desktop widths, 150% text and high contrast; default options unchanged; a reference scrolling its definition into view by pointer and keyboard; a named heading opening a narrow item named by its own text; a followed link reaching the root through scoped, bubble, dialog and code-copy contexts), 1 host regression (an agent follows a reference to a definition below the fold), Gallery `markdown/document` with 1 regression, 2 inspected baselines and the extended browser journey (heading IDs and task states in the ARIA layer, a reference revealing its definition); `cargo xtask verify` |
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
  source form rather than losing their markers. With the default options
  they still do.
- The application gives heading IDs as a list in source-map order
  (`MarkdownSpec::heading_ids`) rather than through a callback: the source
  map already gives each heading's index, text and range, and a list keeps
  `MarkdownSpec` a plain copyable value. An ID is used as the heading's
  semantic ID unchanged, so `stobie reveal ID` takes exactly what the
  application chose; keeping IDs unique is the application's part.
- A named heading is a new `UiRole::Heading` node rather than a `Section`,
  so agents find a document's headings by role and the AccessKit and ARIA
  agreement checks compare it with the heading assistive technology sees.
  Headings without an ID publish no node, as before.
- The application reveals through `Frame::reveal`, the same queue and
  settling an agent's reveal uses, so an in-application link and an agent
  bring a heading into view alike.
- The application classifies links as a list in source-map order
  (`MarkdownSpec::link_kinds`), as it names headings, and the source map
  lists each link's destination and start-tag range for it to classify and
  to compare with its own parse. A link past the end of the list is
  external, so existing callers keep their behaviour.
- `External` stays under the link recipe's URL rules, and `InApplication`
  is only reported, never opened, so no classification can make Stobie open
  an unsafe URL.
- A followed link is read from `PresentationContext::followed_link` after
  presenting rather than returned, so `markdown` keeps returning a
  `Response` and no caller changes; the low-level function returns it in
  `MarkdownResponse::followed`.
- Agents follow an in-application link through the existing
  `stobie.open-link` capability: the host asks the link's next presentation
  to follow it, as a press would, the route copy controls use. A request the
  pass does not take is withdrawn, so a link that is no longer presented is
  never followed later.
- A task box is a read-only AccessKit check box and a `UiRole::Checkbox`
  node that is not enabled: the document, not the reader, owns its state,
  so nothing toggles it. It is named by its item's first paragraph or
  heading, which assistive technology would otherwise read separately.
- Footnotes are numbered in order of first appearance, as GitHub and most
  renderers number them, and a reference names its number (`[n]`), the
  text a reader sees. A reference scrolls its definition into view inside
  Stobie and is not reported as followed: the definition is in the same
  document, so the application has nothing to decide. Footnote IDs, like
  heading IDs, are a list in source-map order and only add a node an agent
  or the application can reveal.
- Maths keeps its TeX source without the `$` delimiters, which only mark
  where it starts and ends, in the italic monospace face: monospace says
  "source", and italics tell it apart from inline code. Typesetting stays
  out of scope.

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

## Increment 2

`MarkdownSpec::heading_ids` (none by default) gives the application's ID
for each heading in source-map order. A heading with an ID takes its
presentation identity from the ID rather than its place, carries the ID as
its AccessKit author ID, reports its place to a reveal on every pass, and
while in view is published as a `UiRole::Heading` node named by its
presented text (`MarkdownResponse::heading_nodes`, published by both scoped
Markdown calls). `Frame::reveal` lets the application reveal a node through
the host's reveal queue. A list item too narrow to indent that opens with
something other than text now keys its marker's line apart from its blocks,
so their identities no longer shift at narrow widths. `MarkdownBlock`,
`MarkdownHeading` and `MarkdownSourceMap` are `#[non_exhaustive]`, as
increment 1's review asked, before anything is added to them. Host
regressions reveal headings below the fold of a visible-only scroll area by
`stobie.reveal` and by `Frame::reveal` at 390 and 1280 points, including
headings inside a quote and a list item, and an unknown ID is reported as
not presented. The CLI's outline names the new role.

## Increment 3

`markdown_source_map` lists every link (`MarkdownLink`) with its destination
as pulldown-cmark reports it (`mailto:` before an email autolink's address)
and its start tag's source range plus the caller's base, the carried item
for a consumer's conformance test. `MarkdownSpec::link_kinds` (none by
default) gives each link a `MarkdownLinkKind`: `External` (the default and
the existing behaviour), `InApplication` or `Unavailable(reason)`. An
in-application link is presented, focused and published exactly as an
external one, without `link_description`; every activation reports it as a
`FollowedMarkdownLink` (index and destination) through
`MarkdownResponse::followed` and `PresentationContext::followed_link`, and
the scoped calls, the chat message bubble and dialogs carry it to their
parent. An unavailable link keeps its text plain, takes no Tab stop and is
published with the application's reason. The host routes
`stobie.open-link` on an in-application link to its next presentation and
withdraws an untaken request after the pass. The `library` example follows
relative links between two synthetic documents and reveals the heading a
fragment names. External links behave as before: every existing link
regression passes unchanged.

## Increment 4

Design contract. Task: read a knowledge document's open tasks, follow a
footnote to its note and back to the text, and read a formula as written,
at phone and desktop widths, at 150% text and in high contrast. Hierarchy:
the text leads; a task's state reads at a glance from its box without
competing with headings; a footnote reference is a small raised link that
does not interrupt the line; definitions sit apart at the end, quieter than
the body; maths is visibly source, distinct from prose and from inline code
without looking like a control. Reference: the existing `markdown/digest`
recipe (lists, link colour, code in `MonospaceTechnical`) and GitHub's
rendering of task lists and footnotes. States: ordinary (dark comfortable),
narrow light high contrast at 150% with the definitions below the fold,
keyboard focus on a reference, a quoted (muted) task, a task whose marker
pulldown-cmark reports outside its paragraph, and a footnote definition
opening with a list. Excluded: toggling a task (the document owns it),
typeset maths, back-links from a definition to its references, and loading,
empty and error states (application-owned).

The parser now models task items, footnote definitions and display maths as
blocks and footnote references and inline maths as styled runs. A task
marker belongs to its item wherever pulldown-cmark reports it, including
before the item's first paragraph or alone before a blank line; a marker no
kept item holds keeps its source text. Definitions keep their label and
number whatever block opens them, and display maths normalises CRLF and CR
to LF. A reference to a definition that is not presented is an unavailable
link. `MarkdownResponse` gained `task_nodes` and `footnote_nodes`, published
by both scoped calls, the message bubble's and dialogs' Markdown and the
code-copy path. Carried review items: a named heading opening a list item
too narrow to indent is named by its own text (its node's name and its
AccessKit heading's value; the label keeps the whole line, marker included); `MarkdownLinkKind` and `MarkdownResponse` are
`#[non_exhaustive]`; an unavailable control with no description is described
"Unavailable: reason" rather than "; unavailable: reason", through one
helper the audit, links, actions, switches and rows share. Existing callers
keep their behaviour: with the default options every earlier Markdown
regression passes, and adding `footnote_ids` (defaulted) changes no caller
that uses `..MarkdownSpec::default()`; every literal in the repository was
updated.

Task and reference: the contract above; Gallery `markdown/document` in dark
comfortable mode and narrow light high contrast at 150%, beside
`markdown/digest`. Behavioural checks: passed (recipe, host and Gallery
regressions; the browser journey). Presentation checks: passed. First
critique, before reading the rationale: the headings, then the filled box of
the done task, are noticed first; open boxes read as open in both
appearances without pulling focus from the text; references are small,
raised and in link colour, and do not break the line; the definitions sit
apart after the rule, quieter than the body, numbers aligned in their
gutter; maths reads as source and differs from both prose and inline code.
What competes: inline maths in the monospace face sets wider than the prose
around it. What became harder: the second item of the list opening a
definition sat outside the definition, unmuted, at the body's indent; and
the task whose marker stands alone shows a box with no text beside it.
Reading the rationale answered the first (maths is source, and monospace is
what says so) and the third (pulldown-cmark ends that item at the blank
line, so the text that follows is not the task's; the box is named "Task").
The second was the story's source, whose Rust string continuation stripped
the item's indentation; with the indentation kept, re-critique found the
whole list inside the definition, muted and in its gutter. No presentation
change was needed.

Design decision: accepted. Remaining trade-offs: a definition has no link
back to its references; a lone task marker's box has no text of its own;
display maths wraps mid-token in a narrow column, as code does.
