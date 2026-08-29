# Panes

The serialisable `polyorama_core::Workspace` is the sole dock tree. The dock
owns workspace layout; each pane owns only its local content and scrolling.
Do not introduce a second docking state or let a pane allocate GPU resources.

Pane entry points receive narrow read models plus intent, demand and render
sinks. Keep durable annotation changes in the document, session-only
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

Associate pane UI nodes with stable `PaneId`-derived semantic IDs and pane or
domain references. The snapshot may expose visible rows/cells, but never
materialise a logical collection merely for inspection.
