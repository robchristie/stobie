# Frozen UI evaluation seed

This is a deterministic seed for evaluating an agent or change against the
current component gallery and semantic contract. The task IDs, fixture story
IDs, viewport, variants and pass assertions are frozen. Do not solve a task by
changing its fixture, assertions or score thresholds; propose a new versioned
seed instead.

The canonical machine-readable source is
[`ui-evaluation-seed.json`](ui-evaluation-seed.json). Commands should use
explicit output directories and retain their semantic snapshot, text audit and
selected image evidence. A task may use the existing gallery fixture but must
not assume that a screenshot alone proves semantics or interaction.

## Scoring

Score every rubric dimension from 0 to 2:

| Score | Meaning |
| --- | --- |
| 0 | Missing, contradicted by evidence, or the required check was not run. |
| 1 | Partially demonstrated: the check ran, but a required assertion, variant or evidence artefact is missing. |
| 2 | Fully demonstrated by retained machine-readable evidence and the required visual/interaction proof. |

The total is the sum of the four dimensions: visual/text contract, semantic
contract, interaction contract, and verification evidence. Each task therefore
scores 0–8; the normalised task score is `total / 4`, yielding 0–2. A seed run
reports the mean normalised score over all six tasks (0–2). A task is passing
only when every dimension scores 2; a seed run is passing only when all tasks
pass. This deliberately prevents a strong screenshot from compensating for a
missing keyboard or semantic check.

## Task index

| ID | Frozen fixture | Primary proof |
| --- | --- | --- |
| `ui-seed-01` | `button/keyboard-focus` | focus, action semantics and keyboard activation |
| `ui-seed-02` | `tabs/many-long-labels` | measured ellipsis and semantic full titles |
| `ui-seed-03` | `tabs/narrow` | whole-target overflow and roving keyboard navigation |
| `ui-seed-04` | `splitter/hover-active` | current geometry plus drag and keyboard adjustment |
| `ui-seed-05` | `status/error-long-message` | wrapped error text and high-contrast readability |
| `ui-seed-06` | `virtual-grid/partial` | bounded, virtualised loading/partial/error observations |

See the JSON for exact variants, required assertions and evidence artefacts.
