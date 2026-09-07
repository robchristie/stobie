# UI review

Review UI changes as observable behaviour, not as a screenshot-only opinion.
Start with the smallest affected Rust test, then render the relevant fixed
gallery story and inspect its semantic and text observations. Use the current
`cargo xtask ui` surface for list, render, inspect and text audit operations;
write output to an explicit disposable directory and never update a baseline
as part of review.

The exact command forms and baseline policy are documented in the
[deterministic snapshot guide](../ui-snapshots/README.md). The canonical
verification command is `cargo xtask ui verify --output-dir <path>`; it has no
baseline-update option and emits a complete failure bundle on drift.

## Design review contract

Before a material presentation change, add a short contract to its existing
plan: the representative user task; the intended information and action
hierarchy; the reference appearance or artefact being compared; and the
required normal, narrow, loading, empty, partial, error, selected, disabled
and keyboard-focus states that apply. State deliberate exclusions. The contract
describes the proposed experience, so it is not inferred from an approved
baseline or from a passing machine check.

Review the candidate on three independent judgements with evidence:

- **Behavioural correctness:** the declared task completes through the current
  pointer, keyboard and semantic targets, and state changes, recovery and
  unavailable actions behave as contracted.
- **Presentation correctness:** measured text, hierarchy, layout, responsive
  states, contrast and component states visibly match the contract and retain
  the required semantic content.
- **Design quality:** the hierarchy helps the user find the next action and
  understand status, the reference is used intentionally, and any new visual
  direction serves the task rather than decoration.

Do not combine these judgements into a score. The frozen evaluation seed's
machine scoring remains a regression check for its own tasks; it cannot turn a
design judgement into acceptance or compensate for an unresolved finding.

Use a bounded two-pass critique for a material candidate. First, inspect the
task, reference and candidate without reading the implementation rationale and
record what is noticeable first, what competes unnecessarily and what became
harder. Then read the rationale and decide whether it
answers those observations or requires a revision. Permit one revision and one
re-critique; after two inconclusive rounds, retain the competing evidence and
ask for the relevant human preference. Do not manufacture further visual
variation to force agreement.

For routine presentation changes, capture the existing reference beside the
candidate and, where the work deliberately changes direction, one bounded
alternative that demonstrates the proposed hierarchy. Review both the ordinary
experience fixture and an adversarial qualification fixture: long or localised
text, narrow width, high contrast or font scale, and loading, partial, error or
disabled state as applicable. The ordinary fixture proves the intended working
experience; the adversarial fixture probes its limits and does not become the
default visual target.

Record the decision compactly with the semantic evidence owner:

```text
Task and reference:
Behavioural checks: passed / failed / unavailable
Presentation checks: passed / failed / unavailable
Design decision: accepted / changes requested / pending
Main reason and remaining trade-off:
Exact candidate evidence links:
```

Let linked machine provenance identify the revision, fixture, viewport, theme,
font and artefact hashes; do not transcribe those fields into several documents.
Machine artefacts do not supply the design rationale. Keep this record separate from baseline approval: approving
a reviewed snapshot establishes regression comparison only, while design
acceptance establishes that the contracted experience is suitable. A mechanical
refactor must instead demonstrate visual equivalence against the approved
reference at affected fixtures; it does not require a palette or direction
exercise.

## Required review questions

- Does the component use typed tokens and egui measurement, with no unmanaged
  visual literals or character-count sizing?
- Are overflow, alignment, line limits, full semantic text and narrow behaviour
  declared and visible in the appropriate long-text or narrow story?
- Does `UiSnapshot` contain stable current geometry, correct role/name/state,
  action and pane/domain references without enumerating virtualised data?
- Do AccessKit parity and text-layout audits have no unexplained findings?
- Does text evidence retain coverage counts and exclusions? An empty text audit
  certifies only observed Polyorama components, not every visible string. Check
  native response recording when adding a control or popup option.
- Do pointer, keyboard and—where relevant—physical native/browser paths use
  the same current semantic target?
- Does the warmed UI remain event-driven, with no unconditional repaint?

Use the catalogue's fixed story IDs and recommended viewports. Review at the
changed story's documented theme/density/font-scale state, and add a focused
story when the defect cannot be represented by an existing frozen fixture.
For a material UI change, finish with the relevant native and Wasm build and
the canonical `cargo xtask verify` before delivery. Score frozen tasks with
[`../ui-evaluation-seed.md`](../ui-evaluation-seed.md); retain command output,
snapshots and captures with the evidence owner.
