# Polyorama design language

Status: increment 5 gallery implementation candidate

## Visual thesis

Polyorama is a precise analytical instrument: dense but not cramped,
technically capable without looking unfinished, and visually quiet enough that
scientific imagery, data and current selection remain dominant. Its chrome is
matte and planar, its accent is cool cyan-teal, and hierarchy comes from
surface tone, spacing and typography before borders or decoration.

The operator goal is to inspect linked scientific views, results and
annotations without losing selection, camera, tool or worker provenance. The
application bar owns global lifecycle actions and status. The dock owns layout;
each pane owns its content and scrolling.

## Surface and colour hierarchy

The semantic surface sequence is `canvas` → `panel` → `raised`. Canvas is the
quiet workspace ground, panel holds persistent pane or application chrome, and
raised is reserved for transient or selected foreground content. Do not add a
card merely to group related content; use spacing, alignment or a divider.

Text has `primary` and `muted` roles. Muted text still carries useful state and
must meet the same body-text contrast target. `accent.primary` identifies the
current action or state; `selection.background` preserves readable primary
text. `focus.ring` is visually independent of selection. Success, warning and
error communicate status, never category or decoration.

Scientific overlays use the separate `overlay.annotation`,
`overlay.selected`, `overlay.vertex` and `overlay.footprint` roles. These
high-chroma foregrounds are authored for variable raster imagery and therefore
do not follow panel text colours when the application theme changes.

Light and dark are equal authored modes. High contrast is an orthogonal
preference with a light and dark result, not a colour inversion. Authored
foreground/background text pairs target at least WCAG 2.2 AA 4.5:1 in standard
modes and 7:1 in high-contrast modes. The token compiler tests primary and
muted text on core surfaces, text on selection, and text on the accent.

## Geometry, density and hit targets

The spacing unit is four points. Comfortable density uses a 38-point
application bar, 28-point visual controls and 7×5-point inline/block rhythm.
Compact density uses 32-point application chrome, 24-point visual controls and
5×3-point rhythm. Both densities use the same component and action vocabulary.

Visual geometry and interaction geometry are distinct. A compact control may
paint at 24 points while retaining at least a 32-point hit target; pointer and
keyboard affordances must not shrink simply because information density rises.
Corner radii are restrained at three points. Components may increase their
allocation for 125% or 150% font scale, but must not silently reduce hit size.

## Semantic typography and overflow

Typography is role-based. `body` is the normal reading and data label role at
13 points and weight 400. `label` is the short chrome or control role at 12.5
points and weight 600. Both use the application UI font selected through egui;
the generated `FontWeight` type records semantic weight intent while egui owns
the installed font family and all production measurement.

Default alignment and overflow are semantic:

| Content | Alignment | Lines | Default overflow |
| --- | --- | --- | --- |
| Names, labels and prose | Start | One in chrome; multiple in content | Truncate in chrome with semantic full text; wrap in content |
| Numeric result columns | End | One | Truncate only after preserving sign, unit and semantic full text |
| Status and errors | Start | Wrap when persistent | Wrap, or move detail to diagnostics when chrome is narrow |
| Tabs | Centre | One | Truncate, then move tabs to an overflow control |
| Toolbars | Start | One | Collapse labels or move lower-priority actions to overflow |

Every reusable component must eventually declare alignment, line count,
minimum useful width, semantic full text and one of: scroll, wrap, truncate,
collapse, move to overflow controls, or a deliberate minimum state. Character
count is never a text-width proxy.

The measured-text layer exposes the bounded roles `application_title`,
`pane_title`, `section_heading`, `body`, `secondary`, `caption`,
`tabular_value`, `monospace_technical`, `button_label`, `tab_label`, `status`
and `error`. Each role resolves its font family, size, weight intent and colour
from generated tokens. Egui galley layout is the production measurement
authority; role names never become runtime stylesheet selectors.

Implemented overflow policies are `ellipsis`, `wrap`, `clip`, `scroll` and
`expand`. Ellipsis and bounded wrapping use egui's `Galley::elided` as the sole
truncation signal. Clip and scroll preserve intrinsic layout and explicitly
permit content bounds outside the local allocation because the owning clip or
scroll surface constrains visible paint. Expand requires the useful allocation
to contain the measured text. All policies retain full semantic text.

