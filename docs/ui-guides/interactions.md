# Interactions

Every user-visible capability has a closed `ActionId` registry entry. Its
`ActionSpec` supplies the stable ID, label, description, optional compact
label, shortcut and scope. Controls, shortcut routing, semantic metadata,
tests and physical targeting must share that action identity; do not create a
parallel string action name.

Bind a control to an `ActionTarget`: application actions have no pane target,
while pane and active-pane actions have a stable `PaneId`. Respect
`Availability`: enabled, disabled with an observable reason, or hidden are
different states. Disabled controls retain clear semantic explanation and do
not silently consume an unavailable action.

## Keyboard and pointer behaviour

Use the registered shortcut only in its valid scope. Pane shortcuts require an
active pane and non-command shortcuts yield to egui text input. Tabs support
roving Left/Right/Home/End plus Enter/Space activation. Splitters expose the
same 0.05 adjustment through keyboard and semantic increment/decrement paths
as they do through the gesture command path.

Use stable IDs and current semantic geometry for physical automation. During a
drag, preserve the original press position when delayed drag recognition is
involved, and commit only the completed gesture. Repaint only for a recorded
reason: interaction, state transition or scheduled work—not an unconditional
frame loop. Reduced motion removes custom animation timing rather than hiding
state changes.
