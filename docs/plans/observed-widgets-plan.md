# Observed widgets and recipe-first examples

Status: complete
Delivery: [PR #PENDING](https://github.com/robchristie/stobie/pulls)

## Outcome and boundaries

The README says agents can operate every Stobie application with no
inspection code in it. A review on 2026-10-26 found that this held only for
widgets presented through recipes, and that the code every author copies did
otherwise:

- The `stobie new` template and the `notes` example presented their draft
  field as a raw `egui::TextEdit` inside `native`, then built a `UiNode` by
  hand and observed it: about sixteen lines of inspection code where
  `Frame::text_field` takes one call. The composition guide presented that
  route as the norm.
- The host clipped visible-only nodes before pairing text fields with their
  egui text edits, and a field without an AccessKit `author_id` paired only
  when its node's rectangle equalled the edit's. A hand-built field partly
  scrolled out of view in a dock pane lost `stobie.set-text`.
- Plain egui widgets, such as `ui.checkbox`, published no semantic node, and
  `UiRole` had no role for a checkbox, table, tree, menu, progress indicator
  or image.

Out of scope: semantic actions for plain widgets (an agent clicks them
physically, or the application declares an action); a checkbox's mixed
state; recipes for tables, trees and menus.

## Decisions

- The template and the example present the draft with `Frame::text_field`
  and `TextFieldSpec::id`, keeping the IDs `<prefix>.draft` and
  `notes.draft`. The recipe adds a visible "New note" caption above the
  field, which also names it.
- The host keeps each published node's rectangle from before clipping and
  pairs author-less text inputs on it, rather than requiring an `author_id`,
  so existing hand-built fields keep working unchanged.
- `PresentationContext::observed(ui, id, role, name, render)` takes an
  explicit semantic ID, as `observe_node` and `TextFieldSpec::id` do, so
  agents and documentation can address the widget. It builds the node from
  the response (rectangle, enabled, focused, and checked from the toggled
  state egui puts on the widget's AccessKit node, which inspection always
  enables) and gives the AccessKit node the same author ID, label, role and
  shown bounds. Text inputs, buttons, radio buttons, combo boxes and sliders
  are recorded as native text controls, so an observed `TextEdit` pairs for
  set text and the text audit stays clean.
- `UiRole` gains `Checkbox`, `Table`, `Tree`, `Menu`, `Progress` and `Image`.
  One `accesskit_role` mapping now serves both the AccessKit audit and
  `observed`; the agreement check maps them to `checkbox`, `table`, `tree`,
  `menu`, `progressbar` and `img`, and the ARIA layer gains `table`. The CLI
  names roles from their serde names, so the outline shows them as
  `checkbox` and so on, and `--role checkbox` selects them.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Template and example on the text field recipe | Complete | Template and example tests; the outside-app smoke builds the scaffold and sets its draft through `stobie`; `stobie observe` on the `notes` example |
| Pair text fields on unclipped rectangles | Complete | Host test `a_custom_field_partly_scrolled_out_of_view_keeps_set_text`, which fails without the fix |
| `observed` and the generic roles | Complete | Host tests that each role is published, agrees with AccessKit and the ARIA layer, reports a checkbox's state and enabled state, and sets an observed text edit's text; a CLI outline and selector test |
| Guides | Complete | Composition, start, components and accessibility guides and the README |
| Candidate qualification | Complete | `cargo xtask verify` on the reviewed head; review, CI and merge retained in the PR |

## Limits

- A checkbox in egui's mixed state publishes no `checked` value, while its
  AccessKit node is mixed, so the AccessKit audit reports it.
- `observed` reads a toggle's state from AccessKit; without inspection, which
  enables AccessKit, it publishes none, but then nothing is published at all.
