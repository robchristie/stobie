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

Action buttons retain their full token allocation while semantic and AccessKit
bounds use `Response::interact_rect`: a partly scrolled control advertises its
current visible target. Custom controls must keep both observations consistent
when clipping, and omit fully hidden nodes from current visible snapshots.

## Existing recipe families

The current component layer covers action buttons, dock tabs and overflow,
splitters, application and pane toolbars, property rows, result rows, status
badges, navigation items and virtual thumbnail cells. Their deterministic examples are the
gallery catalogue stories, including `tabs/many-long-labels`, `tabs/narrow`,
`splitter/hover-active`, `toolbar/narrow`, `property-row/long-value`,
`status/error-long-message`, and the virtual-grid stories.

When a new state is consequential, add a typed gallery story and a bounded
fixture rather than a runtime description or unbounded data set. See
[UI review](ui-review.md) for the evidence loop.

## Compact status chips

Use `PresentationContext::status_chip(ui, key, StatusChipSpec::new(label, tone))`
for concise status beside task names, detail headings, property summaries or
toolbars. Applications own workflow states, localisation, transitions and the
explicit `StatusTone`. “Scheduled” can be neutral; “Needs review” is a warning
only when the application's rules say so. Optional `IconId` is decorative;
optional `explanation` supplies context. Use a stable local key such as `"status"`
in a domain-scoped context, independently of label or tone changes.

The chip measures one start-aligned, vertically centred, regular `Status` line
with `TextOverflow::Ellipsis`. It is content-sized, capped by the generated
192-point maximum scaled with 100–150% text. `StatusChipWidth::AtMost(points)`
sets a smaller finite positive logical-point cap. Resolved inline/block spacing
supplies padding; artwork uses `icon_size` and one inline gap. Reserve these
before measuring the label. The useful label minimum is intrinsic width capped
at one scaled token minimum-hit width, with measured ellipsis as the floor.
Smaller parents/caps receive that minimum allocation: the parent must wrap,
scroll or relocate the chip. The recipe never implicitly becomes icon-only.
Empty/whitespace labels are rejected. Long messages belong in reading surfaces.

Standalone status defaults to `TextInteraction::Selectable`, using
`present_measured_text` and egui selection/copy. Selecting an entire elided galley
copies the original label, including non-ASCII text. Choose `Inert` explicitly
inside a pointer-owned row so click/drag stays with the parent. Inert chips still
own an informational accessible label. `StatusChipResponse` separates the
decorative `visual_rect`, selection `text_rect`, optional `icon_rect` and
`truncated` from its text `response`; selection sensing is not activation.
There is no button minimum hit target, focus stop, toggle, click action or live
announcement. Ellipsis provides a full-label tooltip; supplied explanation always
provides tooltip context.

Opaque panel fill and primary text use the validated `ApplicationTheme` pair,
preserving 4.5:1 standard and 7:1 high contrast on ordinary, hovered or selected
parents. The generated pill radius scales with text and is bounded by half the
visual height. Neutral uses a decorative boundary; success/warning/error use
semantic status colours. Artwork matches text foreground. Boundary colour
supplements the complete label and is not the sole meaning cue.

`UiRole::StatusChip` maps to one AccessKit Label with full `value`, optional
description and stable author ID. Bounds match the currently clipped text-response
allocation. Fully hidden owners/successful observations are omitted; partial
text retains allocation and effective clip, and pass coverage retains every
attempt. The scoped method inherits explicit parent/domain metadata and existing
viewport/pass publication rules. Decorative chrome/artwork adds no owner or new
platform accessibility claim.

The application-owned state mapping is an executable rustdoc example on
`PresentationContext::status_chip`; this short consumer helper uses the same API:

```rust
use polyorama_ui_egui::{DesignTokens, IconId, PresentationContext,
    PresentationObservations, PresentationScope, SemanticUiId,
    StatusChipSpec, StatusTone};

fn scheduled(ui: &mut egui::Ui, tokens: DesignTokens, task_id: u64)
    -> PresentationObservations
{
    let mut p = PresentationContext::new(ui, tokens, 1.0,
        PresentationScope::new("task-detail").child(task_id), SemanticUiId::root());
    p.status_chip(ui, "status", StatusChipSpec {
        icon: Some(IconId::Calendar),
        explanation: Some("Starts after the collection is approved"),
        ..StatusChipSpec::new("Scheduled", StatusTone::Neutral)
    });
    p.finish(ui)
}
```

