# Markdown evidence

## Experience and design decision

Task and reference: read a model's Markdown digest (a heading, numbered
linked titles each with a one-line note, a short list, a quote and a
command) at phone and desktop widths, open a title in a new tab without
leaving the conversation, and copy an item. Compare with the
[reading-text recipe](reading-text-evidence.md), whose measured path the
blocks share, and with the conventions of chat clients that render Markdown:
prose leads, headings are modest, list numbers align, links are secondary
but recognisable, code is set apart.

Behavioural checks: passed. Recipe regressions cover every element at phone
(343 points) and desktop (960 points) widths at 100% and 150% text (roles,
heading levels in AccessKit, measured numbers in one column, nested
indentation, clean text and parity audits); a ten-item digest with every link
named by its title; every prefix of the digest and 25 malformed sources
(unclosed emphasis, links and fences, control characters, CRLF, mixed scripts,
zero-width characters, tabs, empty labels and URLs, a table) presenting
without a panic or audit finding; completed blocks keeping their exact
allocation at every line of a streaming reply; nesting 40 lists or quotes
deep at 200 points without text leaving its column, with flattened items led
by their marker; unsafe schemes (`javascript:`, `data:`, `vbscript:`,
`file:`, relative and empty) published as unavailable links with their
reason, never opening by pointer, middle click or keyboard and never failing
the text audit; a link opening a new tab by click, middle click, Tab then
Enter or Space, and AccessKit, while clicks beside it open nothing; a link
wrapped over several lines opening from each line as one Tab stop; disabled
and inert presentations; a drag across a paragraph copying it with the link's
label; a link after shaped clusters (combining marks, a keycap, ZWJ emoji) and with non-ASCII text in its label keeping its label and the same hit-area width, opening from its first and last glyphs; hover keeping the layout; a scroll area publishing only visible blocks
and links; identities stable across widths and appearances; and the parser's
block model, raw HTML kept as text and hostile nesting bounded. A host
regression publishes Markdown links as `stobie.open-link`, opens one in a new
tab when an agent invokes it and refuses the relative one as unavailable.
Gallery regressions check both stories' audits and links.

Presentation checks: passed. First critique of the ordinary digest and the
narrow high-contrast column at 150%, before reading the rationale: the
heading, then the numbered titles, are noticed first; the numbers align and
wrapped notes hang under their titles; links are recognisable by colour and
underline without competing with the heading; the quote recedes in muted text
behind its bar; the unbroken identifier and the long code line wrap inside the
column; unsafe links read as plain words; raw HTML is visibly source. What
competes: code in the monospace face looks larger than the prose at the same
point size. What became harder: the keyboard focus ring sat on the link's
first glyph and the dash after it. Reading the rationale answered the first:
code uses the `MonospaceTechnical` role at the body size, as everywhere else
in Stobie, and a smaller code size is a typography-token decision, not this
recipe's. It did not answer the second, so one refinement gives the ring half
a spacing unit of clearance either side, inside the spaces around the link.
Re-critique accepted it: the ring now clears the glyphs and sits just short of
the following dash. Remaining trade-offs: a selection copy omits list numbers
and bullets, which are not part of the text; list items are always spaced
tightly, so a "loose" list's paragraphs sit closer than CommonMark's HTML
would place them, in exchange for no reflow while streaming; an unfinished
`**` or link shows its punctuation until it closes; a screen reader hears a
link's text once in its paragraph and again as the link.

Design decision: accepted after that one refinement.

## Rendered and physical evidence

- [Digest, dark comfortable](../tests/ui-snapshots/expected/markdown-digest-dark/visual.png).
- [Narrow light high contrast at 150%](../tests/ui-snapshots/expected/markdown-long-high-contrast/visual.png).

