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
Invalid edits retain the last valid preview and disable export. **Copy theme
JSON** exports the typed `ThemeColours` source; check it into the consuming
application and parse/validate it once. It does not approve snapshot baselines.
