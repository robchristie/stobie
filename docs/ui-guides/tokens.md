# Tokens

[`design/tokens/polyorama.tokens.json`](../../design/tokens/polyorama.tokens.json)
is the only authored token source. It supports the repository's bounded
DTCG-style subset: typed scalar leaves, complete aliases, four named theme
variants and two density variants. Runtime UI selects generated
`ThemeVariant`/`DensityVariant` values and consumes typed `DesignTokens`; it
does not look up token strings, parse JSON per frame or invent local colours,
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
