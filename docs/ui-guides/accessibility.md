# Accessibility and semantics

Egui/AccessKit provides the framework semantics where available. Polyorama
augments it with `UiSnapshot`: stable IDs, current bounded geometry, complete
names and descriptions, role, enabled/focused/selected state, actions,
disabled reason, pane/domain reference, text selectability and measured-text
observations. The snapshot is an observation of the current frame, not a
second application tree.

The native applications compile eframe's AccessKit adapter, which consumes the
same egui tree exercised by semantic tests. Adapter compilation, framework
semantics and keyboard proof are independent evidence axes; none establishes
working screen-reader support by itself. Claim end-user support only for an
OS/assistive-technology combination whose representative workflow is retained
in the
[accessibility integration evidence](../accessibility-integration-report.md).
Stock eframe 0.36.1 provides no browser AccessKit adapter and discards the tree
update in `WebRunner`; keep that route blocked unless an upstream-supported
integration replaces the retained reproduction. Do not substitute eframe's Web
Speech event output for a browser accessibility tree.

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

Keyboard proof is not optional for buttons, tabs, splitters, bounded result
selection or viewport alternatives. Long visible labels may elide, but their
semantic names and tooltip/description must retain the complete text. Automated
tree inspection may verify adapter input but cannot replace actual
assistive-technology use. Report every unqualified or unavailable platform
accurately rather than inferring support from compilation or snapshots.
