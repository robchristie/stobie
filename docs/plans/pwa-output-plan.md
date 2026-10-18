# Installable web application output

Status: complete
Delivery: [PR #94](https://github.com/robchristie/stobie/pull/94)

Framework work item 4 of the [iOS PWA spike report](../ios-pwa-spike-report.md#framework-work):
`stobie build-web` and production packaging produce an installable web
application. The spike wrote its manifest, icons, service worker and iOS
tags by hand (`pwa/manifest.webmanifest` and `pwa/sw.js` in its local
checkout), and served them at mutable paths, because the production package
puts every file except `index.html` under `assets/<content hash>/` and adds
`<base href>`. Bokkie's iPhone client needs this output from the build.

## Decision

**The page's head is the single source of the application's identity, and
the build derives the manifest and service worker from it.** A page asks for
the output by linking `manifest.webmanifest`.

- iOS reads `apple-mobile-web-app-title`, `apple-touch-icon` and
  `theme-color` from the page, not the manifest, so those tags must be in the
  page whatever else holds the configuration. Reading the manifest's name,
  short name, colours and icons from the same tags means nothing is written
  twice. The alternatives were Cargo metadata or a manifest template file;
  both duplicate what iOS needs in the page and can drift from it.
- `stobie-build`'s `pwa` module scans the head's start tags (skipping
  comments and `script`, `style` and `title` content), validates the
  declaration and the icon PNGs (dimensions from the PNG header), and writes
  the manifest and service worker. The build fails, naming the missing tag,
  when a required one is absent. A page without the link is unaffected, so
  the Lab, Gallery, Record Desk and existing consumers package exactly as
  before, with unchanged content IDs.
- **Development** (`stobie build-web`): the files are written into `web/`
  beside `index.html`, like the startup loader, and added to the generated
  names that staging skips. The service worker caches nothing ahead and is
  network-first throughout.
- **Production** (`--production`, `stobie package-web`, the repository's
  production build): the files are written at `<app>/`, outside the hashed
  directory, and recorded in the package manifest's new optional `pwa` field.
  The page's manifest link is rewritten to `../../manifest.webmanifest`,
  because `<base href="./assets/<id>/">` resolves relative URLs into the asset
  directory. Icons stay ordinary assets, immutable and hashed; the manifest
  names them there. The service worker carries the content ID as its version
  and precaches the page, the manifest and every asset.
- **Service worker**: page network-first (any navigation in scope is the one
  page), `assets/<64 hex>/` cache-first, anything else network-first with a
  cached fallback; old versions' caches deleted on activation; `skipWaiting`
  and `clients.claim`.
- **Startup loader**: `startApplication` registers `sw.js` beside the page
  once the application has started, when the page links a manifest
  (`serviceWorker: 'auto'`, or `true` or `false`), and records the outcome in
  `__STOBIE_STARTUP.serviceWorker`. `fitViewport: true` keeps CSS variables at
  the visual viewport's size, so the shell stays above the iOS keyboard, as
  the spike's own script did.
- **Template**: `stobie new` writes the page with every tag, including
  `viewport-fit=cover`, page zoom off, safe-area insets on a fixed shell, and
  placeholder icons (192, 512 and the 180 apple-touch-icon) generated as PNGs
  in code, because the repository allows binary files only in reviewed
  locations.
- `tools/browser-serve.mjs` serves the PWA files as mutable (`no-cache`), and
  both servers give `.webmanifest` the `application/manifest+json` type.

## Acceptance

- Unit tests (`stobie-build`): head parsing (comments, scripts, quoting,
  entities, body links ignored, the colour without a media query), each
  missing requirement named, icons checked for size, format and staying inside
  the root, manifest fields and icon prefixes, service worker substitution;
  packaging of an installable page (root files, link rewrite, assets without
  development files, precache of every asset, the package manifest's `pwa`
  field) and of a page without one (unchanged, no `pwa` field).
- Unit tests (CLI): the template page is installable and the generated icons
  pass the checks.
- Loader tests: registration only for a page that links a manifest, the
  scope and URL beside the page, unsupported and failed outcomes; the visual
  viewport sizing and keyboard inset.
- Production server test: the files are served at the web root as mutable,
  with the manifest's media type, and not from the hashed directory.
- `tools/pwa-smoke.mjs` in the outside-app stage, on a `stobie new`
  application's development and production output in Chromium:
  - the files are at the web root;
  - the head carries `viewport-fit=cover` and the Apple tags;
  - CDP `Page.getAppManifest` and `Page.getInstallabilityErrors` report no
    errors;
  - icons are served as PNGs from the asset directory;
  - the service worker registers and controls the page;
  - a controlled reload fetches only the page and the manifest from the
    server;
  - an offline reload starts the application.
- `cargo xtask verify` passes.

## Delivery increments

| Increment | Status | Evidence |
| --- | --- | --- |
| Declaration, manifest, service worker and icons in `stobie-build` | Complete | Unit tests |
| Development and production output, servers | Complete | Packaging and server tests |
| Loader registration and viewport fitting; template page and icons | Complete | Loader and CLI tests |
| Browser verification | Complete for Chromium | PWA smoke in the outside-app stage; no iPhone run (see [Device check](#device-check)) |

## Device check

Not run. Chromium covers installability, caching and offline start, but not
the iPhone's home-screen installation, its icon and splash, safe areas, the
keyboard shell or launching offline from the home screen. The spike proved
each of those on the owner's iPhone with hand-written files that these
generated ones reproduce: the same tags, layout, manifest fields and caching
policy, with the manifest and worker at the web root. Differences from the
spike: icons are served from the hashed asset directory, and the manifest
`id`, `start_url` and `scope` are relative. A device check therefore needs the
owner. Bokkie's first iPhone slice is the natural place for it: install from
Safari, check the icon and name, launch standalone, rotate, raise the
keyboard, and relaunch offline in Aeroplane Mode.

## Observations

- Chromium reports `in-incognito` as an installability error for Playwright's
  default context, so the smoke uses a persistent profile.
- `stobie new ios-spike` still titles the app "Ios Spike", which now also
  becomes its home-screen name; edit the page's tags.

## Later work

- An iPhone check of the generated output (owner).
- Maskable icons, and screenshots for richer install prompts, if wanted.
- An update notice: the new service worker takes over at once, so an open
  page keeps running its package until reloaded.
