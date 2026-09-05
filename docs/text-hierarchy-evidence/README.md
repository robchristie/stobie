# Typography and bounded text calibration

The selected candidate implements the dense tool scale and an optional reading
profile with real Source Sans 3 regular and semibold faces, content-sized labels,
explicit fixed slots, and observable failed layout requests. The complete
question, probe, exit condition, fixture identities, results and decisions live
in [calibration.json](calibration.json). The runtime source and browser artefact
identities are in [source-manifest.json](source-manifest.json); this is a
calibration checkpoint, with final exact-head qualification owned by the
coordinating landing change.

The smallest representative visual probe uses `typography/dense` and
`typography/reading`, with fixed content and component IDs 700–706. Four
configurations cover dark 100% dense, dark 100% reading, light 125% reading and
narrow high-contrast 150% reading. The accepted captures use the canonical
`tools/ui-capture.mjs` headful Xvfb/WebGPU SwiftShader path, including its
spatial-pixel-variation assertion. The initial headless Lantern probe had valid
snapshots but black screenshots; that visual evidence was rejected.

The inspected candidate has a visible title, pane-title, section, body and
metadata hierarchy. Semibold is a different bundled face with different
measured metrics, not merely a recorded weight intent. Muted metadata remains
readable, and enlarged narrow text wraps inside the declared allocation. Native
semantic headings use the same resolved font, line height and emphasis.

| Accepted visual | Treatment |
| --- | --- |
| [Reading at 100%](reading-dark-100.png) | 21-point title, 18-point pane title, 15-point section, 14-point body and 12.5-point metadata |
| [Reading at 150%, narrow high contrast](reading-high-contrast-150-narrow.png) | The same roles scaled together, with bounded wrapping |

Each retained text JSON has all seven required components, zero findings and
attempted/successful/failed counts of 7/7/0. Native controls remain explicitly
outside structural text measurement. The gallery semantic regression asserts
the complete title, heading, reading and metadata strings, including the native
heading, so an omitted component cannot pass through an empty observation set.

The component regression checks a one-line string with a two-line allowance
against a deliberate two-line slot at 100%, 125% and 150%. It also submits
`max_lines = 24`, asserts `InvalidMaxLines(24)`, inspects actual fallback paint
and widget semantics, verifies the audit failure and proves that a later
observation filter cannot erase its failed-attempt count. This is the library
bounded-label contract; the consumer owns a scrollable evidence reader and its
long-fixture end-content regression.

The [bundled-font record](../../crates/polyorama-ui-egui/assets/fonts/README.md)
retains the upstream revision, SHA-256 identities, copyright and SIL Open Font
Licence. Defaults retain egui's existing proportional fallback font chain.

Focused tests, workspace tests, relevant Clippy, token drift checking, the
release native gallery build and release WASM build passed. `cargo xtask verify` also passed formatting, lint, workspace tests, architecture,
release native/WASM builds, both browser smokes, all five deliberately inspected
snapshot baselines and both native smokes. Snapshot semantic identities, names,
roles, actions and enabled states were preserved. Exact review and landing
identities belong in the owning pull request; these calibration files do not
replace those gates.
