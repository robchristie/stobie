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
badges, navigation items, list rows, composers, avatars, message bubbles,
conversation rows, unread dots, switches, copy controls, headings with controls, responsive navigation, dialogs, sheets, confirms and virtual thumbnail cells. Their deterministic examples are the
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
Space, and assistive-technology activation share one route. In the browser
every new-tab request opens with `noopener` and `noreferrer`, so the linked
page cannot navigate the application's tab: every `AppHost` and every Stobie
link component installs that protection, whether the application starts
through `WebHost` or runs eframe's web runner itself. An application without
an `AppHost` that asks for new tabs from its own code calls
`install_new_tab_links(ctx)` once.

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

## Copy

Use `PresentationContext::copy(ui, key, &CopySpec::new(text, label,
confirmation, failure))` for a button that copies a given text, such as an
invitation code, a link or a message, to the clipboard. All four strings are
the application's own words; Stobie adds none. Copying changes no
application state, so the control needs no action or intent.

```text
p.content(ui, "code", &code, ContentTextSpec { interaction: TextInteraction::Selectable, ..CODE });
p.copy(ui, "copy-code", &CopySpec::new(&code, "Copy code", "Copied", "Couldn't copy"));
```

- **Faces.** `content` shows the copy glyph and label (`IconLabel`, the
  default), the label alone (`Label`) or the glyph alone (`Icon`, named by the
  label and with the label as its tooltip); `emphasis` is any action emphasis,
  `Quiet` by default. Prefer a face with the label wherever a failure must be
  read, because an icon-only control shows a failure only as its warning
  glyph. The control is as wide as the widest of its label, confirmation and
  failure, so nothing beside it moves when it confirms. `description` adds an
  accessible description and a tooltip line; `availability` disables the
  control with a reason or hides it, taking no space.
- **Outcome.** On success the face shows a check and the confirmation for
  `COPY_CONFIRMATION` (two seconds), then returns to the label; when the
  platform refuses the write it shows a warning and the failure for
  `COPY_FAILURE` (five seconds). New text drops an earlier outcome. The
  accessible name stays the label throughout; a polite status beside the
  control, present and empty before any outcome, takes the confirmation or
  failure as its name, so assistive technology announces it once. Snapshots
  show that status only while it has a message. `CopyResponse` gives the
  button's response and the `CopyState` shown.
