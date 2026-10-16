# Emuella viewer

The Emuella image viewer, its regional source and tools crates, its tools and
its documentation moved to
[robchristie/emuella-viewer](https://github.com/robchristie/emuella-viewer) on
2026-10-16. That repository depends on Stobie through a pinned git revision and
builds for the web with `stobie build-web`, like any application outside this
repository.

The documents that lived here are in that repository's `docs/`: the service,
calibration, NITF preparation and qualification guides (`emuella-viewer-*`),
the viewer acceptance reports and receipts (`viewer-acceptance-*`,
`viewer-native-*`), real-scene viewing (`real-scene-*`) and representation
efficiency (`representation-efficiency-*`). Their raw evidence stays in
stobie-evidence; see the [evidence archive](evidence-archive.md).

Stobie keeps the framework contracts the viewer uses: the
[regional adapter contract](regional-adapter-contract.md), the image layer
crates and the [browser startup loader](browser-startup.md).
