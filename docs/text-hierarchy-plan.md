# Text hierarchy and attention UI

Status: active

Next action: Complete independent review and landing of the verified Polyorama candidate, then pin and qualify Bokkie against its merged revision.

## Outcome and ownership

Implement a visible semantic type hierarchy with real regular and semibold
faces, explicit content-sized and fixed-slot text geometry, and observable
layout failures. In Bokkie, organise attention around identity, reason, next
action and supporting evidence; provide a selectable scrollable evidence reader.

Polyorama owns fonts, tokens, measured/native type resolution, component layout
and text diagnostics. Bokkie owns application composition, virtual row recipes,
attention emphasis, dependency pin and long-evidence interaction proof. Both
repositories are public source; there is no separate system control plane for
this pair. This plan is the coordination record and contains only source and
synthetic-fixture evidence. No deployment, release or live-data work is included.

## Acceptance

- Semantic headings differ visibly from reading text through size, actual face,
  line height and emphasis; measured and native controls resolve consistently.
- Keep dense library defaults appropriate; evaluate Bokkie's page title at
  20–22 points, section heading at 14–16, body at 14 and metadata at 12–13.
- A short content label with a two-line maximum consumes one measured line;
  fixed-slot labels explicitly retain their requested geometry.
- Invalid requests including 24-line bounded labels produce diagnostics,
  visible fallback and failing development/qualification audit evidence.
  Coverage distinguishes attempted, successful and failed components.
- Bokkie prioritises meaningful current actions and evidence, retains technical
  identities in disclosures/confirmation, omits routine absent-error/evidence
  noise and chooses emphasis according to attention kind.
- Virtual row height follows its recipe, typography, density and font scale;
  enlarged text does not overlap or silently disappear.
- Open Raw durable evidence and prove actual content near the end of a long
  synthetic fixture is rendered and selectable through a scrollable reader.
- Run each repository's canonical checks, relevant native/WASM builds, focused
  regressions and browser/semantic evidence. Independently review each exact
  candidate and land Polyorama before pinning and landing Bokkie.

## Calibration

Question: which token-backed hierarchy and row recipe preserve dense-tool
usability while making headings and task information visibly distinct?
Smallest probe: representative headings/body/one-line wrapped labels and one
attention row at 100% and 150% scale, plus the long-evidence disclosure.
Evidence owners: Polyorama's text tests/gallery and Bokkie's UI qualification.
Exit: select one coherent candidate with measured bounds, actual font-face and
semantic evidence; reject clipping, invented whitespace and missing content.

## Delivery checkpoint

| Increment | Owner revision | Consumer revision | Result | Status | Evidence |
| --- | --- | --- | --- | --- | --- |
| Library typography/layout/diagnostics | Base `4330e1a596b71d2ed632adbbfa823ea02efbe16b` | — | Full canonical verification passes, including inspected snapshot baselines and native/browser smokes | Active | [Owner evidence](text-hierarchy-evidence/README.md) |
| Attention composition and evidence | — | Base `b2575d6` | 42 focused tests, strict lint and physical long-evidence browser journey pass; final consumer composition and qualification underway | Active | Bokkie UI qualification |

Task worktrees: `/nvme/development/polyorama-text-hierarchy` and
`/nvme/development/bokkie-worktrees/text-hierarchy`, each on
`codex/text-hierarchy`. Canonical checks are `cargo xtask verify` and Bokkie's
`tools/check.sh` plus `tools/check-ui.sh`. Polyorama probes use its owned ignored `.tools/runtime/text-hierarchy/`;
Bokkie probes use `target/text-hierarchy/`. Worktree `target` links reuse existing
repository caches, and browser dependencies/sysroot are reused without claiming
ownership. Cleanup covers only task-owned probe outputs and links, never shared
caches; its evidence belongs to landing records.