Low-level callers use `status_chip` with a stable `PresentationId`, then publish
`status_chip_semantic_node` and measured observations. `status_chip_colours`
exposes the painted pair for theme checks. Choose the existing wrapping
`status_badge`/`PresentationContext::badge` for longer selectable status/error
messages. Choose `NavigationBadge` for navigation-row counts/context, including
its `99+` abbreviation and row-owned semantics. Gallery `status-chip/tasks`,
`status-chip/treatments` and `status-chip/long-narrow` use the production recipe;
the [evidence record](../status-chip-evidence.md) retains visual and physical proof.

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
snapshot node and, when painted, measured text. Its widget identity is separate from
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

## Typed icons and action presentations

Select artwork with `IconId`; never pass a glyph, emoji or runtime icon name.
`PresentationContext::action` remains the text-only recipe. `icon_action` paints
an icon-only action; `action_with_icon` paints a leading decorative icon and
label. `action_with_content` accepts the explicit `ActionButtonContent` choice.
These methods share capability, availability, emphasis, toggle, response and
logical-identity contracts. `ActionButtonSpec` struct literals remain compatible.

The icon-only allocation is a square token minimum hit target (32 points at the
reference scale), with centred square visual chrome and 16-point artwork. Artwork
scales with 100–150% font preferences. Its full action name and description own
semantics and a tooltip, including a disabled reason; the icon has no separate
accessible owner and produces no measured-text attempt. Icon+label uses one
start-aligned, inert, eliding `ButtonLabel`, vertically centred beside the icon.
Its gap is `spacing.inline` (5 points compact, 7 comfortable). Full action text
remains in semantics and appears in a tooltip on truncation/compact presentation.

A labelled action reserves padding + icon + gap + **measured ellipsis** as its
minimum useful width. It can therefore exceed a smaller available width. A parent
must provide that minimum, wrap the action group or move an action into explicit
overflow. Selecting `icon_action` is an explicit caller decision, retaining a
usable name; the component never silently collapses its label. Text-only layout
and its historical identities remain compatible. Quiet, QuietBorderless, Normal
and Primary share the existing enabled, pressed/selected, disabled and focus
rules; icons use the exact resolved label foreground. Enabled pressed/selected
actions have an inset state outline separate from keyboard focus, and Primary
hover changes its outer border. These shared chrome corrections make states
visible even when the analytical reference aliases hover and selection fills;
resting text-only layout and appearance remain compatible.

The following helper compiles as a rustdoc example on `icon_action`. Handle the
returned click through your application's intent/command route and publish the
returned observations for this pass:

```rust
use polyorama_ui_egui::{
    ActionButtonSpec, ActionButtonState, ActionEmphasis, ActionKey, ActionTarget,
    Availability, DesignTokens, IconId, PresentationContext, PresentationObservations,
    PresentationScope, SemanticUiId,
};

// A is the application's own ActionKey enum; undo/save are application actions.
fn toolbar<A: ActionKey>(
    ui: &mut egui::Ui, tokens: DesignTokens, font_scale: f32, undo: A, save: A,
) -> (bool, PresentationObservations) {
    let mut p = PresentationContext::new(
        ui, tokens, font_scale, PresentationScope::new("document-toolbar"),
        SemanticUiId::root(),
    );
    let mut save_clicked = false;
    ui.horizontal(|ui| {
        p.icon_action(ui, "undo", ActionButtonSpec {
            target: ActionTarget::application(undo),
            availability: Availability::Disabled { reason: "History is empty".into() },
            state: ActionButtonState::Momentary,
            emphasis: ActionEmphasis::QuietBorderless,
            compact: false,
        }, IconId::Undo);
        save_clicked = p.action_with_icon(ui, "save", ActionButtonSpec {
            target: ActionTarget::application(save),
            availability: Availability::Enabled,
            state: ActionButtonState::Momentary,
            emphasis: ActionEmphasis::Primary,
            compact: false,
        }, IconId::Save).clicked();
    });
    (save_clicked, p.finish(ui))
}
```

