# Interactions

Every user-visible capability has an entry in an application-owned enum that
implements the framework `ActionKey` trait. Its generic `ActionSpec<A>`
supplies the label, description, optional compact label, shortcut and scope;
the key supplies the stable external ID. Controls, shortcut routing, semantic
metadata, tests and physical targeting must share that typed identity; do not
create a parallel string action name or add application capabilities to
`polyorama-ui-egui`.

Bind a control to an `ActionTarget<A>`: application actions have no pane
target, while pane and active-pane actions have a stable `PaneId`. Shared UI
components remain generic over `A`; semantic snapshots retain only a
`SemanticActionId` derived from `ActionKey::stable_id` so diagnostic consumers
do not need the originating application enum. Respect
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

Text selection is an explicit interaction, not a consequence of a text role.
Selectable measured text uses egui's label-selection state so drag, multi-label
selection and copy remain platform-consistent. A component with an owning click
or drag gesture keeps its text inert; expose the same technical value in the
Inspector when selection would compete with that gesture.
