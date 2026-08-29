# Polyorama UI guides

These guides are the implementation and review entry points for the UI layer.
They apply the normative [design language](../design-language.md) and preserve
the architectural boundaries in the repository [working agreements](../../AGENTS.md).
They are deliberately focused: repository policy stays in `AGENTS.md`, and
component API details stay with the Rust types.

- [Components](components.md): production recipes, text and responsive contracts.
- [Panes](panes.md): narrow views, ownership and layout responsibilities.
- [Interactions](interactions.md): actions, gestures, shortcuts and repaint.
- [Tokens](tokens.md): authored design values and generated typed values.
- [Accessibility](accessibility.md): AccessKit, snapshots and keyboard paths.
- [UI review](ui-review.md): deterministic evidence and review checks.

The frozen task set and its scoring rules are in
[`../ui-evaluation-seed.md`](../ui-evaluation-seed.md) and the machine-readable
[`../ui-evaluation-seed.json`](../ui-evaluation-seed.json).
