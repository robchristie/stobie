// Check that `stobie build-web` and `stobie build-web --production` wrote an
// installable web application for a page that asks for one: the manifest and
// service worker at the web root, Chromium's installability checks passing,
// the service worker controlling the page, hashed assets served from its
// cache, the page network-first, and an offline reload starting the
// application. Run inside the smoke environment (tools/outside-app-smoke.sh).
//
// node tools/pwa-smoke.mjs APP_NAME WEB_DIR PRODUCTION_DIR EVIDENCE_DIR
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { createProductionServer } from './browser-serve.mjs';

const [name, webArgument, productionArgument, evidenceArgument] = process.argv.slice(2);
assert(name && webArgument && productionArgument && evidenceArgument, 'usage: APP_NAME WEB_DIR PRODUCTION_DIR EVIDENCE_DIR');
const [web, production, evidence] = [webArgument, productionArgument, evidenceArgument].map(path => resolve(path));
await mkdir(evidence, { recursive: true });
const report = { app: name, development: null, production: null };

// The files sit at the web root, outside the hashed asset directory.
const packageManifest = JSON.parse(readFileSync(join(production, 'manifest.json'), 'utf8'));
const packaged = packageManifest.apps.find(app => app.name === name);
assert(packaged?.pwa, 'the package manifest records the PWA files');
assert.equal(packaged.pwa.manifest.path, `${name}/manifest.webmanifest`);
assert.equal(packaged.pwa.serviceWorker.path, `${name}/sw.js`);
for (const file of ['manifest.webmanifest', 'sw.js']) {
  assert(existsSync(join(web, file)), `build-web wrote ${file} beside index.html`);
  assert(existsSync(join(production, name, file)), `the package has ${file} at its web root`);
  assert(!existsSync(join(production, packaged.assetDirectory, file)), `${file} is not in the hashed directory`);
}

const { chromium } = await import('playwright');
const launch = await import('./browser-launch.mjs');

async function check(label, url, { assetPrefix, server }) {
  // A persistent profile: Chromium never installs from an incognito one.
  const profile = await mkdtemp(join(evidence, `profile-${label}-`));
  const context = await chromium.launchPersistentContext(profile, {
    ...launch.hostedLinuxWebGpuLaunchOptions(), env: launch.uiLibraryEnvironment(), viewport: { width: 1024, height: 768 },
  });
  const result = { url };
  try {
    const page = context.pages()[0] ?? await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    const ready = () => page.waitForFunction(() => window.__STOBIE_STARTUP?.status === 'ready', null, { timeout: 60000 });
    await page.goto(url);
    await ready();
    // The page's head carries what iOS reads.
    const head = await page.evaluate(() => ({
      viewport: document.querySelector('meta[name=viewport]')?.content,
      title: document.querySelector('meta[name=apple-mobile-web-app-title]')?.content,
      capable: document.querySelector('meta[name=apple-mobile-web-app-capable]')?.content,
      appleTouchIcon: document.querySelector('link[rel=apple-touch-icon]')?.href,
      manifest: document.querySelector('link[rel=manifest]')?.href,
    }));
    assert.match(head.viewport, /viewport-fit=cover/);
    assert.equal(head.capable, 'yes');
    assert.equal(head.manifest, new URL('manifest.webmanifest', url).href, 'the manifest link names the web root');
    const cdp = await context.newCDPSession(page);
    const manifest = await cdp.send('Page.getAppManifest');
    assert.deepEqual(manifest.errors, [], `${label}: manifest errors`);
    const parsed = JSON.parse(manifest.data);
    assert.equal(parsed.short_name, head.title);
    assert.equal(parsed.display, 'standalone');
    for (const icon of parsed.icons) {
      assert(icon.src.startsWith(assetPrefix), `${label}: icon ${icon.src} under ${assetPrefix}`);
      const response = await page.request.get(new URL(icon.src, head.manifest).href);
      assert.equal(response.headers()['content-type'], 'image/png', `${label}: ${icon.src}`);
    }
    // The service worker registers from the loader and takes control.
    await page.waitForFunction(() => navigator.serviceWorker.controller !== null, null, { timeout: 30000 });
    result.serviceWorker = await page.evaluate(() => window.__STOBIE_STARTUP.serviceWorker);
    assert.equal(result.serviceWorker.state, 'registered');
    assert.equal(result.serviceWorker.scope, new URL('./', url).href);
    const installability = await cdp.send('Page.getInstallabilityErrors');
    assert.deepEqual(installability.installabilityErrors, [], `${label}: installability`);
    result.manifest = parsed;
    // A reload while controlled: the page comes from the network, the
    // hashed assets from the cache.
    server?.requests.splice(0);
    await page.reload();
    await ready();
    if (server) {
      const paths = server.requests.map(request => new URL(request.url, 'http://local').pathname);
      result.onlineReloadRequests = paths;
      assert(paths.includes(`/${name}/`), `${label}: the page is network-first`);
      assert(!paths.some(path => path.includes('/assets/')), `${label}: hashed assets are cache-first (${paths.join(', ')})`);
    }
    // Offline, the cached page and assets start the application.
    await context.setOffline(true);
    await page.reload();
    await ready();
    await launch.assertBrowserRenderer(page, '#app-canvas');
    await context.setOffline(false);
    result.offline = 'ready';
    assert.deepEqual(errors, [], `${label}: no page errors`);
  } finally {
    await context.close();
    await rm(profile, { recursive: true, force: true });
  }
  return result;
}

// Development: `stobie serve`-style static files from the web directory.
const { createServer } = await import('node:http');
const { stat, readFile } = await import('node:fs/promises');
const { extname, normalize } = await import('node:path');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.wasm': 'application/wasm', '.png': 'image/png', '.webmanifest': 'application/manifest+json' };
const development = createServer(async (request, response) => {
  try {
    const path = normalize(join(web, new URL(request.url, 'http://local').pathname.slice(1) || 'index.html'));
    if (!path.startsWith(web) || !(await stat(path)).isFile()) throw new Error('not found');
    response.writeHead(200, { 'Content-Type': mime[extname(path)] ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(await readFile(path));
  } catch { response.writeHead(404); response.end(); }
});
await new Promise(fulfil => development.listen(0, '127.0.0.1', fulfil));
const packagedServer = createProductionServer({ directory: production, recordRequests: true });
await new Promise(fulfil => packagedServer.listen(0, '127.0.0.1', fulfil));
try {
  report.development = await check('development', `http://127.0.0.1:${development.address().port}/`, { assetPrefix: 'icons/' });
  report.production = await check('production', `http://127.0.0.1:${packagedServer.address().port}/${name}/`, {
    assetPrefix: `assets/${packaged.contentId}/icons/`, server: packagedServer,
  });
} finally {
  development.close();
  packagedServer.close();
  await writeFile(join(evidence, 'pwa.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`PWA smoke passed: development and production output installable and offline; evidence ${evidence}/pwa.json`);
