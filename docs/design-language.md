# Polyorama design language

Status: increment 2 foundation

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
the generated `FontWeight` type records intent until the measured-text
increment supplies complete recipes. Production text measurement remains
egui's responsibility.

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
`DensityVariant`; it never requests a token by string. Increment 2 applies the
tokens only to the application bar recipe. Broader pane and control migration
waits for measured text and reusable component increments.

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
