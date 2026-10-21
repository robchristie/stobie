# Panes

The serialisable `stobie_core::Workspace` is the sole dock tree. The dock
owns workspace layout; each pane owns only its local content and scrolling.
Do not introduce a second docking state or let a pane allocate GPU resources.

Pane entry points receive narrow read models plus intent, demand and render
sinks. In a hosted application, `Frame::dock` gives each pane the `Frame`, an
intent and action sink, never the application; the host observes pane, tab
and splitter semantics. Keep durable annotation changes in the document, session-only
selection/camera/tool/gesture preview in the session, and hover/focus in the
UI. A completed gesture creates one validated command and one undo record;
render its preview during that frame.

## Responsive responsibility

Classify the available pane rectangle with `PaneSizeClass`:

- narrow: under 360 points; regular: 360–719; wide: 720 or more;
- shallow: under 280 points; regular: 280–599; tall: 600 or more.

Narrow panes retain the primary action and current state, collapse secondary
labels and move lower-priority actions to overflow. Shallow panes reduce
vertical chrome before sacrificing content. Scrolling remains local to the
pane. Wide and tall variants may reveal context but must still virtualise large
results and thumbnail collections.

## Responsive navigation

A chat-style application's list, open item and detail use the navigation
recipe, `Frame::navigation` (or `PresentationContext::navigation`), rather
than its own `SidePanel`s. The recipe lays its panes out by the width it is
given, using `NavigationLayout`'s typed breakpoints in layout points, the
same at every text size:

- stack, below 720 points (`SPLIT_FROM`): one level at a time, the deepest
  open one, full width. The list is the root; opening an item pushes its
  content under a header with Back, and a shown detail is a further push.
- split, from 720 to 1039 points: the 320-point list beside the content. A
  shown detail takes the content's place, under a header with Back.
- columns, from 1040 points (`COLUMNS_FROM`): the list, the content and a
  shown detail of 320 points side by side, with no Back.

Each breakpoint is the first width at which every pane it shows keeps its
width (320 for the list and the detail, 400 for the content), so between
breakpoints the content takes what is left and nothing shrinks below its
minimum. The application owns what is open and whether the detail shows; the
layout follows only that state and the width, so a resize, a rotation or a
browser window dragged across a breakpoint keeps what is open, and each
pane's scroll position survives because each pane keeps its identity. Each
pane but the empty content is a region named by its title, with a header of
at least the application bar's height. The content's, the detail's and the
empty prompt's bodies are inset from the pane's edges by as much as the
header's controls (`spacing.inline`), so text and bubbles align with the
title and keep off the screen's edge on a phone; the list's body is not, so
its rows' selection and hover fill its width.
`NavigationOptions::with_body_inset` turns either default over. Keyboard focus follows the reader: opening a
level that shows Back moves focus to Back, and Back returns it to the
control that opened that level. The recipe pushes no browser history; the
in-app Back is the way back, and the browser's Back and the platform's back
gesture leave the application as they do for any page. Safe areas are the
web shell's: the PWA template pads the page by the device's safe-area
insets, so the recipe draws within them. See the
[navigation recipe](components.md#responsive-navigation).

Associate pane UI nodes with stable `PaneId`-derived semantic IDs and pane or
domain references. The snapshot may expose visible rows/cells, but never
materialise a logical collection merely for inspection.
