# Tokens

[`design/tokens/polyorama.tokens.json`](../../design/tokens/polyorama.tokens.json)
is the authored analytical reference token source. It supports the repository's bounded
DTCG-style subset: typed scalar leaves, complete aliases, four named theme
variants and two density variants. Runtime UI selects generated
`ThemeVariant`/`DensityVariant` values and consumes typed `DesignTokens`; it
does not look up token strings, parse JSON per frame or invent component-local colours,
spacing, radii or font sizes.

Edit the source, then run:

```sh
cargo xtask tokens generate
cargo xtask tokens check
```

Commit the deterministic generated Rust when it changes. `tokens check` is the
drift gate. Do not edit generated values by hand.

The visual grammar is canvas → panel → raised, with primary/muted text,
selection, an independent focus ring, and status colours used only for status.
The spacing unit is four points. Compact and comfortable retain the same
component vocabulary; font scale remains bounded to 100–150%. Theme and
contrast are orthogonal: high contrast is authored light/dark output, not an
inversion. Consult the [design language](../design-language.md) before adding
a token; a token is justified by a stable semantic role, not one-off styling.

Applications may instead own a complete `ThemeColours` definition, validated
once by `ApplicationTheme::new`. Resolve custom tokens with `theme.resolve`
and apply native styles with `apply_design_system_with_theme` using matching
preferences and typography. The gallery's **Appearance workbench** previews
three authored identities on the selected production story, switches back to
the analytical reference for comparison, and edits nine bounded colour roles.
Primary and muted text must meet 4.5:1 in regular modes and 7:1 in high-contrast
modes on canvas, panel, raised, hover, selection and quiet-hover backgrounds.
Invalid edits retain the last accepted preview and disable export. **Copy theme
JSON** exports the typed `ThemeColours` source; check it into the consuming
application and parse/validate it once. It does not approve snapshot baselines.

The unchanged analytical reference retains historical muted-text state contrast
that fails strict `ApplicationTheme::new` validation. The workbench labels this
compatibility status and the export's required import route. Its exact JSON
round-trips through `ApplicationTheme::from_analytical_colours`, which accepts
only the complete original palette, with no changed roles. This exception is
not available for edited reference colours; they must pass `ApplicationTheme::new`
in all four modes. An invalid reference edit retains the original preview and
disables export until repaired or reset. Prefer an authored application preset
as the starting point for strict theme editing.
