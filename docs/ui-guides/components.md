# Components

Use the production recipes in `polyorama-ui-egui`; the gallery must call those
same recipes rather than recreate their appearance. A component is responsible
for its allocation, painted bounds, hit bounds, stable semantic identity and
declared text contract. It is not responsible for document mutation or owning
the application model.

## Required contract

Before adding or changing a reusable component, state and test:

- its text role, horizontal alignment, line limit, overflow policy and
  `TextInteraction` (`Inert` or `Selectable`);
- minimum useful width, narrow behaviour and whether it moves work into an
  explicit overflow control;
- complete semantic name/description, role, state, actions and visible focus;
- visual and minimum hit geometry separately; and
- bounded observations required for `UiSnapshot` and text auditing.

When using a native text control, call `record_native_text_control` on its
response with the matching `NativeTextControlKind`, including any options
submitted inside an open popup. This records the denominator without pretending
to measure egui's internal labels. Collect `text_audit_coverage` at the end of
the same UI pass; never substitute AccessKit nodes for layout observations.
Ordinary labels, headings and hover text remain an explicit excluded category.

Initialise fonts with `apply_design_system` before the first egui pass. For a
reading surface, apply `apply_design_system_with_typography` and resolve the
same `TypographyProfile` on its tokens. For application identity, use `apply_design_system_with_theme` and the same
`ApplicationTheme::resolve` output for custom recipes. Native widgets can consume
`TextRole::style(...).rich_text(...)` to share the complete role treatment.

Choose `measured_content_label` for actual bounded content height and
`measured_fixed_slot_label` only when the parent requires a deliberate line
slot. Never use a bounded label as a document reader; long evidence belongs in
a selectable scroll surface. Invalid component layouts paint a diagnostic and
fail audits. Preserve failed observations when filtering geometry, retain
coverage after the same layout pass, and assert required semantic content.

Use `TextSpec`, `TextRole` and `TextOverflow`; production widths come from
egui galley measurement, never character counts. Keep numeric values end
aligned. Retain full semantic text when paint is elided. The five supported
policies are `ellipsis`, `wrap`, `clip`, `scroll` and `expand`; choose one
explicitly rather than relying on incidental clipping.

Technical and user-content text is selectable unless its owning pointer
interaction conflicts with selection. Inspector values, diagnostic values,
status and error messages, and longer pane-local information use
`TextInteraction::Selectable`; buttons, tabs, toolbars, splitters, result rows
and thumbnail labels remain `Inert`. Selectable measured text must use
`present_measured_text`, which preserves Polyorama allocation and clipping
while delegating selection and copy behaviour to egui. Do not reproduce
cursor hit testing or clipboard handling in a component.

For dense chrome, preserve a token minimum hit target even when compact visual
geometry is smaller. Do not add decorative cards: spacing, aligned text and
the surface hierarchy should communicate grouping first.

## Existing recipe families

The current component layer covers action buttons, dock tabs and overflow,
splitters, application and pane toolbars, property rows, result rows, status
badges and virtual thumbnail cells. Their deterministic examples are the
gallery catalogue stories, including `tabs/many-long-labels`, `tabs/narrow`,
`splitter/hover-active`, `toolbar/narrow`, `property-row/long-value`,
`status/error-long-message`, and the virtual-grid stories.

When a new state is consequential, add a typed gallery story and a bounded
fixture rather than a runtime description or unbounded data set. See
[UI review](ui-review.md) for the evidence loop.

## Scoped presentation

Use `PresentationContext` when a feature needs the shared heading, bounded content,
fixed-slot label, property row, status badge and action recipes together. It holds resolved tokens, font scale and observations
for the current egui context, viewport and layout pass. Each method still receives
`&mut egui::Ui`; the adapter owns no UI, application model, intents or transport.
Application code continues to arrange rows, columns, scrolling and virtualisation.