Polyorama-owned text components may emit a bounded `TextLayoutObservation`
with a typed stable component and parent ID, role, alignment, allocation,
layout paint bounds, clip, declared overflow and line limit, actual line count
and truncation state. Egui 0.36 has no reliable public baseline metric, so the
baseline is reported as unavailable rather than inferred. The deterministic
audit uses a one-point tolerance for raster/layout rounding and rejects invalid
useful geometry, undeclared out-of-bounds text, unexpected lines, undeclared
truncation, alignment deviation and overlapping sibling text. Observations are
concentrated on Polyorama components; they are not a second UI tree and do not
enumerate ordinary egui labels or virtualised collections.

An empty text audit means **every observed Polyorama text component passed**,
not that every visible string in the frame was structurally audited.
`TextAuditCoverage` accompanies current snapshots and retained text evidence:
`measured_components` counts distinct observed component IDs;
`native_text_controls` counts explicitly recorded native control responses in
that viewport's current layout pass; `observed_native_controls` counts those
whose internal text has structural observations (currently zero).
`excluded_categories` always names ordinary egui labels, including headings and
hover text, and names each unobserved native text category used in that pass.
Missing or null coverage means unavailable, not zero controls.

Native combo boxes, radios, sliders and selectable options are recorded at their
recipe or application call sites. The denominator includes submitted clipped
controls, gallery chrome and open popup options, but not closed popup options
or virtual items that were never instantiated. A native control is counted once
by response ID, not once per internal string. These counts describe bounded
instrumentation, not a census of all visible text. For example, the preferences
control contributes nine radios and one slider, with zero structurally observed
native controls. Its ordinary field labels remain excluded. Native egui layout,
exact gallery snapshots, AccessKit and keyboard tests provide complementary
protection; semantic or keyboard coverage must not be counted as text-layout
coverage. Native widgets need not be replaced to increase an observation score.

Dock tabs are the first migrated component. Their desired width and ellipsis
layout are measured by egui, their label painter is strictly clipped, stable
widget IDs derive from `PaneId`, and pane drag/activation behaviour retains the
canonical workspace path. Widget semantics
retain the complete title, and a tooltip appears only when the galley is
elided. A dock tab paints compact geometry but interacts through a token
minimum-hit rectangle; it exposes an AccessKit `Tab` role, full title,
selected state and author ID, with a token-coloured focus ring. Focused tabs
lock horizontal arrows against egui's spatial focus navigation and support
Left, Right, Home and End roving activation plus Enter and Space. Input is
interpreted before tab paint and AccessKit emission so the selected tab,
focused target and shown pane agree in the activation frame, including when a
previously hidden overflow tab becomes active.

Responsive width classes cap desired tab width. Measured titles first truncate
within whole targets down to the minimum hit width. When all tabs still cannot
fit at that minimum, only whole targets around the active tab are exposed and
an explicit, token-coloured overflow trigger reaches every pane. A strip too
narrow for any minimum-width tab is a deliberate overflow-only minimum state;
the trigger remains the sole non-overlapping target. Dock splitters retain a
five-point visual divider with a centred token minimum hit rectangle, semantic
`Splitter` role and current fraction, and token focus ring. Focused arrow keys
and AccessKit increment/decrement actions use the same exact 0.05 fraction
step, lock their adjustment axis against spatial focus navigation, and retain
the single completed `ResizeSplit` command path. Splitters use
stable `DockNodeId`-derived widget IDs and egui's click-and-drag arbitration so
nearby pane surfaces cannot steal the boundary gesture. Delayed drag
recognition reconstructs the press origin from total drag delta, keeping the
preview and final command faithful to the complete pointer movement.

These are compatible egui AccessKit tree updates, not a claim of delivered OS
or browser screen-reader support: the application has not enabled eframe's
native adapter and its current web integration discards those updates. The
future semantic snapshot and action registry remain separate work.

## Icons

Icons use a bounded typed `IconId` vocabulary owned by the UI component layer;
arbitrary Unicode glyphs and runtime icon-name strings are not accepted. The
initial vocabulary should cover global actions, pane tools, disclosure,
status, overflow and directional movement. Prefer project-authored geometric
SVG paths under Apache-2.0. Any third-party icon set requires a checked-in
licence and attribution record before its paths enter the generated or source
tree. The later component increment owns the first icon implementation.

## Motion

Motion explains continuity, focus movement or a state transition. The quick
duration is 120 ms; repeated ambient animation and ornamental movement are
out. `MotionPreference::Reduced` reduces framework animation time to zero and
future custom components must select their reduced-motion path explicitly.
Motion never creates an unconditional repaint loop.

## Responsive panes

Pane behaviour is classified independently on each axis:

- width: narrow below 360 points, regular from 360 to 719, wide from 720;
- height: shallow below 280 points, regular from 280 to 599, tall from 600.