`tools/markdown-browser-smoke.sh` runs in canonical verification on WebGPU
and again on WebGL2. In Chromium it reads the digest's observations (each
block reading text, four list numbers, five links with their URLs), finds
each link and both headings with their levels in the ARIA layer, drags across
the first item and reads the actual clipboard, which holds the item exactly
with its link's label, and confirms the drag opened nothing. A physical click
on a link inside the text, and Enter and Space after focusing a link through
the ARIA layer, each open a new tab showing its URL, with no `window.opener`
and no referrer, while the Gallery page keeps its URL and story; example.org
is answered locally and every other request is refused, so nothing leaves the
host. In the narrow column it checks the wrapped code, clicks both unsafe
links and sees no tab open, scrolls to the reply still arriving (whose
finished link is a link and whose unfinished one is text), clicks a link in a
disabled region without effect, and confirms the idle page presents no
frames. Screenshots and workflow JSON, with source revision, WASM hash and
settled idle proof, are written under ignored `.tools/runtime/`; the owning
pull request retains the results.

## Size

pulldown-cmark 0.13.4 (MIT) and its new dependency unicase 2.10.0 (MIT or
Apache-2.0), without the HTML renderer or command-line features, are the
only additions to the lockfiles. Measured against `c6527963`, before the
per-row actions landed, the Gallery's release WASM after wasm-bindgen grew
from 11,453,072 to 11,725,743 bytes (+272,671; 4,062,514 to 4,161,332 gzip,
+98,818; 2,873,477 to 2,947,504 Brotli, +74,027). After
`wasm-opt -Oz` it grew from 9,062,661 to 9,297,496 bytes (+234,835, +2.6%;
Brotli +71,098, +2.6%). Both figures include the recipe and the two Gallery
stories as well as the parser.

## Baseline decision and qualification

All 21 fixture text audits passed. The 19 existing fixtures keep identical
metadata, semantics and measured observations; the native-control count
rises from 40 to 42 for the two new catalogue entries, and one pixel each of
`navigation-long-high-contrast` and `status-chip-treatments-light` moves by
one colour step as the glyph atlas grows. Both new fixtures' visual, semantic and text artefacts
were inspected outside `expected/` and accepted explicitly. Frozen evaluation
tasks, fixture definitions and scoring are unchanged. Verification never
updates baselines.

## Platform limits

Browser proof uses Chromium with WebGPU/SwiftShader and WebGL2. There is no
native physical journey for Markdown: its links open through the link
recipe's `OpenUrl` route, which the reading-text native smoke already proves
reaches the platform's default browser. This adds no screen-reader
qualification: headings, links and AccessKit parity are semantic and keyboard
evidence only.

## Document Markdown

The [document Markdown plan](plans/document-markdown-plan.md) extended the
recipe for long knowledge documents: caller-chosen parser options and a
source map, heading anchors, application-resolved links, task lists,
footnotes, maths and tables. This section is its evidence record; the plan
lists each increment's regressions and decisions.

### Experience and design decision

Task and reference: read a knowledge document's open tasks, follow a
footnote to its note, read a formula as written, and read, compare and copy
a table, at phone and desktop widths, at 150% text and in high contrast,
including a table wider than a phone. References: the `markdown/digest`
recipe above and GitHub's rendering of task lists, footnotes and tables.
The Gallery story `markdown/document` holds a synthetic survey document
with every extension and a seven-column table.

