# Accessibility and semantics

Egui/AccessKit provides the framework semantics where available. Polyorama
augments it with `UiSnapshot`: stable IDs, current bounded geometry, complete
names and descriptions, role, enabled/focused/selected state, actions,
disabled reason, pane/domain reference, text selectability and measured-text
observations. The snapshot is an observation of the current frame, not a
second application tree.

Custom interactive controls must expose a usable role and name, current state,
supported action, focusability, visible focus and a token minimum hit target.
Derive IDs from stable pane/domain identity. For example, dock tabs and
splitters use `PaneId` and `DockNodeId`; actions use their `ActionTarget`
semantic ID. Never target stale pixel coordinates in tests or automation.
Nodes that contain a supported text-selection surface set `text_selectable`;
measured-text observations retain the corresponding `TextInteraction` so a
physical test can resolve the current allocation before dragging.
Each visible string has one AccessKit owner. Standalone content uses its text
node; label/value pairs expose adjacent label and value nodes without repeating
the pair on an aggregate container; interactive chrome owns its painted text.

## Review checks

Inspect the current snapshot by stable action, role, pane or domain reference.
Require finite, positive node rectangles within the root; unique IDs; existing
parents; and disabled reasons only on disabled nodes. Compare the snapshot with
the AccessKit tree using the built-in parity audit: role, name, enabled state,
selection, description, click/adjust actions and bounds must agree.

Keyboard proof is not optional for buttons, tabs and splitters. Long visible
labels may elide, but their semantic names and tooltip/description must retain
the complete text. If native or browser platform screen-reader plumbing is not
enabled, report that limit accurately rather than claiming delivered support.