Build `PresentationScope` from logical feature and domain keys, then give each
control a local string or typed enum key (`Hash + Debug`). For example,
`PresentationScope::new("obligation-detail").child(obligation_id)` with local keys
`"header-cancel"` and `"confirmation-cancel"` identifies two instances of the same
capability. Do not use row positions, display labels, counters or an incidental
egui parent ID. Move a logical scope unchanged between desktop and narrow layouts.
Domain metadata can use `DomainReference::External { namespace, id }`; it does not
change an action's capability target or supply identity implicitly.

```rust,ignore
let mut presentation = PresentationContext::new(
    ui, tokens, font_scale,
    PresentationScope::new("obligation-detail").child(obligation_id),
    detail_parent,
).with_domain_reference(DomainReference::External {
    namespace: "example.obligation".into(),
    id: obligation_id.to_string(),
});
presentation.heading(ui, "title", "Current obligation");
presentation.content(ui, "explanation", explanation, ContentTextSpec {
    role: TextRole::Body,
    overflow: TextOverflow::Wrap,
    max_lines: 3,
    interaction: TextInteraction::Selectable,
});
ui.horizontal(|ui| {
    if presentation.action(ui, "header-cancel", cancel_spec).clicked() {
        intents.push(cancel_intent);
    }
});
let observed = presentation.finish(ui);
text_layouts.extend(observed.text_layouts);
semantic_nodes.extend(observed.semantic_nodes);
```

An action specification paints the production component and records its matching
snapshot node and measured text. Its widget identity is separate from
`ActionTarget`, so repeated capabilities remain distinct. Heading and content
methods delegate the existing text recipes and retain their accessible text
owners, with stable responses and AccessKit author IDs. The legacy low-level
recipes retain their previous identities and appearance. Low-level action callers
can opt into `action_button_with_identity` and its matching
`action_semantic_node_with_identity` without migrating to the adapter.

Construct and finish inside each pass. Replace the latest publication for its
viewport on a repeated layout pass; do not append an earlier pass's publication.
Reusing or publishing an adapter in a different context, viewport or pass fails
immediately. A publication's measured count covers its retained local layouts;
attempted, failed and native counts use the shared viewport-pass inventory at
that point. Its text/node vectors contain only its own submissions. Recompute
`text_audit_coverage` once over the merged layouts at the viewport boundary;
never sum partial contexts' coverage. Keep failed observations when filtering visibility;
`retain_visible_text` enforces this while the pass inventory also retains failed
attempts independently of filtered geometry.

Use `native(ui, kind, closure)` for a native control: return its `Response` and
any application result from the closure. This records the native denominator,
without claiming egui's internal label layout was measured. Annotate an exceptional
raw presentation through `raw(ui, key, reason, closure)` and retain its returned
`raw_presentations` when exposing authoring observations. The reason explains the
presentation need, such as a native document reader. Raw code still owns required
native-control or measured-text recording. Ordinary egui layout needs no raw
annotation. The adapter does not recursively wrap every egui API.

`fixed_slot` delegates the deliberate line-slot recipe; `property_row` and `badge`
retain the production recipes' layout, selectable text and text observations.
Property IDs reserve the bit used by the recipe's label/value child IDs, so all
published numbers remain exactly representable in JavaScript. `tokens()` and
`font_scale()` expose the resolved appearance for application-owned layout and
native compositions without repeating appearance arguments at every component.

For a repeated row, call `scoped(ui, domain_id, |ui, row| { ... })` and use stable
local control keys inside the closure. The child inherits the semantic parent and
domain metadata, and merges its observations into the enclosing publication.
Where several domain objects share one enclosing context, application-owned
semantics must supply each object's correct domain reference explicitly; the
logical child key does not infer domain metadata. `observe_node` retains a custom
row or evidence node with the application's explicit public identity, parent and
domain reference. It does not manufacture text measurements or replace the
AccessKit owner. Keep native readers and row interactions in the application;
annotate their exceptional paint with `raw` and record native controls where used.