Behavioural checks: passed. Parser regressions compare source ranges, link
destinations and footnote numbering with pulldown-cmark's own reports
(CRLF, multibyte text, a caller's base after frontmatter); recipe
regressions present every extension at phone and desktop widths, 150% text
and high contrast with clean text and AccessKit audits; host regressions
reveal headings below the fold by agent and by application request, follow
a footnote reference to its definition below the fold by agent, and follow
links of each kind by agent, pointer and keyboard; the `library` example
resolves a relative link to another document's heading. With the default
options every earlier Markdown regression passes unchanged.

Presentation checks: passed. The first critiques, before reading the
rationale, and their resolutions are recorded in the plan's increments 4
and 5: headings lead, then the done task's filled box and the table's
raised header row; references are small raised links; definitions sit
apart, muted, after a short rule; maths is source in the italic monospace
face; numbers align on their end edge and the stacked records keep each
value beside its label. Increment 6 resolved the twelve items carried from
those reviews.

Design decision: accepted. Remaining trade-offs: a definition has no link
back to its references; a lone task marker's box has no text of its own;
display maths wraps mid-token in a narrow column; a stacked table loses
comparison down a column; a wrapped cell's lines share one alignment edge;
the source map does not locate cells or spans within a block (gap S5).

### Rendered and physical evidence

Reviewed baselines, re-inspected in increment 6:

- [Document, dark comfortable](../tests/ui-snapshots/expected/markdown-document-dark/visual.png).
- [Narrow light high contrast at 150%](../tests/ui-snapshots/expected/markdown-document-narrow-high-contrast-150/visual.png).

Captures archived in stobie-evidence
[`7fc8cf9`, `docs/markdown-evidence/document-markdown/`](https://github.com/robchristie/stobie-evidence/tree/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown),
from Stobie `51c521b` (the tree increment 7 changes only in documentation)
in Chromium 151 on WebGPU, with each image's observations in
[`capture.json`](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/capture.json):

- Tables at desktop width (960 points): a grid at
  [100%, dark](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-wide-dark-opening.png),
  [150%, light high contrast](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-wide-light-high-contrast-150-table.png)
  and [150%, dark high contrast](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-wide-dark-high-contrast-150-table.png),
  the seven columns at their natural widths, numbers on their end edge and
  the empty cell a visible gap.
- Tables at phone width (320 points): stacked as records at
  [100%, dark](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-dark-table.png) and at 150% in
  [light high contrast](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-light-high-contrast-150-table.png)
  ([end](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-light-high-contrast-150-table-end.png))
  and [dark high contrast](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-dark-high-contrast-150-table.png).
  Measured from the pixels, the opening rule sits 16 pixels below the Counts
  heading's glyphs and 9 above the first record's at 100% (dark), and 18
  and 10 at 150% (light high contrast): it groups with the records it
  opens rather than underlining the heading.
- Footnote definitions in the phone column after a physical click on
  reference `[1]`: [100%, dark](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-dark-footnote-revealed.png),
  [150%, light high contrast](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-light-high-contrast-150-footnote-revealed.png)
  and [150%, dark high contrast](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/document-narrow-dark-high-contrast-150-footnote-revealed.png),
  each definition muted with its number in the gutter and the second's list
  inside it.
- The open task box's outline measured from captured pixels against the
  panel surface inside it, at device scale 1 and 2 with the same result
  ([dark, 1x](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/capture/task-box-dark-standard-1x.png)):
  7.51:1 in the dark theme (`#9daab3` on `#13181d`), 14.19:1 in dark high
  contrast, 6.39:1 in the light theme and 11.48:1 in light high contrast,
  each the theme's muted text and panel tokens exactly, above WCAG 1.4.11's
  3:1. The recipe regression checks the same outline against the canvas,
  panel and raised surfaces.

`tools/markdown-browser-smoke.sh` presents the document in canonical
verification on WebGPU and again on WebGL2: at 640 points dark and in the
narrow light high-contrast column at 150%, it finds each heading in the ARIA
layer with its level and the application's ID, each task as a read-only
checkbox with its state, and the table as an ARIA table (seven column
headers and five rows as a grid; four records, without column headers,
stacked); it checks maths in the technical face, and a physical click on
reference `[1]` reveals the definition and opens nothing. Its output at
`51c521b` is archived beside the captures
([workflow](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/smoke/browser-workflow.json),
[revealed definition](https://github.com/robchristie/stobie-evidence/blob/7fc8cf97b32a510e69451be25c09e8cde4c2036d/docs/markdown-evidence/document-markdown/smoke/browser-document-footnote-revealed.png)).

### Size and platform limits

No lockfile gained a package: `unicase`, already locked as pulldown-cmark's
dependency, became a direct one. Browser proof uses Chromium with
WebGPU/SwiftShader and WebGL2. This adds no screen-reader qualification:
headings, tasks, tables and footnotes are semantic, ARIA and keyboard
evidence only. Maths is never typeset.
