# Responsive navigation

Status: complete
Delivery: [PR #114](https://github.com/robchristie/stobie/pull/114)

## Outcome and boundaries

A chat-style application shows a list of destinations, such as
conversations, the one open from it and, optionally, that item's detail.
On a desktop the three stand side by side and the detail can be shown or
hidden; on a phone the list is the root, opening an item pushes it full
screen with Back, and the detail is a further push. Stobie had the
`navigation_item` recipe, the dock and egui's `SidePanel`, but no split or
stack recipe with Back, and a navigation item's target was one enum variant
per destination, so a dynamic list (one row per item) could not be
navigation items. Stobie now owns one navigation recipe that lays the panes
out by width, with Back, the detail toggle, regions agents can read, and
focus that follows the reader, and bound destinations whose argument names
the item. Applications own which item is open, whether its detail shows,
their destinations and how they route the actions.

Browser history is unchanged: the recipe pushes no history entries. The
in-app Back is the way back, and the browser's Back and the iOS edge swipe
leave the page as they do for any page, so they never fight the stack.
Integrating the stack with history, an announcement of the new level beyond
the focus move, extra header controls (such as a search or a menu beside the
title), a resizable list and a swipe-to-go-back gesture are later work.

## Implementation and UI acceptance contract

Representative task: on a phone, scan the conversation list, open one, read
it, show its details, then go back to the list; on a desktop, do the same
with everything in view and hide the details again; resize between them
without losing the open conversation. Names lead in the list and the open
row is marked; the open item's title heads its pane; Back and the detail
toggle are quiet icons in the header. Reference: the chat recipes'
conversation list and bubbles, the navigation item's current state, the
application bar's height, and chat and mail clients whose list, message and
detail panes collapse into a stack with Back on a phone.

- Recipe: `PresentationContext::navigation(ui, key, &NavigationSpec,
  availability, render)` and the hosted `Frame::navigation(ui, p, key,
  &NavigationSpec, render)`, which triggers Back and the toggle. The spec
  holds the list's title, the open item (`NavigationContent`: title, domain
  reference and optional `NavigationDetail` with its title, shown state,
  toggle action and icon) and the Back action. `render` is called once per
  pane shown with a `NavigationPane` (`List`, `Content`, `Detail`, or
  `Empty`, the content's place while nothing is open) and a child context
  whose nodes belong to the pane and which publishes only what is visible.
- Layout: `NavigationLayout::for_width`, typed policy in layout points:
  `Stack` below `SPLIT_FROM` (720, where `PaneWidthClass::Wide` starts),
  `Split` from 720 and `Columns` from `COLUMNS_FROM` (1040). Each
  breakpoint is the first width at which every pane it shows keeps its
  width: 320 for the list and the detail, at least 400 for the content.
  Between breakpoints the content takes what is left. The stack shows the
  deepest open level; the split shows the list beside the content, a shown
  detail taking the content's place with Back; the columns show all three
  with no Back.
- Headers: the application bar's height (or more for large text), a subtle
  bottom edge, Back as a quiet `ArrowLeft` icon at the start wherever the
  level it closes covers the one beneath, the title as an eliding section
  heading, and on the content the detail toggle, an icon toggle whose
  pressed state says whether the detail shows. Panes beside one another
  share a subtle edge; the list and the detail sit on the panel surface, the
  content and any stacked level on the canvas.
- Destinations: rows take a `BoundAction` with the item's ID, so one action
  opens any of them: `Frame::conversation_row`, or the new
  `Frame::navigation_item(ui, p, target, DestinationSpec)` and
  `PresentationContext::bound_navigation_item` for plain destinations. The
  open row is `selected`, a `NavigationItem` with `aria-current`.
- Semantics and agents: each pane but `Empty` is a `Pane` node and an
  AccessKit region named by its title, holding its header's controls and its
  body's nodes; the content's pane carries the open item's domain reference.
  Agents observe the panes and the current row, open an item through the
  open action's row bindings or, while the list is covered, its default
  binding with any ID, and go back with the Back action in every layout.
- Focus: opening a level that shows Back moves focus that was somewhere to
  Back; Back returns focus to the control that opened the level. Agents'
  changes take no focus that nobody had.
- State: the layout follows only the application's state and the width, so
  a resize keeps what is open, and each pane keeps its egui identity, so its
  scroll position survives layouts and levels.
- States: nothing open, an item open, its detail shown and hidden; the
  current, plain and disabled (archived) rows; long and localised titles and
  destinations with counts; a list crossing the fold; phone, tablet and
  desktop widths; 150% text; light, dark and high contrast. Excluded:
  loading and error states of the content (application-owned), and an empty
  list, which the application presents in the list pane.

Calibration question: does each width read as the same application, with
the open item obvious and the way back plain, and does a resize keep the
reader's place? Smallest probe: Gallery `navigation/responsive` at the
Gallery's new Desktop width in dark comfortable and at the Wide width in
light compact, and `navigation/responsive-narrow` in light high contrast at
150%. Evidence owner: [Responsive navigation evidence](../responsive-navigation-evidence.md).
Exit: focused host regressions at phone, tablet and desktop widths, a
browser journey, inspected baselines and an accepted two-pass critique, with
at most one refinement.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Layout policy, recipe, headers and bound destinations | Complete | 9 hosted regressions at phone, tablet and desktop widths, including resizes, Back, selection, scrolling, focus and agents |
| Gallery, browser journey, documentation and baselines | Complete | [Responsive navigation evidence](../responsive-navigation-evidence.md), three inspected baselines, `tools/responsive-navigation-browser-smoke.sh` |
| Candidate qualification | Complete | `cargo xtask verify` on the candidate; review, CI and merge retained in the PR |

## Closeout

The navigation recipe and its layout policy, bound destinations, the hosted
API, the Gallery stories with three fixtures and the Desktop Gallery width,
the browser journey, the compilable API example and the documentation are
complete. Final review, CI and merge qualification remain with the owning
pull request; this completed product state does not assert that it has
merged.

## Later work

- Integrating the stack with browser history, so the browser's Back and
  the platform's back gesture pop a level, without fighting the iOS swipe.
- Announcing the new level to assistive technology beyond the focus move.
- Header accessories: further controls beside the title, such as search or
  a menu.
- A resizable or collapsible list on a desktop.
