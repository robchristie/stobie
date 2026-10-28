# Document Markdown

Status: complete
Delivery: [PR #151](https://github.com/robchristie/stobie/pull/151)

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
| 5. Tables presented as structure | Complete | A grid with a header row, column alignment and wrapping cells that stacks as records when too narrow, with AccessKit table, row, column header and cell nodes and ARIA counts and indices; 2 new parser regressions and 1 extended (alignments, header, padded and truncated rows, inline cells, a table in a list item and one with no body rows), 6 new recipe regressions and 1 extended (grid or stacked at phone and desktop widths, 150% text and high contrast with clean audits; a cell wrapping in its column; tables in a quote and a list item and one with no body rows; the AccessKit table; default options unchanged; drag-copying a table's cells), 1 ARIA regression, Gallery `markdown/document` with a wide table, its extended regression, 2 re-inspected baselines and the extended browser journey (the table as a grid and stacked in the ARIA layer); `cargo xtask verify` |
| 6. Polish carried from reviews | Complete | Each item carried from the increment 4 and 5 reviews fixed or decided (see Increment 6); 2 new parser regressions (footnote labels match under pulldown-cmark's Unicode case folding; display maths as a link's text keeps the link), 5 new recipe regressions and 2 extended (a stacked table counts only its records and presents a linked header row once, by AccessKit and ARIA, at phone and desktop widths and 150% text; its opening rule sits a section below a heading; a quoted table's header row shows its raised surface; an open task box keeps 3:1 against every surface in every theme; a table copies stacked, wrapped and at 150%; a narrow item's heading labelled by its own text; task names without footnote numbers), the Gallery regression, browser journey and 2 baselines updated for the new task name, outline and spacing; `cargo xtask verify` |
| 7. Evidence record and closeout | Complete | The document Markdown section of `docs/markdown-evidence.md`: 16 captures of the Gallery's `markdown/document` at phone and desktop widths, 100% and 150% text and both high-contrast themes (the seven-column table as a grid and stacked, footnote definitions revealed by a click in the phone column), the open task box's outline measured from captured pixels in all four themes, and the Markdown smoke's output, archived in stobie-evidence `7fc8cf9`; two decisions amended and the components guide's Markdown bullets reflowed (see Increment 7); `cargo xtask verify` |

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
- Enabled extensions that Stobie does not present as structure keep their
  source form rather than losing their markers. With the default options
  tables, task lists, footnotes and maths still do.
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
- Tables use Stobie's own `Alignment` (start, centre, end) rather than
  pulldown-cmark's, which has no `Eq` and names left and right rather than
  start and end; a column with no marker starts its cells.
- A table is a grid while every column fits its least width: its longest
  word, capped at twice the minimum content width so one unbroken token
  cannot force the whole table into records, plus the cell padding. Spare
  width is shared in proportion to how much more each column would take
  unwrapped. Otherwise each body row (or the header row of a table with
  none) is stacked as a record of full-width lines, each led by its column's
  header in strong text and ": ", as phone layouts commonly present tables;
  empty cells and records are left out. A horizontally scrolling table
  would hide columns a reader cannot tell are there. Amended in increment
  6: the header row is also a record of its own, first, where it holds a
  link or footnote reference (see below).
- A cell aligns its whole galley within the column, so a wrapped cell's
  lines share one edge; the cell's selectable area is the galley's extent.
- The grid's vertical padding is the spacing unit so that egui copies a
  table's rows as lines whose cells are separated by spaces, with no blank
  line between rows. Amended in increment 6: egui separates copied text by
  its distance from the text before it, so a row whose last cell is shorter
  than another of its cells is followed by a blank line, as the components
  guide records.
- Table, row, column header and cell nodes come from nested child `Ui`s,
  whose AccessKit nodes take the table roles, counts and zero-based indices
  (ARIA's are one-based). A table publishes no `UiNode`: agents read it
  through its cells' text, and no new role is needed until one acts on a
  table. A cell's identity is keyed by its source row and column, so it is
  the same in the grid and the stack.
- The source map keeps a table as one block; spans within it are gap S5's.
- Footnote labels match as pulldown-cmark matches them, through the same
  `unicase` comparison (Unicode case folding), already in the tree as its
  dependency and now a direct one: lower-casing alone numbered `[^Straße]`
  and `[^STRASSE]` apart where pulldown-cmark joins them.
- A stacked table presents its header row as a record of its own, first,
  where a header cell holds a link or footnote reference. The leads repeat
  the header's text as plain text; repeating its links in every record would
  publish one link many times under one identity. A header without links
  stays out of the stack, as before.
- A task's name is the words of its first paragraph or heading without
  footnote references' numbers: each reference is a link of its own, and
  "[2]" read as part of a task's name is noise.
- An open task box is outlined in the muted text colour, as the switch's
  off outline is: the control border is below 3:1 against the document
  surfaces in the dark and light themes, and a box's outline is the only
  sign of its state.
- A wrapped cell's lines keep one alignment edge (accepted trade-off, as
  increment 5 recorded). egui aligns a galley's rows about its anchor rather
  than within an allocated rectangle, and the reading-text measurement, text
  audit, selection and link areas all place a galley by its left edge;
  aligning each line would give cells an anchor of their own throughout.
  End alignment serves numeric columns, which rarely wrap.

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

## Increment 5

Design contract. Task: read a knowledge document's tables, compare values
down a column and along a row, and copy them, at phone and desktop widths,
at 150% text and in high contrast, including a table wider than a phone.
Hierarchy: headings lead; the header row reads as the table's labels
without competing with headings; values align so a column scans; rules
separate rows without boxing every cell. Reference: GitHub's table
rendering and the existing `markdown/document` recipe (raised surface for
quoted and code blocks, decorative border for rules). States: ordinary
(dark comfortable, the table as a grid), narrow light high contrast at 150%
(the same table stacked), a quoted table, a table in a list item, a table
with no body rows, a cell wrapping in its column, an empty cell and a link
in a cell. Excluded: sorting, resizing or scrolling columns, cell selection
as a unit, and per-cell source spans (gap S5).

The parser now models a table as a block of column alignments, a header
row and body rows of inline cells (styles, links and escaped pipes kept),
padding short rows and dropping cells past the header's count. The recipe
presents it as a grid or, when its columns cannot fit, as stacked records,
as the decisions describe. Text in cells selects and copies as other
Markdown text does: dragging across the grid copies one line for each row.
AccessKit and the ARIA projection carry the table, its rows, column headers
and cells with counts and indices; the ARIA projection gained the cell and
header roles. With the default options a table is still a paragraph of its
rows' source, and no signature changed.

Task and reference: the contract above; Gallery `markdown/document`, whose
document gained a seven-column counts table, in dark comfortable mode and in
narrow light high contrast at 150%. Behavioural checks: passed (parser,
recipe, ARIA and Gallery regressions; the browser journey). Presentation
checks: passed. First critique, before reading the rationale: the headings
are noticed first, then the raised header row; numbers align on their right
edge and the centred tide column reads as a column; the empty tide in row D
is a visible gap rather than a missing cell; rules are light and the table
does not look like a control. In the narrow column each record reads as
"Pool: A" with strong labels that are body size and so do not compete with
headings. What competes: nothing in the grid. What became harder: in the
stack a reader can no longer compare one column down the rows, and the
records are long enough to push the formula below the fold. Reading the
rationale answered both: a grid that does not fit would hide columns or
break every word, and the records keep each value beside its label. No
presentation change was needed.

Design decision: accepted. Remaining trade-offs: a stacked table loses
comparison down a column; a wrapped cell's lines share one alignment edge
rather than aligning each line; the source map does not locate cells.

## Increment 6

Each item the increment 4 and 5 reviews carried, and its resolution:

- Footnote labels folded with `to_lowercase` where pulldown-cmark uses
  Unicode case folding: fixed, with `unicase` (see Decisions); a parser
  regression compares the references pulldown-cmark reports with Stobie's
  numbering for "ß", "SS" and the long s.
- The narrow baseline does not show the footnote definitions: decided. The
  baseline keeps the document's opening, its tasks, heading anchors and
  stacked table, which change with the width; the definitions at phone width,
  150% text and high contrast are covered by the recipe regression, a
  reference revealing its definition by the host regression and the browser
  journey, and increment 7's evidence record captures a revealed definition.
- A named heading opening a list item too narrow to indent kept the marker
  in its AccessKit name: its text runs hold "• ", from which assistive
  technology names a heading without a label. Fixed: its label and value
  are its own words; the recipe regression checks both and the runs.
- A task's name included a footnote reference's "[2]": stripped (see
  Decisions); the recipe, Gallery and browser regressions name tasks
  without it.
- Display maths that is a link's whole text dropped the link: fixed, it
  stays inline in the link; a parser regression covers it alone and after
  display maths.
- The open task box's outline contrast was unmeasured: measured from the
  tokens, the control border it used was 2.43:1 against the canvas, 2.25:1
  against the panel and 2.03:1 against the raised surface in the dark theme
  (2.77, 3.03 and 2.55 in the light theme), below WCAG 1.4.11's 3:1. It is
  now the muted text colour, 8.10, 7.51 and 6.76 in the dark theme and at
  least 5.37 in every theme; a recipe regression checks the painted outline
  at 3:1 against each surface in all four themes.
- A stacked table with an all-empty body row counted the record it left
  out: fixed, a stacked table's row count and indices count only the records
  presented, by AccessKit and in the ARIA projection.
- A link or footnote reference in a header cell could not be reached in a
  stacked table: fixed, the header row is then a record of its own (see
  Decisions), its links presented once and operable.
- The header row's raised surface might not show inside a quote: checked,
  no change. A quote paints only its bar, and a message bubble swaps its
  surfaces so that the raised surface still differs from its ground; a
  recipe regression finds the header fill behind a quoted table's header
  row, inside the quote, not covered.
- A stacked table's opening rule directly under a heading read as the
  heading's underline: fixed, a stacked table under a heading is set a
  section below it, as under a paragraph; a grid keeps the block spacing.
- Copying was proved only for the grid at 100%: a regression copies a grid
  at 100% and 150%, a stacked table (each record's lines, a blank line
  between records) and a grid with a wrapped cell (its text unbroken). egui
  separates copied text by its distance from the text copied before, so a
  row whose last cell is shorter than another of its cells is followed by a
  blank line; recorded in the components guide rather than reordering
  cells, which would change their order for assistive technology.
- A wrapped cell's lines share one alignment edge: kept as a trade-off (see
  Decisions).

The Gallery's `markdown/document` baselines were re-rendered and inspected
for the open task's outline, the task's name without "[2]" and, in the
narrow fixture, the stacked table's space under its heading.

## Increment 7

The evidence record is the "Document Markdown" section of
[`docs/markdown-evidence.md`](../markdown-evidence.md), with its raw
captures in stobie-evidence
[`7fc8cf9`](https://github.com/robchristie/stobie-evidence/tree/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown)
and its row in the [evidence archive](../evidence-archive.md). The items the
increment 6 review carried:

- Two decisions predated increment 6 and are amended in place: a stacked
  table also presents a header row holding a link as a record, and a copied
  grid row whose last cell is shorter than another is followed by a blank
  line.
- The components guide's Footnotes and Tables bullets are reflowed: an
  overlong line and an unindented continuation.
- The stacked table's opening rule, measured in the phone-width captures,
  sits 16 pixels below the heading's glyphs and 9 above the first record's
  at 100%, and 18 and 10 at 150%: it reads as the table's rule, not the
  heading's underline.
- The open task box's outline, measured from captured pixels at device
  scale 1 and 2, is 7.51:1 against the panel in the dark theme and at least
  6.39:1 in every theme, the muted text and panel tokens exactly.
- Footnote definitions in the phone column are captured after a click on a
  reference at 100% and at 150% in both high-contrast themes.

## Closeout

Increments 1 to 6 landed as PRs
[#143](https://github.com/robchristie/stobie/pull/143),
[#144](https://github.com/robchristie/stobie/pull/144),
[#145](https://github.com/robchristie/stobie/pull/145),
[#147](https://github.com/robchristie/stobie/pull/147),
[#148](https://github.com/robchristie/stobie/pull/148) and
[#149](https://github.com/robchristie/stobie/pull/149); increment 7 is this
plan's delivery. Later work stays outside this plan, as its boundaries
excluded it: mapping or highlighting a source span within presented text,
laying out only what is near view in very long documents, and typesetting
maths.
