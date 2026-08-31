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
fixture and states the bounded meaning of a pass. Missing coverage or counts
inconsistent with the observations fail verification. Empty findings mean
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
