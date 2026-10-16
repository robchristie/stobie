# Stobie documentation

Start with the guides. Plans, reports and evidence below them record how
features were built and qualified; read them for history and rationale, not to
learn the current API.

## Build an application

- [Start a new app](start-a-new-app.md): `stobie new`, building natively and for
  the web, and pinning Stobie.
- [Compose a Stobie application](application-composition.md): the
  `Application` trait, actions, panes, state and the compiled `notes` example.
- [Compose an image application](image-application-composition.md): the
  optional imaging layer, tiles, the runtime and the renderer.
- [UI guides](ui-guides/README.md): components, panes, interactions,
  accessibility, tokens and UI review.
- [Design language](design-language.md): visual and semantic contracts.
- [Production browser builds](browser-startup.md): web packaging, WebGPU and
  WebGL2 selection, and startup measurement.
- [Regional source adapter contract](regional-adapter-contract.md): external
  regional image sources for the imaging layer.

## Operate and inspect applications

- [The `stobie` command](agent-cli.md): launch or attach to an application,
  read its UI and invoke its actions from a shell.
- [Inspect and exercise an application](application-interface.md): the
  inspection protocol behind the command.

## Contribute

- [Contributing](../CONTRIBUTING.md): crate boundaries, verification, Git
  history and evidence rules.
- [Plan lifecycle](plan-lifecycle.md): how plans in [plans/](plans/) are
  written, closed and checked.
- [CI verification and cache ownership](ci-cache.md): verification stages and
  focused reproduction.
- [Evidence archive](evidence-archive.md): where raw evidence lives, and the
  history rewrites.
- [UI evaluation seed](ui-evaluation-seed.md): frozen tasks for repeatable UI
  evaluation.
- [Private development preview](private-preview.md): a managed HTTPS preview of
  the Lab.

## Records and history

- [Plans](plans/): one execution plan per delivered piece of work, all
  complete unless marked active.
- Reports and qualification evidence:
  [accessibility integration](accessibility-integration-report.md),
  [application composition](api-documentation-evidence.md),
  [application interface](application-interface-evidence.md),
  [browser startup results](browser-startup-results.md),
  [iOS PWA spike](ios-pwa-spike-report.md),
  [navigation items](navigation-item-evidence.md),
  [Record Desk](record-desk-evidence.md),
  [status chips](status-chip-evidence.md) and
  [verification friction](verification-friction.md).
- The original vertical slice: [goal](vertical-slice-goal.md) and
  [report](vertical-slice-report.md).
- The design-agent loop: [goal](design-agent-loop-goal.md),
  [baseline](design-agent-loop-baseline.md) and
  [report](design-agent-loop-report.md).
- The `*-evidence/` folders hold the small receipts and summaries those
  reports cite.
- [Project rename](project-rename.md) from Polyorama, and the
  [Emuella viewer](emuella-viewer.md), which now has its own repository.