Narrow panes preserve the primary action and current state, collapse secondary
labels, move low-priority actions to overflow and keep scrolling local. Shallow
panes reduce vertical chrome before content. Wide or tall panes may reveal
secondary context but must not materialise complete result or thumbnail
collections. Exact breakpoints are typed component policy, not token aliases;
later gallery stories and physical checks will calibrate them.

`PaneWidthClass`, `PaneHeightClass` and `PaneSizeClass` encode these exact
breakpoints. The gallery exercises narrow and regular/wide reference recipes;
complete application migration remains later work.

## Token source and supported subset

[`../design/tokens/polyorama.tokens.json`](../design/tokens/polyorama.tokens.json)
is the single machine-readable source. It uses a deliberately bounded,
DTCG-style JSON subset:

- nested JSON objects form groups and dot-separated token paths;
- every base leaf has `$type` and `$value`, with optional `$description`;
- supported scalar types are `color`, `dimension`, `number`, `duration`,
  `fontSize` and `fontWeight`;
- colours are `#RRGGBB` or `#RRGGBBAA`; dimensions and font sizes are finite
  logical points; duration is a non-negative whole number of milliseconds;
  font weight is a whole number from 1 to 1000;
- an alias is the complete string `{path.to.token}` and must preserve type;
- `$themes` contains exactly `light`, `dark`, `light-high-contrast` and
  `dark-high-contrast`; `$densities` contains exactly `compact` and
  `comfortable`;
- an override leaf has `$value`, optional `$type` for an exact type assertion,
  and optional `$description`; it must reference an existing base token; and
- theme overrides are restricted to `colour.*`, density overrides are
  restricted to `spacing.*` and `geometry.*`, and typography/motion are common
  across variants in this increment; then aliases are resolved for the complete
  combination.

Arrays, composite DTCG values, group-level type inheritance, gradients,
shadows, typography composites, runtime extensions and other `$` constructs
are unsupported. The compiler reports the responsible group, token or variant
and rejects unknown constructs, types, missing references, type mismatches,
cycles, malformed colours, non-finite/out-of-range numbers and variant sets.
All numeric values must also remain finite after conversion to the generated
`f32` representation.

Run `cargo xtask tokens generate` after editing the source. It validates all
eight theme/density combinations and deterministically writes checked-in typed
Rust. `cargo xtask tokens check` validates and rejects drift; it is also part of
`cargo xtask verify`. Normal native and Wasm compilation reads only the
generated Rust—there is no network access or per-frame JSON parsing.

Generated values are exposed as `DesignTokens` with typed colour, point,
ratio, weight and duration fields. Runtime UI code selects `ThemeVariant` and
`DensityVariant`; it never requests a token by string. Increment 2 applied the
first application-bar recipe and increment 3 applies typography, colour and
spacing tokens to dock-tab text. Broader pane and control migration waits for
reusable component increments.

## Gallery and reference recipes

`polyorama-gallery` is a native and browser application, not a second widget
implementation. Its typed Rust catalogue has 18 stable story IDs and fixed
metadata for description, component group, recommended viewport, applicable
appearance/density variants and interaction scenarios. It supports the four
light/dark and standard/high-contrast combinations, both densities, 100%, 125%
and 150% font scale, and narrow/regular/wide story surfaces. Verification uses
a representative matrix rather than a needless full Cartesian snapshot suite.

The gallery calls the same production `dock_workspace`, application bar,
measured action button, property row, result row, status badge and thumbnail
cell recipes available to application code. Reference data is fixed and
bounded: no clocks, randomness, workers, complete result rows or complete
thumbnail collections participate. Action buttons, result rows and thumbnail
cells expose full role/name/state/click semantics, stable author IDs, visible
focus and measured labels; the application-owned `ActionKey` contract remains
increment 6.

Every typed story renders headlessly through the production recipes at four
representative configurations. Its `TextLayoutObservation` values must pass
the strict audit. Native and browser launch smokes additionally retain selected
screenshots, the serialised manifest and current Rust-owned story snapshot. The
gallery never requests a frame unconditionally; a warmed browser frame counter
must remain unchanged while idle. Strict expected/actual/diff baseline
management and `cargo xtask ui` commands remain increment 7.

## Preferences and licence

`UiPreferences` is presentation state, separate from the document and session.
Appearance, contrast, density, bounded font scale and motion are orthogonal.
Schema version 1 persists with the application state; missing fields and
unknown current values fall back independently, obsolete schema versions reset
predictably, and font scale is clamped to 100–150%. System appearance resolves
against egui's current system theme.

The token source, generator, generated Rust and component recipe are Polyorama
project code under the repository's Apache-2.0 licence. They contain no
third-party design assets.
