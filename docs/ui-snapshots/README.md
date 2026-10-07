# Deterministic UI snapshots

[`fixtures.json`](fixtures.json) is the versioned, closed fixture manifest for
Polyorama's selected visual, semantic and text baselines. Each fixture pins its
gallery story, viewport, data seed, appearance, contrast, density, font scale,
width class, bundled font set and pinned browser-WebGPU/SwiftShader renderer
contract.

Build the browser package once, then use an explicit ignored output directory:

```sh
cargo xtask build-web
cargo xtask ui list --output-dir .tools/runtime/ui-list
cargo xtask ui render --fixture application-shell-dark --output-dir .tools/runtime/ui-render
cargo xtask ui inspect --fixture application-shell-dark --output-dir .tools/runtime/ui-inspect
cargo xtask ui audit-text --all --output-dir .tools/runtime/ui-audit
cargo xtask ui verify --output-dir .tools/runtime/ui-verify
```

For safety, UI output must be a dedicated directory beneath the repository's
ignored `.tools/` tree. A versioned ownership marker is required before the
tool will replace a non-empty directory; source, baseline and arbitrary user
directories are rejected before any recursive cleanup.

Every command writes a versioned JSON `summary.json`; `list`, `render`,
`inspect` and `audit-text` retain their corresponding machine-readable
artefacts. `verify` compares pixels at zero tolerance and compares canonical
metadata, semantic snapshots and text observations structurally.

Each `text.json` includes `coverage` with measured component and native control
counts plus excluded categories. The `audit-text` summary retains coverage per
fixture and states the bounded meaning of a pass. Missing coverage, failed component attempts or counts
inconsistent with the observations fail verification. Attempts are recorded
independently of the visible observation subset, so filtering cannot erase a
failed request. Empty findings mean
“Every observed Polyorama text component passed”, not “Every visible string was
structurally audited”. Counts cover the submitted layout pass, including clipped
controls and gallery chrome; ordinary labels remain excluded. See the
[design language](../design-language.md) for the denominator and native-widget
boundary.

The verifier is deliberately read-only with respect to `expected/`. It has no
approval or update mode. A mismatch writes a fixture-specific bundle under
`<output>/failures/` containing expected and actual metadata, semantic, text
and visual artefacts, machine-readable diffs, a visual diff and capture logs.
When capture or comparison cannot produce an artefact, the same bundle records
that category as explicitly unavailable and retains every artefact and log
that was produced. Audit findings are serialised before the command reports
failure, so `audit-text` remains diagnostic rather than collapsing into a
capture error.
CI invokes this same verifier through `cargo xtask verify` and uploads the
ignored verification evidence when a gate fails.

Baseline changes are ordinary reviewed source changes. Generate candidate
artefacts outside `expected/`, inspect every affected visual and semantic/text
diff, and copy only the deliberately accepted files into the checked-in tree.
CI must never perform that operation.

The icon fixtures pin the production action presentations in dark comfortable
mode, the vocabulary in light compact mode, and narrow labelled actions in light
high contrast at 150%. Physical hover/press, keyboard, toggle/disabled and display
scale probes run through `tools/icon-actions-browser-smoke.mjs` and the Linux
native companion in full verification. Catalogue growth changes Gallery chrome
and native-control coverage; review those differences alongside any deliberate
artwork change before updating the original fixtures. Frozen seed tasks and their
assertions remain unchanged.

Navigation fixtures add the ordinary dark sidebar, light compact state/count
examples, and narrow light high contrast at 150%. The physical
`tools/navigation-browser-smoke.sh` and Linux native companion run through full
verification. The browser journey exercises actual Tab/Enter/Space, pointer
hover/press, consumer selection/count changes, disabled and zero-count rows,
large counts, long label/badge elision, scrolling and bounded idle behaviour.
Successful partially clipped text is measured by the recipe but omitted from
Gallery's fully visible audited subset; the pass coverage inventory still counts
those attempts and retains every failed measurement. Clipped row interaction
semantics remain visible with current bounds.
