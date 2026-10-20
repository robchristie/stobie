# Components

Use the production recipes in `stobie-ui-egui`; the gallery must call those
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
`TextRole::style(...).rich_text(...)` (with the `TextRoleStyles` trait in
scope) to share the complete role treatment.

Choose `measured_content_label` for actual bounded content height and
`measured_fixed_slot_label` only when the parent requires a deliberate line
slot. Never use a bounded label as a document reader: text of any length, such
as a message or long evidence, uses the [reading-text recipe](#reading-text) in
a scroll area. Invalid component layouts paint a diagnostic and
fail audits. Preserve failed observations when filtering geometry, retain
coverage after the same layout pass, and assert required semantic content.

Use `TextSpec`, `TextRole` and `TextOverflow`; production widths come from
egui galley measurement, never character counts. Keep numeric values end
aligned. Retain full semantic text when paint is elided. The bounded recipes
support `ellipsis`, `wrap`, `clip`, `scroll` and `expand` with 1 to 8 lines;
choose one explicitly rather than relying on incidental clipping. The sixth
policy, `wrap_unbounded`, belongs to reading text and links alone: bounded
recipes reject it with an `UnboundedOverflow` diagnostic.

Technical and user-content text is selectable unless its owning pointer
interaction conflicts with selection. Inspector values, diagnostic values,
status and error messages, and longer pane-local information use
`TextInteraction::Selectable`; buttons, tabs, toolbars, splitters, result rows
and thumbnail labels remain `Inert`. Selectable measured text must use
`present_measured_text`, which preserves Stobie allocation and clipping
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
badges, navigation items, list rows, composers and virtual thumbnail cells. Their deterministic examples are the
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
use stobie_ui_egui::{DesignTokens, IconId, PresentationContext,
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

## Reading text

Use `PresentationContext::reading_text(ui, key, text, ReadingTextSpec::default())`
for text of any length that a person reads: a chat message, a reply, a
document or long evidence. It wraps at the available width to as many lines as
the text needs, allocates its full measured height and is never truncated.
Paragraph breaks (`\n`) start new lines. `ReadingTextSpec` chooses the measured
`TextRole` (body by default) and `TextInteraction`: reading text is selectable
by default, through `present_measured_text` and egui's own drag selection and
copy, so selecting the whole text copies it exactly. Choose `Inert` only where
a parent owns the pointer, such as a clickable row.

Put long text in a scroll area, such as `egui::ScrollArea::vertical()`; the
recipe has no scroll surface of its own. Its observation declares
`TextOverflow::WrapUnbounded` with a line limit of 0 (none) and reports the
actual line count, however large. The audit holds the painted text to its
allocation but accepts a parent scroll area's clip, so a message that crosses
the fold keeps its observation, with the clip recorded. Text scrolled fully
out of view adds no observation, while the pass's coverage still counts the
attempt. The text response owns one accessible label with the complete text
and the context's stable author ID; there is no separate semantic node, as for
`content`. Use `content` for short bounded text (1 to 8 lines) with a
deliberate overflow policy, and `reading_text` whenever the text could be
longer.

Low-level callers use `reading_text(ui, identity, text, spec, tokens,
font_scale, observations)` with a stable `PresentationId`. Every reading
surface shares one measured path from a `LayoutJob`, so later rich text
(styled runs, links within text, code) is measured, observed and audited the
same way; [Markdown](#markdown) presents through it. Gallery
`reading/conversation` and `reading/long-narrow` use the
production recipe; the [evidence record](../reading-text-evidence.md) retains
the review.

## Markdown

Use `PresentationContext::markdown(ui, key, source, MarkdownSpec::default())`
for Markdown a person reads, such as a reply from a model. It presents
CommonMark with strikethrough on the reading-text path: paragraphs; headings
(level 1 in the application-title role, level 2 the pane-title role, levels 3
to 6 the section-heading role); bulleted and numbered lists to any depth;
block quotes; fenced and indented code; horizontal rules; and, inside text,
emphasis, strong, strikethrough, inline code and links. Code uses the
`MonospaceTechnical` role in egui's bundled monospace face (Hack) on the
raised surface; a code block wraps at the available width rather than
scrolling sideways. Raw HTML is shown as written and never interpreted; an
image shows its alternative text and loads nothing; tables are not
recognised and keep their source text, as do footnotes, task lists and maths.
Bare URLs are not linked; `<https://…>` and `<name@example.org>` autolinks
are.

Each block is one measured reading surface, wrapped at its column's width and
never truncated, observed as `ReadingText` with `TextOverflow::WrapUnbounded`
and selectable by default, so a drag can select across blocks and copy them.
A numbered list's numbers are measured, end-aligned `ListMarker` labels in a
column that always fits two digits; bullets (disc, circle, square by depth)
and the quote bar are decorative paint. Blocks inside a list item are spaced
like the items, whatever blank lines the source has, so a list does not
reflow as a reply streams in. Nesting indents while the column keeps two
token minimum-hit widths of text; deeper or narrower items keep their marker
at the start of their text instead, and the model keeps at most 16 levels.
Headings are AccessKit headings with their level, each block's label carries
its complete text and the context's stable author ID, and there is no
separate semantic node for text, as for `reading_text`.

A link inside the text is underlined in `link_colour`, with a heavier
underline and a pointing hand on hover, and a focus ring that clears its
glyphs. It opens in a new browser tab (natively, the default browser) from a
click anywhere on any line it wraps across, a middle or modifier click, Tab
then Enter or Space (one Tab stop per link), or assistive-technology
activation. A drag that starts on a link selects text instead. Each link is a
`UiRole::Link` node named by its text with its URL as its value, so hosts
offer it as `stobie.open-link`; `MarkdownSpec::link_description` gives every
link the application's description, such as "Opens in a new tab". Links
follow the [link recipe's rules](#links): only `http`, `https` and `mailto`
URLs open. A link to any other URL, a relative one or an empty one keeps its
text as ordinary text, opens nothing by any route and is published as an
unavailable link whose reason is `INVALID_LINK_URL`; unlike the standalone
link, it records no failing text observation, because the URL is content,
not an application mistake. A disabled `Ui` mutes links and opens nothing.

Any source parses and presents without panicking, including one still
arriving: an unfinished `**`, link or fence stays text or code until it
closes. Put long content in a scroll area: blocks and links scrolled fully
out of view add no observation or node, while coverage counts every attempt.
Choose `TextInteraction::Inert` only where a parent owns the pointer; links
stay activatable.

Low-level callers use `markdown(ui, identity, source, spec, parent, tokens,
font_scale, observations)` and publish the returned `link_nodes`. Gallery
`markdown/digest` and `markdown/long-narrow` use the production recipe; the
[evidence record](../markdown-evidence.md) retains the review.

## Links

Use `PresentationContext::link(ui, key, LinkSpec::new(label, url))` for a link
to an external URL. Activating it opens the URL in a new browser tab by
default, so a single-page application keeps its in-memory state; eframe's web
runner would otherwise open a same-tab link as `_self`, replacing the page.
Natively it opens the platform's default browser (Stobie enables eframe's
`links` feature). `LinkTarget::SameTab` is the explicit opt-out; a middle or
modifier click always opens a new tab. Pointer release, Tab then Enter or
Space, and assistive-technology activation share one route. A hosted web
application opens every new-tab request with `noopener` and `noreferrer`, so
the linked page cannot navigate the application's tab.

A link opens only `http://` and `https://` URLs with something after the
scheme and `mailto:` addresses (`link_url_allowed`). Any other URL, including a blank one or a
`javascript:` or `data:` URL, paints the usual measured diagnostic, records a
failing observation (`InvalidUrl`) and presents an unavailable link whose
semantic and accessible description give the reason; it never panics and
opens nothing. Stobie checks only the scheme: whether a URL from an untrusted
source, such as a model's reply, should be offered as a link at all is the
application's decision.

The label is an underlined, inert measured label in the spec's `TextRole`
(body by default). It wraps at the available width without a line limit and
is never truncated; a blank label shows the URL. The allocation hugs the label,
with half a spacing unit either side so the focus ring clears the glyphs, and
keeps the token minimum hit size; the label is start-aligned within it and
centred vertically. Hover shows the URL in a tooltip, a pointing-hand cursor and a
heavier underline without moving the allocation; keyboard focus adds the token
focus ring. The colour is the theme's accent where it keeps 4.5:1 against both
canvas and panel surfaces (`link_colour`), otherwise primary text; the
underline marks the link either way, so colour is never the only cue. A
disabled `Ui` gives a muted link that opens nothing. Place links on canvas or
panel surfaces; the analytical reference accent keeps at least 4.5:1 there in
light and dark and 7:1 in high contrast.

`UiRole::Link` maps to one AccessKit `Link` named by the label, with the URL
as its `url`, the optional `description`, a click action while enabled and the
current clipped hit bounds. The semantic node carries the URL as its `value`,
and the parity audit checks that the two agree. The ARIA layer presents it as
`link`. Hosts publish every presented link as the framework capability
`stobie.open-link`, so agents can discover it and open it (`stobie invoke
stobie.open-link --id ID`) without application code; the URL is the target's
meaning, so a link whose URL changed since discovery is stale. An agent's
open always asks for a new tab, even for a `SameTab` link, so the application
under inspection stays open. Descriptions are the application's own words,
for example "Opens in a new tab"; Stobie adds no text of its own.

Low-level callers use `link(ui, spec, identity, tokens, font_scale,
observations)` and publish `link_semantic_node` while it returns a node.

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

In a hosted application (`AppHost`), a context registers its observations
with the pass's collector when it is finished or dropped, so no merging is
needed and the steps below do not apply. Without a host, construct and finish
inside each pass. Replace the latest publication for its
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
use stobie_ui_egui::{
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

The [artwork record](../../crates/stobie-ui-egui/assets/icons/README.md) owns the
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

The [compilable consumer example on `navigation_item`](https://github.com/robchristie/stobie/blob/main/crates/stobie-ui-egui/src/presentation.rs)
defines its own destination enum, selection and counts. Gallery
`navigation/sidebar`, `navigation/states` and `navigation/long-narrow` use this
production API, including scrolling and long-label/badge adversaries.

Low-level callers use `navigation_item` with a stable `PresentationId`, and
publish `navigation_item_semantic_node` only for positive current interaction
bounds. Gallery audits its fully visible text allocation subset, retaining all
measurement attempts in coverage. Production observations include partial clips.
Preserve the viewport/pass inventory when filtering clipped successful text; never discard failed measurements. The scoped method handles ordinary
publication and inherits the context's explicit domain reference and target pane.

## Rows and their actions

A list whose rows each offer actions presents them with the ordinary recipes
and a `BoundAction` per row; [Actions on rows](interactions.md#actions-on-rows)
owns the contract. In a hosted pane:

```text
for person in &self.people {
    frame.list_row(ui, p, BoundAction::new(Action::Select, person.key),
        ListRowSpec { title: &person.name, detail: Some(&person.role) });
    ui.horizontal(|ui| {
        let row = |action| BoundAction::new(action, person.id).with_subject(&person.name);
        frame.action(ui, p, row(Action::Open), ActionEmphasis::Normal);
        frame.toggle(ui, p, "pinned", row(Action::Pin), person.pinned);
        frame.icon_action(ui, p, row(Action::Remove), IconId::Remove, ActionEmphasis::Quiet);
    });
}
```

`Frame::list_row` presents one full-width row whose whole surface performs its
bound action for that row: a `ListRowSpec` with the row's title, one line of
`ButtonLabel` text as in navigation items, and an optional detail, one line of
`Secondary` text, both inert and elided with an ellipsis, within block padding
and at least the token hit height (`list_row_height`). The row is a Button
named by its full title; its description is the detail followed by the
action's description, and a disabled row adds its reason. Hover uses the quiet
action hover fill, press the validated selection pair with an inset outline,
and keyboard focus the outer focus ring. Each enabled row is a Tab stop;
Enter, Space and a pointer release activate it. The text measurements are
`ListRowTitle` and `ListRowDetail` under a `ListRow` parent. A row has no
current or selected state; a destination list uses navigation items. Low-level
callers use `list_row` with a stable `PresentationId` and publish
`list_row_semantic_node` for positive interaction bounds, and
`PresentationContext::list_row` handles ordinary publication.

Gallery `rows/actions` and `rows/long-narrow` present list rows with their
actions, including a disabled action, a disabled row, elision and scrolling;
the [snapshot guide](../../tests/ui-snapshots/README.md) lists their fixtures.

Buttons, icon actions and toggles carrying a bound action keep their visible
label and measurements; their accessible name and tooltip add the subject,
and `PresentationContext::bound_action` is the low-level form. A hidden
availability for a row's argument presents nothing for it.

## Composer

A composer edits a message and sends it: a multiline field that grows with
its text, a send control, and optional leading and trailing controls such as
Attach and a microphone. [Composing and sending](interactions.md#composing-and-sending)
owns the draft, Return, focus and agent conventions. In a hosted pane:

```text
let spec = ComposerSpec::new("Message", ComposerControl::icon(Action::Send, IconId::ArrowUp))
    .hint("Message Ada")
    .leading(ComposerControl::icon(Action::Attach, IconId::Add))
    .trailing(ComposerControl::label(Action::Dictate));
frame.composer(ui, p, "composer", &spec, &self.draft, Intent::Draft);
```

The send action must take a text argument (`argument = "text"`, normally with
a `max_length`); the composer panics otherwise. Each `ComposerControl` shows
its action as an icon (`icon`), its label (`label`) or both (`icon_label`),
so artwork can replace a label later without other changes; an icon-only
control is named by its action's label. `in_pane` targets a pane action.

- **Layout.** One surface on the text-field ground, with the control border
  and corner radius, holds the leading control at the left, the field, then
  the trailing control and send at the right. Its border takes the focus-ring
  colour while the field has focus. Controls are quiet and borderless, except send, which is
  primary; each keeps the token hit height and sits at the bottom of the
  field, so they stay beside the last line as it grows. A hidden control
  takes no space: an empty draft with send hidden leaves the trailing
  control at the right.
- **The field.** A frameless `NativeTextEdit` in `Body` text, padded so that
  one line fills the controls' height. It starts at one line, grows with
  wrapped and typed lines to `max_rows` (`COMPOSER_DEFAULT_MAX_ROWS`, six)
  and then scrolls; the controls follow its height one pass later. Its hint
  shows on one line, elided with an ellipsis, while the accessible
  placeholder keeps the whole hint.
- **Semantics.** The field is a `MultilineTextInput` node named by the
  spec's name, whose value is the draft and whose bounds are the part shown.
  Each control is an ordinary action button node; send carries the draft as
  its argument.
- **Responses.** `Frame::composer` returns a `ComposerResponse` with the
  field's response, whether the draft changed, the text submitted, and each
  control's target and response. `PresentationContext::composer` is the
  low-level form: it edits a `&mut String`, takes an availability closure,
  and leaves the caller to trigger the actions it reports.

Gallery `composer/chat` and `composer/long-narrow` present an empty draft
with the microphone alone, a draft ready to send, grown lines, a long
localised hint, a draft past its line limit and a disabled send; the
[snapshot guide](../../tests/ui-snapshots/README.md) lists their fixtures.

## Avatars

`PresentationContext::avatar(ui, &spec)` allocates and paints one avatar at
its size and returns a hover-only response. An `AvatarSpec` holds a figure,
an `AvatarSize` and an optional image:

```rust
use stobie_ui_egui::{
    AvatarFace, AvatarOutline, AvatarSize, AvatarSpec, OutlineSegment, PresentationContext,
};

// An application's own shape, checked when the program is built.
const DROP: AvatarOutline<'static> = AvatarOutline::new([0.5, 0.04], &[
    OutlineSegment::Cubic([0.62, 0.22], [0.9, 0.45], [0.9, 0.62]),
    OutlineSegment::Cubic([0.9, 0.85], [0.72, 0.98], [0.5, 0.98]),
    OutlineSegment::Cubic([0.28, 0.98], [0.1, 0.85], [0.1, 0.62]),
    OutlineSegment::Cubic([0.1, 0.45], [0.38, 0.22], [0.5, 0.04]),
]);

fn people(ui: &mut egui::Ui, p: &mut PresentationContext, photo: Option<egui::load::SizedTexture>) {
    let bot = AvatarSpec::shape(DROP, egui::Color32::from_rgb(150, 122, 228),
        AvatarFace::Eyes { centre: [0.5, 0.64] }, AvatarSize::List);
    p.avatar(ui, &bot);
    let mut person = AvatarSpec::person("Ada Lovelace", AvatarSize::List);
    person.image = photo; // None while loading or after a failure.
    p.avatar(ui, &person);
}
```

- **Figures.** `AvatarFigure::Person { name }` shows up to two initials, the
  first letter or digit of the name's first and last words in upper case, in
  semibold text on the raised surface with a decorative border, shrinking to
  fit and centred by their ink. A name without letters or digits, or one the
  fonts cannot draw, shows the `IconId::User` glyph instead.
  `AvatarFigure::Shape { outline, fill, face }` fills an `AvatarOutline` with
  the colour and draws `AvatarFace::EYES`, eyes at a chosen centre, or
  `AvatarFace::None`.
- **Outlines.** An outline is a start point and at least two `Line`,
  `Quadratic` or `Cubic` segments in the unit square, closing back to its
  start; concave outlines fill correctly. `AvatarOutline::new` is `const`
  and fails the build for points outside the square; `try_new` returns an
  `AvatarOutlineError` for outlines from data. `CIRCLE`, `ROUNDED_SQUARE`
  and `HEXAGON` are built in. The application owns its shapes and colours,
  including their contrast with the surface behind them.
- **Images.** An image (`egui::load::SizedTexture`) fills the figure's
  outline, cropped to its centred square, with a decorative border. The
  application loads it; pass `None` while it loads or after it fails and the
  figure is drawn.
- **Sizes.** `avatar_size(tokens, size, font_scale)` gives the diameter:
  `avatar.headerSize`, `avatar.listSize` and `avatar.stripSize` (32, 40 and
  56 points) times the font scale clamped to 100–150%.
- **Semantics.** An avatar publishes no node and no measured text, and its
  initials are not observed: it is decorative, like an icon, and the row,
  heading or control beside it names the person or thing. An avatar that
  acts (a pinned person who opens a conversation) belongs inside a control
  with that name.

Low-level callers use `avatar(ui, &spec, tokens, font_scale)`, or
`paint_avatar(painter, &spec, rect, tokens)` to paint into the largest
square in `rect`; `avatar_initials` and `avatar_eye_colour` expose the
derivations. Gallery `avatars/figures` shows each size, people (including
the glyph fallback), the built-in and application shapes, and images; the
[snapshot guide](../../tests/ui-snapshots/README.md) lists its fixtures.

## Local date and time

Format instants for people with `stobie_ui_egui::local_time`, never with
hand-written offsets or relative phrases. `LocalTime::viewer()` takes the
viewer's zone and locale and the system clock; `LocalTime::new(zone, locale,
clock)` chooses each, and `with_zone` shows the same instants in another
zone, such as the IANA zone a person's schedule runs in
(`TimeZone::named("Australia/Adelaide")`, which refuses unknown names).
Instants are `Timestamp`s: whole milliseconds since the Unix epoch, in years
1 to 9999.

| Method | Example (12-hour, day before month) | Example (24-hour) |
| --- | --- | --- |
| `time_of_day` | 9:41 AM | 09:41 |
| `clock_time(ClockTime)`, a wall-clock time without a date | 7:00 AM | 07:00 |
| `short_date`, with the year only outside the current year | 14 Apr, 14 Apr 2025 | 14 Apr |
| `long_date` | Tuesday 14 April 2026 | Tuesday 14 April 2026 |
| `relative_day` | Today, Yesterday, Tomorrow, Tuesday (within the last six days), then the short date | the same |
| `relative_day_and_time` | Yesterday at 7:00 AM | Yesterday at 07:00 |
| `compact`, for a list row | 9:41 AM today, otherwise the relative day | 09:41 |
| `utc_offset` | UTC+9:30, UTC-3:30, UTC | the same |
| `format_duration` (a free function) | 45 s, 12 min, 2 h 10 min, 1 day 4 h | the same |

With month before day (United States English) the dates read "Apr 14",
"Apr 14, 2025" and "Tuesday, April 14, 2026". `TimeZone::name` gives the
zone's IANA name for saying which zone is in use, and `civil` gives an
instant's wall-clock fields and offset for application logic such as
grouping by day.

Days are calendar days on the zone's wall clock: a 23- or 25-hour day at a
daylight-saving change is still one day, a repeated local time keeps its own
offset, and a skipped one never appears. Inject a fixed `Clock`, a named
zone and an explicit `TimeLocale` in tests and gallery evidence; never depend
on the host's zone, locale or clock.

**Platforms.** The browser converts with `Intl.DateTimeFormat` and the
native build with the system's time zone database through `jiff`
(`/usr/share/zoneinfo` on Linux and macOS, and `jiff`'s bundled copy on
Windows); `jiff` is not part of the web build. A Linux host or container
without zoneinfo refuses every named zone and treats the viewer's zone as
`Etc/Unknown`, which behaves as UTC.
Stobie writes the text itself from the wall-clock fields, so the web and
native builds give the same text for the same zone, locale and instant, and
browser engines can differ only in their zone data and in how they resolve
a locale. Shared fixtures in
`crates/stobie-ui-egui/src/local_time/fixtures.json`, across daylight-saving
changes in Adelaide and New York, half- and quarter-hour offsets, local mean
time and the ends of the range, are checked natively by the unit tests and
in Chromium by `tools/local-time-browser.mjs` (gallery-browser stage), which
also checks that the viewer's zone and locale are the page's.

**Locales.** A `TimeLocale` is a clock (`HourCycle::Twelve` or `TwentyFour`)
and a `DateOrder` (`DayMonth` or `MonthDay`); `with_hour_cycle` applies a
person's own choice of clock. The words ("Today", weekday and month names,
"AM") are English in every locale, like the rest of Stobie's text; Stobie
does not yet translate. In the browser, `TimeLocale::viewer()` and
`for_language_tag` read the clock and the order of a numeric month and day
from `Intl`. Natively, the viewer's locale is the first of `LC_ALL`,
`LC_TIME` and `LANG` (a desktop application on macOS or Windows usually has
none of them, and so reads as United States English whatever the system
setting), and a tag resolves through a table that matches
Chromium for 45 tags (checked by the browser test); an unlisted tag takes
its language's entry, and an unlisted language a 24-hour clock with day
before month. Known differences:

- A tag the table does not list can resolve differently from the browser,
  such as `en-DE` (the `en` entry natively, a 24-hour clock in Chromium), and
  Unicode extensions such as `-u-hc-h23` are honoured only by `Intl`.
- Locale conventions a `TimeLocale` does not model are not followed: "am" in
  `en-AU`, "a.m." in `en-CA`, "9 h 41" in `fr-CA` and year-first dates all
  read as above.
- The viewer's zone name in the browser can be an older alias (Chromium
  reports `Asia/Calcutta` for `Asia/Kolkata`); a named zone keeps the name it
  was given on both platforms.
- A platform zone without an IANA name, such as a POSIX `TZ` rule, is named
  `Etc/Unknown`.

Each format in the browser makes one `Intl` call, or two where the text
depends on today, and `LocalTime::viewer()` reads the
platform each time it is called, so keep a `LocalTime` and replace
it when the zone or locale may have changed, such as when the application
returns to the foreground. Times shown relative to now ("Today", "in 2 h")
change as time passes: schedule a later pass with
`Frame::request_repaint_after` for the next change rather than repainting
every frame.
