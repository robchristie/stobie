# Stobie icons

These 35 geometric SVGs are original Stobie project artwork, authored for
this repository under its [Apache-2.0 licence](../../../../LICENSE). No third-party
paths, icon fonts, branded artwork or attribution dependencies are included.

SVG is the artwork authority: a 24×24 view box, two-unit monochrome strokes,
round caps/joins and deliberate optical insets. Paths use absolute `M`, `L`,
`Q` (quadratic Bézier), `C` (cubic Bézier), `A` (elliptical arc, unrotated)
and `Z` commands; with circles they are the complete supported subset.
Relative and shorthand commands and rotated arcs are refused. Filled circles
are used only for dots.
The component size is the `icon.size` token (16 points, scaled with font
preferences); artwork coordinates and stroke proportions are independent of
control hit geometry. Colours come from the owning theme/control.

Run `python3 tools/generate-icons.py` after editing artwork. It validates the
closed subset and generates static Rust geometry; the drift regression runs in
`cargo xtask verify`. Native and WASM use the same egui vector painter, with no
loader, runtime SVG parsing, font dependency, network access or raster asset.
Round stroke caps and joins are painted by the shared renderer. The compiler
turns quadratics and arcs into cubic Béziers (an arc in pieces of at most a
quarter turn) and refuses any curve whose computed control points leave the
view box; a path of straight lines alone keeps its polyline form. The
painter flattens each cubic with egui's tolerance at the artwork's size.
`attach`, `mic` and `user` are drawn with arcs.