- **The gesture.** Pointer release, Tab then Enter or Space, an
  assistive-technology activation and an agent's invocation all copy. In the
  web host each tap, click, key press and ARIA layer click writes inside its
  own event, which Safari requires on iOS; [Interactions](interactions.md#copying-to-the-clipboard)
  owns the route. Natively, egui hands the text to the platform clipboard and
  the control confirms, because it cannot observe a failure.
- **Privacy.** The text is never published: the control is a `Button` node
  named by its label, and agents discover it as `stobie.copy-text`, whose
  target meaning is a revision (`text-revision:N`) that moves on whenever the
  text changes, never the text or a digest of it. Show the text itself, if
  the user should read it, with content text; an application that must not
  publish a secret withholds that content text and keeps the copy control.

Gallery `copy/actions` presents a copy control beside a code, an icon-only
control under a message, a long localised label and confirmation, and a
disabled control; the [snapshot guide](../../tests/ui-snapshots/README.md)
lists its fixtures.

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

A heading with controls at its end, such as a section's title with Edit,
is one `heading_row(ui, key, text, controls, availability)` call
(`Frame::heading_row` in a hosted application, which also triggers each
clicked control with its argument). Each `HeaderControl` is an action, an
`ActionTarget` or a `BoundAction`, icon-only unless built with other
content, and may be a toggle whose pressed state the application owns. The
controls keep their whole size at the row's end in the order given, and the
heading elides in the room they leave, keeping its full text for assistive
technology. When the controls would leave the heading less than its least
width (its whole text, or three minimum hit targets), they take a line of
their own below it, end-aligned, and the response says so (`wrapped`). Do
not put controls after `heading` or `content` in `ui.horizontal`: those
take the whole width they are given, which pushes what follows past the
row's end. A visible action control laid out past its row's end keeps its
semantic node, so the semantic audit reports it (an invalid rectangle once
it is wholly out of sight) rather than it vanishing from agents and
assistive technology, and debug builds mark it on screen. Gallery
`heading/controls` shows one, two and five controls beside short and long
titles at a phone's width.

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

The icon-only allocation is a square touch target (`touch_target_size`, 44
points in both densities, or the scaled control height where that is larger),
with centred square visual chrome at the control height and 16-point artwork. Artwork
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
rules; icons use the exact resolved label foreground. Pressed/selected
actions have an inset state outline separate from keyboard focus, so the on
state never relies on fill colour alone, and Primary hover changes its outer
border. Hover fills have their own colour, distinct from the selection fill
of a toggled-on action. A disabled toggled-on action keeps a dimmed selection
fill and a muted inset outline; a disabled `QuietBorderless` action stays
borderless and unfilled. Resting text-only layout remains compatible.

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

## Responsive navigation

A chat-style application's list, open item and detail are one navigation
recipe. In a hosted application:

```text
let spec = NavigationSpec {
    list_title: "Conversations",
    content: open.map(|bot| NavigationContent {
        title: &bot.name,
        reference: Some(DomainReference::External { namespace: "bot".into(), id: bot.id.clone() }),
        detail: Some(NavigationDetail { title: "About", shown, toggle: Act::Details, icon: IconId::Info }),
    }),
    back: Act::Back,
};
frame.navigation(ui, p, "bots", &spec, |ui, p, frame, pane| match pane {
    NavigationPane::List => for bot in &bots {
        frame.conversation_row(ui, p, BoundAction::new(Act::Open, bot.id.as_str()), row(bot));
    },
    NavigationPane::Content => conversation(ui, p, frame),
    NavigationPane::Detail => details(ui, p, frame),
    NavigationPane::Empty => prompt(ui, p),
});
```

`Frame::navigation` lays the panes out by `NavigationLayout::for_width` (see
[panes](panes.md#responsive-navigation) for the breakpoints), presents each
pane's header, and calls the closure once per pane shown with a child
context and the `Frame`. It triggers Back and the detail toggle itself; the
application's `perform` closes the deepest open level on Back (the detail
when shown, the item otherwise) and turns the detail over on the toggle.
`PresentationContext::navigation` is the scoped form: it returns the header
controls' responses for the caller to route, and each pane's observations,
which it has already registered.

The open item's header can also hold an avatar, a one-line subtitle such as
a status, and the item's own controls, such as Pinned, Hide and Edit
profile. Pass them as `NavigationOptions` to `Frame::navigation_with` (or
`PresentationContext::navigation_with`), which also returns each header
control's response in the order given and, in the hosted form, triggers it
with its argument:

```text
let options = NavigationOptions::new()
    .with_avatar(AvatarSpec::person(&bot.name, AvatarSize::Header))
    .with_subtitle(status)
    .with_control(HeaderControl::icon(BoundAction::new(Act::Pin, bot.id.as_str()), IconId::Check)
        .pressed(bot.pinned))
    .with_control(HeaderControl::icon(Act::EditProfile, IconId::Edit));
frame.navigation_with(ui, p, "bots", &spec, &options, |ui, p, frame, pane| /* as above */);
```

The header reads Back, the avatar, the title over the subtitle, the
controls and the detail toggle last; that is also its focus order. The
controls belong to the content's pane, its node and AccessKit region, like
the toggle. They keep their whole size and the title elides in the room
they leave; when they would leave it less than its least width (its whole
text, or three minimum hit targets), the controls and the toggle take a
second line of the header, end-aligned, so none is dropped. At a phone's
390 points, Back, an avatar, a title and three icon controls besides the
toggle fit on one line at the reference text size; give further actions
their place in the detail. Gallery `navigation/header` shows a long title
with Pinned, Hide and Edit profile.

- Destinations are a dynamic list: one row per item, each a `BoundAction`
  with the item's ID as its argument, so one action opens any of them.
  Use `Frame::conversation_row` for rows with an avatar, a preview and a
  time, or `Frame::navigation_item` with a `DestinationSpec` for plain
  destinations with an icon and an optional badge
  (`PresentationContext::bound_navigation_item` is the scoped form). Mark the
  open item's row `selected`; it is a `NavigationItem` with `aria-current`.
- Headers are at least the application bar's height. Back is an `ArrowLeft` icon
  named by the Back action's label, at the start, wherever the level it
  closes covers the one beneath: every pushed level in the stack, and the
  detail in the split. The title is a section heading that elides, and the
  content's header carries the detail toggle, an icon toggle named by its
  action's label whose pressed state says whether the detail shows. A
  hidden availability presents no control.
- Bodies: the content's, the detail's and the empty prompt's are inset
  from the pane's edges by as much as the header's controls
  (`spacing.inline`), so messages and controls keep off the screen's edge
  on a phone and the pane's edges beside other panes; the list's is not, so
  its rows fill its width. `NavigationOptions::with_body_inset(pane,
  inset)` turns either default over, such as for a full-width media pane.
- Semantics: each pane but the empty content is a `Pane` node and an
  AccessKit region named by its title, holding its header's controls and
  its body's nodes; the content's pane carries `NavigationContent.reference`,
  so agents read which item is open by querying panes. Pane bodies publish
  only what is visible, so they may scroll.
- Agents navigate with the ordinary capabilities: they observe the panes
  and the current row, open an item by invoking the open action with a
  row's argument (or, while the list is covered, with any item's ID, which
  the action's own binding takes), and go back with the Back action, which
  is invocable in every layout, shown or not.
- Keyboard focus follows the reader. Opening a level that shows Back moves
  focus that was somewhere to Back; Back returns focus to the control that
  opened the level, such as its row or the detail toggle. Agents' changes
  take no focus that nobody had. Each control is an ordinary Tab stop.
- The recipe pushes no browser history. The in-app Back is the way back;
  the browser's Back and the platform's back gesture leave the page as they
  do for any page.

Gallery `navigation/responsive` (conversation rows, messages and details)
and `navigation/responsive-narrow` (long and localised destinations with
counts, an archived one, rows below the fold) use this production API at
every Gallery width; the Desktop width shows the columns.

## Dialogs, sheets and confirms

A modal dialog holds a task the application asks for before anything else:
a form, or a question. The application's state says whether it is open; its
actions open and close it. In a hosted application:

```text
if editing {
    let spec = DialogSpec::new("Edit routine", Act::CloseEditor).sheet();
    frame.dialog(ui, p, "editor", &spec, |ui, frame, p| form(ui, frame, p));
}
if confirming {
    frame.confirm(ui, p, "delete", &ConfirmSpec::new(
        "Delete “Morning briefing”?",
        "The routine and its schedule are removed. This cannot be undone.",
        Act::Delete,
        Act::KeepRoutine,
    ));
}
```

Present it after what is behind it, in every pass while it is open.
`Frame::dialog` triggers the dismiss action when the close control, Escape
or (with `backdrop_dismisses`) the scrim dismisses it; `Frame::confirm`
triggers confirm or cancel. `PresentationContext::dialog` and
`PresentationContext::confirm` are the scoped forms, which report what
happened for the caller to route.

- **Layout.** `DialogSpec::new` is centred at every width, 560 points wide
  at the reference font scale (`width` changes it) and no wider than the
  screen allows. `.sheet()` is a bottom sheet across the screen below the
  wide breakpoint (`PaneWidthClass::Wide`, as on a phone) and centred above
  it; `.full_height()` makes the bottom sheet cover the screen, for a form
  that needs the room. A confirm is centred, 400 points wide. The title is a
  fixed header with the close control at its end; content taller than the
  room scrolls beneath it.
- **Safe areas and the keyboard.** A dialog sits inside egui's content
  rectangle, and safe areas are the web shell's: the PWA template pads the
  page by the device's safe-area insets and, with `fitViewport`, sizes it
  from the visual viewport (see [browser startup](../browser-startup.md)).
  A bottom sheet's last control therefore stays above the home indicator,
  and the sheet above an on-screen keyboard.
- **Scrim.** What is behind is covered by the canvas colour at partial
  opacity and takes no pointer input. A press on the scrim does nothing
  unless `backdrop_dismisses(true)`, so a press beside a form does not lose
  what was typed; a confirm's scrim never dismisses.
- **Confirm.** Cancel comes first and takes focus when it opens, so a stray
  Return does not confirm; Escape cancels. The confirm control is labelled
  with what it does and is primary unless `confirm_emphasis` says otherwise.
  The buttons end a row, or stack across the dialog when the row does not
  fit.
- **Work under way.** A disabled dismiss action, as while the dialog's work
  runs, closes it by no route; a hidden one shows no close control.
- **Semantics.** A dialog is a `Dialog` node named by its title, a confirm an
  `AlertDialog` named by its title and described by its message, each
  holding what it presents and marked modal for assistive technology. While
  one is presented the host observes, binds and shows assistive technology
  only it (see [interactions](interactions.md#dialogs)).
- **Fields.** A form's text fields are `NativeTextEdit`s presented through
  `PresentationContext::native`, labelled for AccessKit and observed as text
  inputs, as elsewhere; on iOS they edit through the native field.

Gallery `dialog/sheet` (a routine editor with two text fields and a switch,
over the routine it edits) and `dialog/confirm` (deleting that routine) use
this production API at every Gallery width; Narrow shows the bottom sheet.

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
let spec = ComposerSpec::new("Message", ComposerControl::icon(Action::Send, IconId::Send))
    .hint("Message Ada")
    .leading(ComposerControl::icon(Action::Attach, IconId::Attach))
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

## Chat recipes

A chat application lists its conversations, shows one conversation's
messages, and offers on or off settings. Stobie owns four recipes for that:
the message bubble, the conversation row, the unread dot and the switch.
The application owns its conversations, messages, senders, times, unread
counts and settings, and how it stores and sends them. In a hosted pane:

```text
for conversation in &self.conversations {
    let unread = (conversation.unread > 0).then(|| format!("{} unread messages", conversation.unread));
    frame.conversation_row(ui, p, BoundAction::new(Action::Open, conversation.id),
        ConversationRowSpec {
            title: &conversation.name,
            preview: Some(&conversation.latest),
            time: Some(&conversation.time), // already formatted, such as with local_time
            unread: unread.as_deref(),
            selected: self.open == conversation.id,
            avatar: Some(AvatarSpec::person(&conversation.name, AvatarSize::List)),
        });
}
frame.switch(ui, p, Action::Notifications,
    SwitchSpec { label: "Notifications", on: self.notifications });
for message in &self.messages {
    let side = if message.mine { MessageSide::Own } else { MessageSide::Other };
    p.message_bubble(ui, ("message", message.id), MessageBubbleSpec { side, sender: &message.sender },
        |ui, p| { p.markdown(ui, "text", &message.markdown, MarkdownSpec::default()); });
}
```

- **Message bubble.** `PresentationContext::message_bubble(ui, key, spec,
  render)` presents whatever `render` presents, such as Markdown or reading
  text, inside a rounded bubble. `MessageSide::Own` sits at the end of the
  line on the selection fill; `MessageSide::Other` (another person or an
  assistant) sits at the start on the raised surface; both carry a subtle
  border. A bubble is at most 85% of its column and `chat.bubbleMaxWidth`
  (560 points, grown with text to 150%), hugs shorter content and grows
  down with longer content, which wraps; `message_bubble_max_width` gives
  the bound. The bubble measures its content after presenting it and keeps
  that width for later passes, laying out once more when the content's
  text, the way it is set (density, contrast or text size) or the column
  changes. Content inside sees `message_bubble_tokens`: its surfaces are
  the fill, so links and code blocks are checked against it, and muted text
  becomes primary text on a fill too close to it. The bubble publishes a
  `Section` named by `spec.sender` (the application's words, such as "You"
  or a name) holding its content's nodes and an AccessKit group of the same
  name; the content keeps its own text kinds, so a message's words appear in
  observation as content text. A bubble shows no sender or time of its own:
  an application that wants either in view presents it beside the bubble.
- **Conversation row.** `Frame::conversation_row` presents one full-width
  row whose whole surface performs its bound action for that conversation,
  such as opening it. A `ConversationRowSpec` holds the title, one line of
  `ButtonLabel` text; an optional preview, one line of `Secondary` text; an
  optional time, one line of `Caption` text, already formatted by the
  application and kept whole up to half the title line; optional unread
  words, such as "2 unread messages", which paint the unread dot at the
  preview line's end; whether the conversation is on show; and an optional
  decorative avatar at list size. Text elides with an ellipsis. The row is
  within block padding and at least the token hit height
  (`conversation_row_height`). It publishes a `NavigationItem` named by the
  full title, `selected` while the conversation is on show (the current
  item, exposed as `aria-current`, never as a toggle), whose description is
  the unread words, the preview, the time and the action's description,
  separated by "; "; a disabled row adds its reason. The current row has the
  selection fill and indicator, and a hovered current row adds a control
  border. A hovered row has the hover surface, which every theme keeps
  apart from selection, so it never looks current. Press uses the selection
  pair with an inset outline, and keyboard focus the outer focus ring. Theme
  validation keeps muted text at its contrast floor on those fills; muted
  text becomes primary text only on hand-assembled tokens where it would
  read below 4.5:1. Each enabled row is a Tab stop; Enter, Space and a pointer
  release open it. The measurements are `ConversationRowTitle`,
  `ConversationRowPreview` and `ConversationRowTime` under a
  `ConversationRow` parent; like other control text they carry no content
  text, because the row's node holds it.
- **Unread dot.** `unread_dot_size` (`chat.unreadDotSize`, 8 points, grown
  with text to 150%) and `unread_dot_colour`, the accent where it stands
  out at 3:1 from the surface behind and primary text otherwise.
  `PresentationContext::unread_dot` allocates and paints one, and
  `paint_unread_dot` paints into a rect. The dot is decorative: the words
  beside it, such as a row's unread words, state the unread state.
- **Switch.** `Frame::switch` presents one on or off setting as a
  full-width row whose surface performs its action: the label, one line of
  `ButtonLabel` text, always in view, and a track at the row's end
  (`switch.trackWidth` by `switch.trackHeight`, 36 by 20 points, grown with
  text to 150%). On is an accent track with the thumb at the end; off is an
  outlined track with the thumb at the start, both outline and thumb in
  muted text so they stand out at 3:1. The state is shown by the track and
  thumb, never by the row's fill, so the quiet hover fill cannot be taken
  for on; a disabled switch keeps its position on decorative colours. The
  application owns the setting: activating the switch performs its action,
  which turns the setting over. It publishes a `Switch` named by the label
  whose `checked` is its state (`role="switch"` with `aria-checked` in the
  ARIA layer); its description is the action's description, and a disabled
  switch adds its reason. Measurement is `SwitchLabel`.

Low-level callers use `conversation_row` and `switch` with a stable
`PresentationId` and publish `conversation_row_semantic_node` or
`switch_semantic_node` for positive interaction bounds;
`PresentationContext::conversation_row` and `PresentationContext::switch`
handle ordinary publication, and `paint_switch_track` paints a track.

Gallery `chat/conversation` shows the list beside a conversation on a wide
column and above it on a narrower one, with unread, current and plain rows,
switches on and off, and messages on both sides holding text, Markdown, a
list and a link. Gallery `chat/long-narrow` adds long and localised names
and previews, a row without a preview, an archived (disabled) row, a long
switch label, a disabled switch, and bubbles holding a long link and code,
and scrolls. The [snapshot guide](../../tests/ui-snapshots/README.md) lists
their fixtures.

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
| `relative_day_in_sentence`, inside a sentence | today, yesterday, tomorrow, on Tuesday, on 14 Apr | the same |
| `relative_day_and_time_in_sentence` | today at 3:30 PM, on Saturday at 9:41 AM, on 14 Apr at 7:00 AM | today at 15:30 |
| `compact`, for a list row | 9:41 AM today, otherwise the relative day | 09:41 |
| `utc_offset` | UTC+9:30, UTC-3:30, UTC | the same |
| `format_duration` (a free function) | 45 s, 12 min, 2 h 10 min, 1 day 4 h | the same |

The `_in_sentence` forms continue a sentence ("Signed in today at 3:30 PM",
"Reset on Saturday at 9:41 AM"); begin a sentence, a label or a list row with
the capitalised forms. Both classify the day the same way.

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
every frame. `until_next_day` gives how long until the day in the zone
changes, which is when every relative day and every time of day that
`compact` shows changes. It is exact across a daylight-saving change, even
one that skips midnight; where the zone's answers disagree, or past
`Timestamp::MAX`, it gives at most an hour. Ask again in each pass that shows
such text, so the request follows the clock:

```rust,ignore
// In `ui`, beside a list whose rows show `local.compact(sent)`.
frame.request_repaint_after("the local day changes", local.until_next_day());
```

The repaint then has a recorded reason and runs once a day while the
application is otherwise idle. A timer that fires early finds the same day and
asks again for the few milliseconds left. A wall clock set by hand, or a sleep
that pauses timers, can make the pass late; egui also repaints on input
and when the page or window regains focus.
