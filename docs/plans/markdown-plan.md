# Markdown

Status: complete
Delivery: [PR #103](https://github.com/robchristie/stobie/pull/103)

## Outcome and boundaries

Applications that show replies from a model receive Markdown: headings,
lists of linked titles with notes, emphasis, inline code and code blocks.
Plain reading text shows the punctuation, and egui_commonmark draws plain egui
widgets outside Stobie's measurement and observation. Stobie owns one
Markdown recipe on the reading-text path, parsing CommonMark with
pulldown-cmark (MIT), presenting each block as measured, wrapped, observed
and selectable reading text, with links inside the text under the link
recipe's rules. Applications own the source, its trust and the descriptions
of their links. Tables, images, footnotes, task lists and maths keep their
text; raw HTML is never interpreted. Copy actions, content text in
observation and a DOM text layer for touch selection are separate work.

## Implementation and UI acceptance contract

Representative task: read a model's digest of linked titles at phone and
desktop widths, open a title in a new tab without leaving the conversation,
and copy an item, while the reply may still be arriving. Text leads;
headings are modest; numbers align; links are recognisable and subordinate;
code is set apart. Reference: the reading-text recipe and chat clients that
render Markdown.

- `markdown` and `PresentationContext::markdown` with `MarkdownSpec`
  (interaction, selectable by default; one link description). CommonMark with
  strikethrough: paragraphs, headings 1 to 6 (application title, pane title,
  then section heading roles), bulleted and numbered lists to any depth,
  block quotes, fenced and indented code, rules, emphasis, strong,
  strikethrough, inline code, links and autolinks. Raw HTML shown as written;
  images show their alternative text; tables and other extensions keep their
  source text.
- Each block one measured `ReadingText` surface under
  `TextOverflow::WrapUnbounded`; numbered-list numbers measured `ListMarker`
  labels in a column reserving two digits; bullets and quote bars decorative.
  Code in the `MonospaceTechnical` role (egui's bundled Hack) on the raised
  surface, wrapping rather than scrolling sideways. Headings are AccessKit
  headings with their level.
- Links inside text: link colour and underline, heavier on hover, pointing
  hand, focus ring with clearance; one Tab stop per link; click on any line
  of a wrapped link, middle or modifier click, Enter, Space and AccessKit
  open a new tab. `UiRole::Link` nodes with the URL as value, offered as
  `stobie.open-link`. Only http, https and mailto URLs open; others keep
  their text, open nothing and are unavailable links with the reason,
  without a failing text observation, since the URL is content.
- Streaming safety: any source parses without panicking; nesting is capped
  at 16 levels; list spacing ignores blank lines between items and the
  number column is reserved, so completed blocks never move as a reply grows.
  Nesting stops indenting where the column would fall below two minimum-hit
  widths, leading such items with their marker.
- States: ordinary, long, narrow, 150% text, high contrast, hover, keyboard
  focus, disabled, partial (streaming), unsafe link, partially and fully
  scrolled. Excluded: loading and empty (application-owned).

Calibration question: does a digest read comfortably at phone and desktop
widths, with titles findable and links subordinate? Smallest probe: Gallery
`markdown/digest` and `markdown/long-narrow`. Evidence owner:
[Markdown evidence](../markdown-evidence.md). Exit: focused regressions,
physical browser journeys on WebGPU and WebGL2, the size measurement and
accepted two-pass critique, with at most one refinement.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Parser, recipe, scoped API and open-link through hosts | Complete | 4 parser, 15 recipe, 1 host and 2 Gallery regressions |
| Gallery, documentation and physical evidence | Complete | [UI evidence](../markdown-evidence.md), two inspected baselines, browser journeys on WebGPU and WebGL2, size measurement |
| Candidate qualification | Complete | `cargo xtask verify` on the candidate; review, CI and merge retained in the PR |

## Closeout

The recipe, scoped API, two Gallery stories and fixtures, documentation,
browser journeys and the size measurement are complete. One design
refinement (focus ring clearance) was accepted on re-critique. Final review,
CI and merge qualification remain with the owning pull request; this
completed product state does not assert that it has merged. Tables, a copy
action, content text in observation and touch selection remain separate work.
