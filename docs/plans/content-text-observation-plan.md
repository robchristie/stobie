# Content text in observation

Status: complete
Delivery: [PR #0](https://github.com/robchristie/stobie/pull/0)

## Outcome and boundaries

An agent working an application needs to read what a page says, such as a
heading, a reply or a cited link's text, to confirm an outcome. Observation
published controls' names and values and text layout measurements, but not
the words of presented content, so a consumer read them from a browser page's
accessibility layer instead, which host tests and native applications do not
offer an agent.

Stobie owns a bounded excerpt of each presented content text in observation,
an opt-out region for sensitive text, and reading it through the protocol, the
Rust and JavaScript clients and the CLI. Applications own which text is
sensitive. The text of controls, rows and chips stays with their semantic
nodes. A copy recipe is the next increment of this work and separate here.

## Implementation contract

Representative task: an agent confirms that a reply containing a phrase has
appeared on a conversation page, natively, in a browser and in a host test,
without reading a one-time code the page shows its owner.

- Content kinds: `section_heading`, `content_label` (content, status and
  fixed-slot recipes), `reading_text` (reading text and Markdown blocks),
  `link` and `list_marker`. Each of their text observations carries
  `content`, and no other kind does.
- An excerpt is the laid-out text (Markdown's presented words, the whole text
  even when it is elided on screen), cut on a character boundary to 256
  characters and then ending in `…` with `truncated` true. With 512 text
  observations, content is bounded per observation.
- Only visible text is published, by the host's existing visible-only policy.
- `withhold_content_text` marks presented content `withheld`, with neither
  words nor length; layout, audit and accessibility are unchanged. Regions
  nest. A password's text is never observed.
- The field is optional on the wire, so older observations still read.
- `UiSnapshot::content_text()`; a `content` wait condition in the Rust and
  JavaScript clients and the CLI; `stobie observe --content` in text and
  JSON, within the existing output budgets.

This increment changes no presentation, so it has no UI design calibration.
Gallery text baselines gain each content text's excerpt; their semantic and
visual baselines are unchanged.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Content text in observation | Complete | Protocol, host, client and CLI regressions; CLI smoke parity and budgets; regenerated Gallery text baselines |

## Closeout

Bounded content excerpts, the opt-out region, the clients' content wait and
the CLI's `observe --content` are complete and documented in the
[application interface](../application-interface.md#content-text). Final
review, CI and merge qualification remain with the owning pull request; this
completed product state does not assert that it has merged. A copy recipe
remains separate work.