Low-level callers use `action_button_with_content`, or
`action_button_with_identity_and_content` for distinct logical instances, and
record the matching `action_semantic_node_with_identity`. The reusable
`paint_icon(painter, IconId::Calendar, artwork_rect, foreground)` creates decorative
vector paint only; obtain component dimensions from `icon_size(tokens, font_scale)`
and choose a foreground from the owning resolved theme. Application compositions
still own allocation, hit geometry and complete semantics.

The [artwork record](../../crates/polyorama-ui-egui/assets/icons/README.md) owns the
SVG source/compiler and licence. Gallery `icons/vocabulary`,
`icons/action-presentations`, `icons/toolbar` and `icons/long-narrow` demonstrate
production compositions and adversarial cases. New platform accessibility claims
require the existing [qualification contract](accessibility.md); these recipes
extend semantic and keyboard coverage, not the platform support matrix.

## Navigation items

`PresentationContext::navigation_item` paints one full-width sidebar destination
and automatically records its `NavigationItem` node plus separate
`NavigationLabel`/`NavigationBadge` measurements. Supply a
`NavigationItemSpec<'_, A>` with typed `ActionTarget<A>`, `IconId`, full borrowed
label and description, caller-owned `selected`, `Availability` and an optional
`NavigationBadge`. The leading artwork and badge are decorative; the row alone
owns interaction and accessible text. Existing action APIs remain compatible.

Use stable logical scope and destination keys, such as an application enum,
independently of labels, counts, order and desktop/narrow placement. Handle
`Response::clicked()` through the application's intent/command route. Selection,
routing and persistence belong to the consumer; focus never selects a destination.
Each enabled row is an independent Tab stop; Enter/Space and pointer release
activate it. There is no composite arrow-key or roving-selection policy.

The label is an inert, start-aligned, centred single-line `ButtonLabel` with
explicit ellipsis. Its tooltip and semantic name retain the complete label.
`navigation_item_minimum_width` measures reservations for padding, icon, gaps,
a useful label slot of two minimum-hit widths scaled with text, and the bounded
badge. Parents must provide that width, scroll horizontally or relocate the
sidebar. A smaller parent receives a minimum allocation that overflows; neither
label nor badge is implicitly removed. The full row retains a token hit height,
increasing for density and enlarged fonts. Selected rows keep a leading marker
and fill, hover adds a control outline, press uses the validated selection colour pair and adds an inset state outline, and
keyboard focus adds the outer focus ring. Current state remains recognisable
when reference hover and selection fills coincide, including a disabled current
row. All colours resolve from the supplied theme tokens.

`NavigationBadge::Count { value, meaning }` shows exact 0–99 and `99+` above 99.
Its inert Caption is end-aligned, and the count slot reserves measured `99+`
width so neighbouring count badges align. `Label { text, description }` paints
one centred, eliding Caption. Badge text width is capped at two minimum-hit widths
scaled with text, plus inline padding; it never wraps or becomes another target.
The row description retains the full count and meaning or supplied status
explanation. `None` omits the badge; zero is shown only when explicitly supplied.
Count/status changes preserve the stable destination name, identity and focus.

`UiRole::NavigationItem` uses `UiNode.selected` to mean **current destination**.
AccessKit exposes Button + `aria_current(True/False)`, with no selected/toggled
property. Both use the full name and description, current state, badge meaning,
disabled reason and clipped `Response::interact_rect`. Hidden availability and
fully clipped rows publish no visible node. This is an application navigation
action, distinct from dock tabs and independent toggles. Native AccessKit
semantics and keyboard testing add no new screen-reader qualification; eframe’s
browser accessibility adapter limitation remains unchanged.

The [compilable consumer example on `navigation_item`](https://github.com/robchristie/polyorama/blob/main/crates/polyorama-ui-egui/src/presentation.rs)
defines its own destination enum, selection and counts. Gallery
`navigation/sidebar`, `navigation/states` and `navigation/long-narrow` use this
production API, including scrolling and long-label/badge adversaries.

Low-level callers use `navigation_item` with a stable `PresentationId`, and
publish `navigation_item_semantic_node` only for positive current interaction
bounds. Gallery audits its fully visible text allocation subset, retaining all
measurement attempts in coverage. Production observations include partial clips.
Preserve the viewport/pass inventory when filtering clipped successful text; never discard failed measurements. The scoped method handles ordinary
publication and inherits the context's explicit domain reference and target pane.
