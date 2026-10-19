# Reading text and links evidence

## Experience and design decision

Task and reference: read a long reply in a conversation, then open the
sources it cites without leaving the conversation. Compare with the existing
bounded content recipe (at most eight lines, then ellipsis) and egui's
`Hyperlink`, which eframe's web runner opens in the same tab, replacing the
page. Messages lead; bylines are quiet captions; links are secondary actions
beneath the text they support.

Behavioural checks: passed. Rust regressions cover wrapping at phone (343
points) and desktop (960 points) widths at 100–150% text, more than eight
lines, a 400-line text counted in full, the allocation equal to the measured
height, one accessible label with the complete text and a stable author ID,
drag selection copying a whole multi-paragraph reply, the audit's treatment of
unbounded text (clip accepted, allocation, line limit and truncation held),
bounded recipes rejecting unbounded overflow as a visible failure, scroll-area
partial and hidden observations with honest coverage, and stable identity
across layouts. Link regressions cover a new tab from pointer, Tab then Enter
and Space, and AccessKit click; the same tab only for `LinkTarget::SameTab`,
with a middle click still opening a new tab; a disabled link opening nothing;
the `Link` node, its URL and AccessKit parity (including a mismatched URL);
wrapping long labels and URL fallback for blank labels; a fixed allocation on
hover; only http, https and mailto URLs opening, with `javascript:`, `data:`,
blank and malformed URLs painting a failing diagnostic and an unavailable link
that opens nothing by any route; and link colour contrast in all four reference modes and the three
authored Gallery identities. Host regressions publish each presented link as
`stobie.open-link`, open a new tab when an agent invokes it (even for a
same-tab link), refuse a disabled link, reject a target whose URL changed and
report keyboard focus on the link's node. The web host takes new-tab
requests out of each pass's output and opens them with `noopener` and
`noreferrer`; a regression checks that only new-tab requests are taken.

Presentation checks: passed. The ordinary conversation and the narrow
high-contrast column at 150% were inspected for hierarchy, alignment,
legibility and balance. First critique, before reading the rationale: the
reply reads as one block with clear paragraph breaks; links are recognisable
by colour and underline and stay subordinate to the text; the unbroken
identifier wraps at its hyphens; the long German link label wraps rather than
eliding; the disabled link is clearly unavailable. What became harder: the
keyboard focus ring touched the link's first and last glyphs. Reading the
rationale did not answer that, so one refinement insets the label by half a
spacing unit either side within the link's allocation. Re-critique accepted
it: the ring now clears the glyphs, at the cost of starting link text two
points to the right of body text, which is barely perceptible. Remaining
trade-offs: egui underlines a trailing space where a link label wraps, and
the disabled link is faded by egui's disabled `Ui` on top of the muted
colour, which is acceptable for an inactive control.

Design decision: accepted after that one refinement. Reading text is a
recipe for text of any length, not a document editor: no inline links or rich
text yet (Markdown builds on its `LayoutJob` path), and no touch selection
beyond what egui offers.

## Rendered and physical evidence

- [Conversation, dark comfortable](../tests/ui-snapshots/expected/reading-conversation-dark/visual.png).
- [Narrow light high contrast at 150%](../tests/ui-snapshots/expected/reading-long-high-contrast/visual.png).

`tools/reading-text-browser-smoke.sh` runs in canonical verification on
WebGPU and again on WebGL2. In Chromium it drags across the whole reply and
reads the actual clipboard, which holds the reply exactly, with its paragraph
breaks. A physical click on a link, and Enter and Space after focusing the
link through the ARIA layer, each open a new tab showing the link's URL, with
no `window.opener` and no referrer, while the Gallery page keeps its URL and
story; example.org is answered locally, so
nothing leaves the host. It clicks a disabled link and sees no tab open,
checks the narrow column's wrapping, bare URL and clipped observation,
scrolls it, and confirms the idle page presents no frames.
`tools/reading-text-native-smoke.sh` runs the conversation natively on Linux
(Xvfb, GL/llvmpipe): it selects and copies with xdotool and clicks a link,
whose URL reaches the platform's default browser through a `BROWSER` recorder.
Both write their screenshots and workflow JSON, with source revision, binary
or WASM hashes and settled idle proof, under ignored `.tools/runtime/`; the
owning pull request retains the results.

## Baseline decision and qualification

All 16 fixture text audits passed. The 14 existing fixtures keep
byte-identical pixels, metadata and semantics and identical measured
observations; only the native-control count rises from 36 to 38 for the two
new catalogue entries. Both new fixtures' visual, semantic and text artefacts
were inspected outside `expected/` and accepted explicitly, before and after
the refinement. Frozen evaluation tasks, fixture definitions and scoring are
unchanged. Verification never updates baselines.

## Platform limits

Browser proof uses Chromium with WebGPU/SwiftShader and WebGL2; native proof
uses Linux Xvfb with GL/llvmpipe. The native smoke proves the request reaches
the default-browser mechanism, not that a particular desktop browser opens.
This adds no screen-reader qualification: the `link` role in the ARIA layer
and AccessKit parity are semantic and keyboard evidence only.
