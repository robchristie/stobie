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
label; hover keeping the layout; a scroll area publishing only visible blocks
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
only additions to the lockfiles. The Gallery's release WASM after
wasm-bindgen grew from 11,453,072 to 11,725,743 bytes (+272,671; 4,062,514 to
4,161,332 gzip, +98,818; 2,873,477 to 2,947,504 Brotli, +74,027). After
`wasm-opt -Oz` it grew from 9,062,661 to 9,297,496 bytes (+234,835, +2.6%;
Brotli +71,098, +2.6%). Both figures include the recipe and the two Gallery
stories as well as the parser.

## Baseline decision and qualification

All 18 fixture text audits passed. The 16 existing fixtures keep
byte-identical pixels, metadata and semantics and identical measured
observations; only the native-control count rises from 38 to 40 for the two
new catalogue entries. Both new fixtures' visual, semantic and text artefacts
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
