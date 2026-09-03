import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, extname, join, normalize } from 'node:path';
import { tmpdir } from 'node:os';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';

const root = normalize(join(process.cwd(), 'apps/polyorama-gallery/web'));
const evidenceRoot = normalize(process.env.POLYORAMA_EVIDENCE_DIR
  ?? join(process.cwd(), 'docs/design-agent-loop-evidence'));
await mkdir(evidenceRoot, { recursive: true });
const mime = new Map([['.html', 'text/html'], ['.js', 'text/javascript'], ['.css', 'text/css'], ['.wasm', 'application/wasm']]);
const server = createServer(async (request, response) => {
  try {
    const relative = request.url === '/' ? 'index.html' : request.url.slice(1).split('?')[0];
    const path = normalize(join(root, relative));
    if (!path.startsWith(root) || !(await stat(path)).isFile()) throw new Error('not found');
    response.writeHead(200, { 'Content-Type': mime.get(extname(path)) ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(await readFile(path));
  } catch {
    response.writeHead(404); response.end('not found');
  }
});
await new Promise((resolve) => server.listen(4174, '127.0.0.1', resolve));

const errors = [];
const installedChromium = chromium.executablePath();
const revisionDirectory = basename(dirname(dirname(installedChromium)));
const revision = revisionDirectory.slice(revisionDirectory.lastIndexOf('-') + 1);
const headlessShell = process.env.POLYORAMA_CHROMIUM ?? join(
  dirname(dirname(dirname(installedChromium))),
  `chromium_headless_shell-${revision}`,
  'chrome-headless-shell-linux64',
  'chrome-headless-shell',
);
const browserFlags = [
  '--headless', '--no-sandbox',
  '--enable-unsafe-webgpu', '--enable-features=Vulkan,CDPScreenshotNewSurface', '--use-angle=vulkan',
  '--disable-vulkan-surface',
];
let browser;
let browserProcess;
let browserProfile;
if (process.env.POLYORAMA_USE_SYSTEM_UI_LIBS === '1') {
  browser = await chromium.launch(hostedLinuxWebGpuLaunchOptions());
} else {
  browserProfile = await mkdtemp(join(tmpdir(), 'polyorama-gallery-browser-'));
  browserProcess = spawn(headlessShell, [
    ...browserFlags, '--remote-debugging-port=0',
    `--user-data-dir=${browserProfile}`, 'about:blank',
  ], {
    env: { ...process.env, LD_LIBRARY_PATH: `${join(process.cwd(), '.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const cdpEndpoint = await new Promise((resolve, reject) => {
    let diagnostics = '';
    const timeout = setTimeout(() => reject(new Error(`Chromium CDP endpoint timed out: ${diagnostics}`)), 10_000);
    browserProcess.stderr.setEncoding('utf8');
    browserProcess.stderr.on('data', (chunk) => {
      diagnostics += chunk;
      const endpoint = diagnostics.match(/DevTools listening on (ws:\/\/\S+)/)?.[1];
      if (endpoint) { clearTimeout(timeout); resolve(endpoint); }
    });
    browserProcess.once('error', (error) => { clearTimeout(timeout); reject(error); });
    browserProcess.once('exit', (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`Chromium exited before CDP attachment: code=${code} signal=${signal}\n${diagnostics}`));
    });
  });
  browser = await chromium.connectOverCDP(cdpEndpoint);
}
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
await page.context().grantPermissions(['clipboard-read', 'clipboard-write'], {
  origin: 'http://127.0.0.1:4174',
});
page.on('pageerror', (error) => errors.push(`pageerror: ${error.stack ?? error}`));
page.on('console', (message) => { if (message.type() === 'error') errors.push(`console: ${message.text()}`); });
const launchStartedMs = Date.now();
const releaseObservations = { story_transitions: [] };

const assertRenderedScreenshot = async (screenshot, label) => {
  const statistics = await page.evaluate(async (base64) => {
    const bytes = Uint8Array.from(atob(base64), (character) => character.charCodeAt(0));
    const bitmap = await createImageBitmap(new Blob([bytes], { type: 'image/png' }));
    const canvas = document.createElement('canvas');
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const context = canvas.getContext('2d');
    context.drawImage(bitmap, 0, 0);
    const pixels = context.getImageData(0, 0, bitmap.width, bitmap.height).data;
    let minimum = 255;
    let maximum = 0;
    for (let index = 0; index < pixels.length; index += 4) {
      minimum = Math.min(minimum, pixels[index], pixels[index + 1], pixels[index + 2]);
      maximum = Math.max(maximum, pixels[index], pixels[index + 1], pixels[index + 2]);
    }
    return { minimum, maximum };
  }, screenshot.toString('base64'));
  if (statistics.maximum === 0 || statistics.minimum === statistics.maximum) {
    throw new Error(`${label} has no visible rendered output: ${JSON.stringify(statistics)}`);
  }
};

const snapshot = () => page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot());
const selectStory = async (story) => {
  const before = await snapshot();
  const startedMs = Date.now();
  await page.evaluate((value) => window.__POLYORAMA_GALLERY_HANDLE.select_story(value), story);
  await page.waitForFunction(
    ({ frame, storyId }) => {
      const current = window.__POLYORAMA_GALLERY_HANDLE.snapshot();
      return current.frame > frame && current.story === storyId;
    },
    { frame: before.frame, storyId: story },
    { timeout: 10_000 },
  );
  const current = await snapshot();
  releaseObservations.story_transitions.push({
    story,
    wall_ms: Date.now() - startedMs,
    frame_delta: current.frame - before.frame,
  });
  return current;
};

try {
  await page.goto('http://127.0.0.1:4174', { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => document.body.classList.contains('ready')
    && window.__POLYORAMA_GALLERY_HANDLE?.snapshot().frame > 0, null, { timeout: 30_000 });
  releaseObservations.initial_ready_wall_ms = Date.now() - launchStartedMs;
  const manifest = await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.manifest());
  if (manifest.length !== 18 || new Set(manifest.map((entry) => entry.id)).size !== 18) {
    throw new Error(`invalid gallery manifest: ${JSON.stringify(manifest)}`);
  }
  await writeFile(join(evidenceRoot, 'gallery-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`);

  let current = await snapshot();
  if (current.story !== 'reference/application-shell' || current.story_count !== 18
      || current.text.length === 0 || current.text_audit.length !== 0
      || current.text_audit_coverage?.native_text_controls !== manifest.length + 5
      || current.text_audit_coverage.observed_native_controls !== 0
      || JSON.stringify(current.text_audit_coverage) !== JSON.stringify(current.ui_snapshot.text_audit_coverage)
      || current.ui_snapshot.nodes.length === 0 || current.ui_snapshot.nodes.length >= 1_000
      || current.ui_snapshot.semantic_audit.length !== 0
      || !current.ui_snapshot.nodes.some((node) => node.role === 'tab')
      || !current.ui_snapshot.nodes.some((node) => node.role === 'splitter')) {
    throw new Error(`invalid initial gallery snapshot: ${JSON.stringify(current)}`);
  }
  const overview = await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-overview.png') });
  await assertRenderedScreenshot(overview, 'gallery overview screenshot');

  await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.set_configuration({
    appearance: 'light', contrast: 'high', density: 'compact', font_scale: 1.5, width: 'narrow',
  }));
  current = await selectStory('reference/diagnostics');
  if (current.configuration.appearance !== 'light' || current.configuration.contrast !== 'high'
      || current.configuration.density !== 'compact' || current.configuration.font_scale !== 1.5
      || current.configuration.width !== 'narrow' || current.text_audit.length !== 0) {
    throw new Error(`gallery configuration did not apply coherently: ${JSON.stringify(current)}`);
  }
  await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-high-contrast-narrow.png') });

  current = await selectStory('property-row/long-value');
  if (current.text.length < 2 || current.text_audit.length !== 0
      || current.ui_snapshot.semantic_audit.length !== 0
      || current.text.filter((entry) => entry.interaction === 'selectable').length !== 1
      || !current.ui_snapshot.nodes.some((node) => node.id === 'gallery.story'
        && node.text_selectable === true)) {
    throw new Error(`long-text property story failed: ${JSON.stringify(current)}`);
  }
  await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-long-text.png') });

  current = await selectStory('status/error-long-message');
  if (current.text.length !== 1 || current.text_audit.length !== 0
      || current.ui_snapshot.semantic_audit.length !== 0
      || current.text[0].interaction !== 'selectable'
      || !current.ui_snapshot.nodes.some((node) => node.id === 'gallery.story'
        && node.text_selectable === true)) {
    throw new Error(`error-status story failed: ${JSON.stringify(current)}`);
  }
  await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-error.png') });

  current = await selectStory('virtual-grid/loading');
  if (current.text.length === 0 || current.text_audit.length !== 0
      || current.ui_snapshot.semantic_audit.length !== 0) {
    throw new Error(`loading-grid story failed: ${JSON.stringify(current)}`);
  }
  await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-loading.png') });

  await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.set_configuration({
    appearance: 'dark', contrast: 'standard', density: 'comfortable', font_scale: 1.0, width: 'wide',
  }));
  current = await selectStory('reference/inspector');
  const confidenceValue = current.text.find((entry) => entry.component_id.kind === 'property_row'
    && entry.component_id.instance === 105);
  if (!confidenceValue || confidenceValue.interaction !== 'selectable'
      || !current.ui_snapshot.nodes.some((node) => node.id === 'gallery.story'
        && node.text_selectable === true)) {
    throw new Error(`inspector selection semantics failed: ${JSON.stringify(current)}`);
  }
  const selectionY = (confidenceValue.allocated_rect.min_y + confidenceValue.allocated_rect.max_y) / 2;
  await page.mouse.move(confidenceValue.allocated_rect.min_x + 1, selectionY);
  await page.mouse.down();
  await page.mouse.move(confidenceValue.allocated_rect.max_x - 1, selectionY, { steps: 12 });
  await page.mouse.up();
  await page.keyboard.press('Control+C');
  const copiedInspectorValue = await page.evaluate(() => navigator.clipboard.readText());
  if (copiedInspectorValue !== '99.875 %') {
    throw new Error(`inspector drag-copy mismatch: ${JSON.stringify(copiedInspectorValue)}`);
  }
  releaseObservations.inspector_drag_copy = copiedInspectorValue;

  await page.keyboard.press('Escape');
  current = await selectStory('reference/results');
  if (current.text.some((entry) => entry.interaction !== 'inert')
      || current.ui_snapshot.nodes.some((node) => node.id === 'gallery.story'
        && node.text_selectable === true)) {
    throw new Error(`result rows unexpectedly exposed text selection: ${JSON.stringify(current)}`);
  }
  const resultText = current.text.find((entry) => entry.component_id.kind === 'result_row');
  if (!resultText) throw new Error(`result row text observation missing: ${JSON.stringify(current)}`);
  const clipboardSentinel = 'polyorama-result-row-selection-remains-inert';
  await page.evaluate((value) => navigator.clipboard.writeText(value), clipboardSentinel);
  const resultSelectionY = (resultText.allocated_rect.min_y + resultText.allocated_rect.max_y) / 2;
  await page.mouse.move(resultText.allocated_rect.min_x + 1, resultSelectionY);
  await page.mouse.down();
  await page.mouse.move(resultText.allocated_rect.max_x - 1, resultSelectionY, { steps: 12 });
  await page.mouse.up();
  await page.keyboard.press('Control+C');
  const copiedResultValue = await page.evaluate(() => navigator.clipboard.readText());
  if (copiedResultValue !== clipboardSentinel) {
    throw new Error(`result row drag unexpectedly copied text: ${JSON.stringify(copiedResultValue)}`);
  }
  releaseObservations.result_row_drag_copy = copiedResultValue;

  current = await selectStory('tabs/narrow');
  const narrowTextMin = Math.min(...current.text.map((entry) => entry.allocated_rect.min_x));
  const narrowTextMax = Math.max(...current.text.map((entry) => entry.allocated_rect.max_x));
  if (current.configuration.width !== 'wide' || narrowTextMax - narrowTextMin > 296
      || current.text_audit.length !== 0) {
    throw new Error(`narrow tab story did not retain its own bounded geometry: ${JSON.stringify(current)}`);
  }

  current = await selectStory('splitter/hover-active');
  if (current.text.length === 0 || current.text_audit.length !== 0) {
    throw new Error(`deterministic splitter-state story failed: ${JSON.stringify(current)}`);
  }
  await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-splitter-states.png') });

  current = await selectStory('button/keyboard-focus');
  await page.waitForTimeout(100);
  current = await snapshot();
  if (current.text.length !== 1 || current.text_audit.length !== 0
      || current.ui_snapshot.semantic_audit.length !== 0
      || !current.ui_snapshot.nodes.some((node) => node.actions.includes('fit_view')
        && node.pane === 1 && node.focused)) {
    throw new Error(`keyboard-focus story failed: ${JSON.stringify(current)}`);
  }

  await page.waitForTimeout(600);
  const idleBefore = (await snapshot()).frame;
  await page.waitForTimeout(400);
  const idleAfter = (await snapshot()).frame;
  if (idleAfter !== idleBefore) throw new Error(`gallery repainted while idle: ${idleBefore} -> ${idleAfter}`);
  if (errors.length) throw new Error(errors.join('\n'));
  await writeFile(join(evidenceRoot, 'gallery-browser-snapshot.json'), `${JSON.stringify(current, null, 2)}\n`);
  await writeFile(join(evidenceRoot, 'gallery-browser-evidence.json'), `${JSON.stringify({
    build: 'release',
    browser: browser.version(),
    automation: 'Playwright 1.62.1',
    host: `${process.platform}/${process.arch}`,
    backend: 'browser WebGPU via eframe/wgpu',
    viewport: '1440x900 CSS pixels',
    stories: manifest.length,
    release_observations: releaseObservations,
    idle_frame_before: idleBefore,
    idle_frame_after: idleAfter,
    text_audit_findings: current.text_audit.length,
    text_audit_coverage: current.text_audit_coverage,
    semantic_node_count: current.ui_snapshot.nodes.length,
    semantic_audit_findings: current.ui_snapshot.semantic_audit.length,
  }, null, 2)}\n`);
  console.log(JSON.stringify({ status: 'passed', stories: manifest.length, idleFrame: idleAfter, textAudit: current.text_audit }, null, 2));
} catch (error) {
  await page.screenshot({ path: join(evidenceRoot, 'gallery-browser-failure.png') }).catch(() => {});
  const loading = await page.locator('#loading').textContent().catch(() => '<missing>');
  throw new Error(`${error.stack ?? error}\nloading: ${loading}\n${errors.join('\n')}`);
} finally {
  await browser.close().catch(() => {});
  if (browserProcess?.exitCode === null) {
    browserProcess.kill('SIGTERM');
    await Promise.race([once(browserProcess, 'exit'), new Promise((resolve) => setTimeout(resolve, 2_000))]);
  }
  if (browserProfile) await rm(browserProfile, { recursive: true, force: true });
  await new Promise((resolve) => server.close(resolve));
}
