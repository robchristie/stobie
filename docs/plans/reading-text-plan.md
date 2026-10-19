# Reading text and links

Status: complete
Delivery: [PR #98](https://github.com/robchristie/stobie/pull/98)

## Outcome and boundaries

Applications such as a chat client need to show text of any length (a reply,
a document, long evidence) and links to external pages. The bounded content
recipe stops at eight lines and elides; egui's `Hyperlink` opens in the same
tab on the web, replacing a single-page application and its in-memory state.
Bokkie, the first consumer, worked around both with raw egui.

Stobie owns two recipes: reading text, measured and wrapped without a line
limit, selectable and observed; and a link that opens external URLs in a new
browser tab by default (the platform's default browser natively), with a
`Link` role that agents can discover and open through the framework capability
`stobie.open-link`. Applications own the text, URLs, labels, descriptions and
localisation. Markdown, links inside text, copy actions and content text in
observation are separate work; the reading path measures a `LayoutJob` so
rich text can use it later without bypassing measurement or observation.

## Implementation and UI acceptance contract

Representative task: read a long reply in a conversation at phone and desktop
widths, select and copy it, and open a cited source in a new tab without
losing the conversation. Text leads; bylines are quiet captions; links are
secondary, beneath the text they support. Reference: the bounded content
recipe and egui's hyperlink.

- `TextOverflow::WrapUnbounded`, used only by these recipes: no line limit
  (declared 0), allocation grows to the measured height, never truncated.
  Bounded recipes reject it with `UnboundedOverflow`. `line_count` widens to
  `u32` so long text is counted in full. The audit holds text to its
  allocation but accepts a parent scroll area's clip.
- `reading_text`: wraps at the available width, paragraph breaks, selectable
  by default through `present_measured_text`; one accessible label with the
  complete text and a stable author ID; observation kept while any of it is
  visible, omitted once scrolled out of view, with coverage counting every
  attempt; partially visible text kept by the host's visible-only policy.
- `link`: underlined inert label in the theme accent where it keeps 4.5:1 on
  canvas and panel, otherwise primary text; wraps without truncation; blank
  label shows the URL; token minimum hit size; hover tooltip with the URL,
  pointing hand and heavier underline without moving the allocation; focus
  ring; new tab by default, `SameTab` opt-out, middle or modifier click always
  a new tab; disabled opens nothing. Only http, https and mailto URLs open;
  any other URL is a failing diagnostic and an unavailable link, never a
  panic. Hosted web applications open new tabs with `noopener,noreferrer`. `UiRole::Link` maps to an AccessKit
  `Link` with its URL, checked by parity, and to `link` in the ARIA layer.
- Hosts publish each presented link as `stobie.open-link` (URL as the
  target's meaning); an agent's open always asks for a new tab. eframe's
  `links` feature opens URLs natively.
- States: ordinary, long, narrow, 150% text, high contrast, hover, keyboard
  focus, disabled, partially and fully scrolled. Excluded: loading and empty
  (application-owned), selection of link labels.

Calibration question: does a long reply read comfortably at phone and desktop
widths, and do links stay recognisable and subordinate? Smallest probe:
Gallery `reading/conversation` and `reading/long-narrow`. Evidence owner:
[reading-text evidence](../reading-text-evidence.md). Exit: focused
regressions, physical browser and native journeys and accepted two-pass
critique, with at most one refinement.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Reading text, links and the open-link capability | Complete | 16 recipe, 5 host and 3 Gallery regressions |
| Gallery, documentation and physical evidence | Complete | [UI evidence](../reading-text-evidence.md), two inspected baselines, browser and native journeys |
| Candidate qualification | Complete | `cargo xtask verify` on the candidate; review, CI and merge retained in the PR |

## Closeout

Both recipes, the open-link capability, two Gallery stories and fixtures,
documentation and physical journeys are complete. One design refinement
(label inset so the focus ring clears it) was accepted on re-critique. Final
review, CI and merge qualification remain with the owning pull request; this
completed product state does not assert that it has merged. Markdown and
links inside text, a copy action and content text in observation remain
separate work.
