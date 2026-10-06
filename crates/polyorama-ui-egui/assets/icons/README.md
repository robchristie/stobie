# Polyorama icons

These 30 geometric SVGs are original Polyorama project artwork, authored for
this repository under its [Apache-2.0 licence](../../../../LICENSE). No third-party
paths, icon fonts, branded artwork or attribution dependencies are included.

SVG is the artwork authority: a 24×24 view box, two-unit monochrome strokes,
round caps/joins and deliberate optical insets. Absolute `M`, `L`, `Z` paths and
circles are the complete supported subset. Filled circles are used only for dots.
The component size is the `icon.size` token (16 points, scaled with font
preferences); artwork coordinates and stroke proportions are independent of
control hit geometry. Colours come from the owning theme/control.

Run `python3 tools/generate-icons.py` after editing artwork. It validates the
closed subset and generates static Rust geometry; the drift regression runs in
`cargo xtask verify`. Native and WASM use the same egui vector painter, with no
loader, runtime SVG parsing, font dependency, network access or raster asset.
Round stroke caps and joins are painted by the shared renderer. Curves should
be added only after a required concept and a representative visual probe justify
extending this bounded compiler.
