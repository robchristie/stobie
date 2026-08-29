import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { chmod, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { tmpdir } from 'node:os';
import { chromium } from 'playwright';
import { swiftShaderWebGpuFlags } from './browser-launch.mjs';

const requestFlag = process.argv.indexOf('--request');
if (requestFlag < 0 || !process.argv[requestFlag + 1]) {
  throw new Error('usage: node tools/ui-capture.mjs --request <request.json>');
}

const requestPath = normalize(process.argv[requestFlag + 1]);
const request = JSON.parse(await readFile(requestPath, 'utf8'));
if (request.schema_version !== 1) throw new Error('unsupported UI capture request schema');
const fixture = request.fixture;
const output = normalize(request.output_directory);
const logs = join(output, 'logs');
await mkdir(logs, { recursive: true });

const repositoryRoot = normalize(process.cwd());
const webRoot = normalize(join(repositoryRoot, 'apps/polyorama-gallery/web'));
const mime = new Map([
  ['.html', 'text/html'], ['.js', 'text/javascript'], ['.css', 'text/css'],
  ['.wasm', 'application/wasm'], ['.png', 'image/png'],
]);
const server = createServer(async (incoming, response) => {
  try {
    const relative = incoming.url === '/' ? 'index.html' : incoming.url.slice(1).split('?')[0];
    const path = normalize(join(webRoot, relative));
    if (!path.startsWith(webRoot) || !(await stat(path)).isFile()) throw new Error('not found');
    response.writeHead(200, {
      'Content-Type': mime.get(extname(path)) ?? 'application/octet-stream',
      'Cache-Control': 'no-store',
    });
    response.end(await readFile(path));
  } catch {
    response.writeHead(404);
    response.end('not found');
  }
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const address = server.address();
if (!address || typeof address === 'string') throw new Error('UI capture server has no TCP address');

const consoleMessages = [];
const pageErrors = [];
const runtimeErrors = [];
let browser;
let browserProcess;
let browserProfile;
let browserDiagnostics = '';
let xvfbProcess;
let xvfbDiagnostics = '';
let x11TemporaryDirectory;

async function launchBrowser() {
  if (process.platform !== 'linux') {
    const headless = process.env.POLYORAMA_BROWSER_HEADFUL !== '1';
    browser = await chromium.launch({ headless, args: ['--enable-unsafe-webgpu'] });
    return;
  }
  const useSystemLibraries = process.env.POLYORAMA_USE_SYSTEM_UI_LIBS === '1';
  const chromiumExecutable = chromium.executablePath();
  let executable = chromiumExecutable;
  let prefix = [];
  let display = process.env.DISPLAY;
  const libraryEnvironment = useSystemLibraries ? process.env : {
    ...process.env,
    LD_LIBRARY_PATH: `${join(repositoryRoot, '.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}`,
  };
  let browserEnvironment = libraryEnvironment;

  if (useSystemLibraries) {
    browserProfile = await mkdtemp(join(tmpdir(), 'polyorama-ui-chromium-'));
  } else {
    x11TemporaryDirectory = await mkdtemp(join(repositoryRoot, '.tools/runtime/ui-x11-'));
    const socketDirectory = join(x11TemporaryDirectory, '.X11-unix');
    await mkdir(socketDirectory);
    await chmod(x11TemporaryDirectory, 0o1777);
    await chmod(socketDirectory, 0o1777);
    const sysroot = join(repositoryRoot, '.tools/sysroot');
    const sandbox = [
      '--unshare-pid', '--die-with-parent',
      '--ro-bind', '/', '/',
      '--bind', x11TemporaryDirectory, '/tmp',
      '--ro-bind', '/usr/bin', '/opt',
      '--ro-bind', join(sysroot, 'usr/bin'), '/usr/bin',
      '--dev-bind', '/dev', '/dev',
      '--proc', '/proc',
    ];
    const privateDisplay = `:${100 + (process.pid % 10_000)}`;
    xvfbProcess = spawn('bwrap', [
      ...sandbox,
      join(sysroot, 'usr/bin/Xvfb'), privateDisplay,
      '-screen', '0', '1440x900x24', '-nolisten', 'tcp', '+extension', 'GLX',
    ], { env: libraryEnvironment, stdio: ['ignore', 'ignore', 'pipe'] });
    xvfbProcess.stderr.setEncoding('utf8');
    xvfbProcess.stderr.on('data', (chunk) => { xvfbDiagnostics += chunk; });
    await new Promise((resolve) => setTimeout(resolve, 500));
    if (xvfbProcess.exitCode !== null) {
      throw new Error(`private Xvfb failed to start: code=${xvfbProcess.exitCode}\n${xvfbDiagnostics}`);
    }
    executable = 'bwrap';
    prefix = [...sandbox, chromiumExecutable];
    display = privateDisplay;
    browserProfile = '/tmp/chromium-profile';
    for (const directory of ['chromium-profile', 'config', 'cache', 'runtime']) {
      await mkdir(join(x11TemporaryDirectory, directory), { mode: 0o700 });
    }
    browserEnvironment = {
      ...libraryEnvironment,
      XDG_CONFIG_HOME: '/tmp/config',
      XDG_CACHE_HOME: '/tmp/cache',
      XDG_RUNTIME_DIR: '/tmp/runtime',
      TMPDIR: '/tmp',
    };
  }

  browserProcess = spawn(executable, [
    ...prefix,
    '--no-sandbox', ...swiftShaderWebGpuFlags(), '--ozone-platform=x11',
    '--remote-debugging-port=0', `--user-data-dir=${browserProfile}`, 'about:blank',
  ], {
    env: { ...browserEnvironment, DISPLAY: display },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  const cdpEndpoint = await new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`Chromium CDP endpoint timed out: ${browserDiagnostics}`)),
      10_000,
    );
    browserProcess.stderr.setEncoding('utf8');
    browserProcess.stderr.on('data', (chunk) => {
      browserDiagnostics += chunk;
      const endpoint = browserDiagnostics.match(/DevTools listening on (ws:\/\/\S+)/)?.[1];
      if (endpoint) {
        clearTimeout(timeout);
        resolve(endpoint);
      }
    });
    browserProcess.once('error', (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    browserProcess.once('exit', (code, signal) => {
      clearTimeout(timeout);
      reject(new Error(`Chromium exited before CDP attachment: code=${code} signal=${signal}\n${browserDiagnostics}`));
    });
  });
  browser = await chromium.connectOverCDP(cdpEndpoint);
}

function stableSemantic(snapshot) {
  const source = snapshot.ui_snapshot;
  const nodes = source.nodes.map((node) => ({
    ...node,
    actions: [...node.actions].sort(),
  })).sort((left, right) => JSON.stringify(left.id).localeCompare(JSON.stringify(right.id)));
  return {
    schema_version: 1,
    pixels_per_point: source.pixels_per_point,
    root: source.root,
    nodes,
    semantic_audit: source.semantic_audit,
  };
}

function stableText(snapshot) {
  const observations = [...snapshot.text].sort((left, right) => {
    const leftKey = `${left.component_id.kind}:${left.component_id.instance}`;
    const rightKey = `${right.component_id.kind}:${right.component_id.instance}`;
    return leftKey.localeCompare(rightKey);
  });
  return { schema_version: 1, observations, audit: snapshot.text_audit };
}

async function pixelStatistics(page, screenshot) {
  return page.evaluate(async (base64) => {
    const response = await fetch(`data:image/png;base64,${base64}`);
    const image = await createImageBitmap(await response.blob());
    const canvas = document.createElement('canvas');
    canvas.width = image.width;
    canvas.height = image.height;
    const context = canvas.getContext('2d', { willReadFrequently: true });
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, image.width, image.height).data;
    const minimum = [255, 255, 255];
    const maximum = [0, 0, 0];
    for (let index = 0; index < pixels.length; index += 4) {
      for (let channel = 0; channel < 3; channel += 1) {
        minimum[channel] = Math.min(minimum[channel], pixels[index + channel]);
        maximum[channel] = Math.max(maximum[channel], pixels[index + channel]);
      }
    }
    return { minimum, maximum };
  }, screenshot.toString('base64'));
}

async function comparePixels(page, expectedPath, actualPath) {
  const [expected, actual] = await Promise.all([readFile(expectedPath), readFile(actualPath)]);
  return page.evaluate(async ({ expectedBase64, actualBase64 }) => {
    const bitmap = async (base64) => {
      const response = await fetch(`data:image/png;base64,${base64}`);
      return createImageBitmap(await response.blob());
    };
    const [expectedImage, actualImage] = await Promise.all([
      bitmap(expectedBase64), bitmap(actualBase64),
    ]);
    const width = Math.max(expectedImage.width, actualImage.width);
    const height = Math.max(expectedImage.height, actualImage.height);
    const pixels = (image) => {
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      context.clearRect(0, 0, width, height);
      context.drawImage(image, 0, 0);
      return context.getImageData(0, 0, width, height).data;
    };
    const expectedPixels = pixels(expectedImage);
    const actualPixels = pixels(actualImage);
    const diffCanvas = document.createElement('canvas');
    diffCanvas.width = width;
    diffCanvas.height = height;
    const diffContext = diffCanvas.getContext('2d');
    const diff = diffContext.createImageData(width, height);
    let differingPixels = 0;
    for (let index = 0; index < diff.data.length; index += 4) {
      const differs = expectedPixels[index] !== actualPixels[index]
        || expectedPixels[index + 1] !== actualPixels[index + 1]
        || expectedPixels[index + 2] !== actualPixels[index + 2]
        || expectedPixels[index + 3] !== actualPixels[index + 3];
      if (differs) {
        differingPixels += 1;
        diff.data.set([255, 0, 180, 255], index);
      } else {
        const grey = Math.round((expectedPixels[index] + expectedPixels[index + 1]
          + expectedPixels[index + 2]) / 9);
        diff.data.set([grey, grey, grey, 255], index);
      }
    }
    diffContext.putImageData(diff, 0, 0);
    return {
      dimensions_equal: expectedImage.width === actualImage.width
        && expectedImage.height === actualImage.height,
      differing_pixels: differingPixels,
      total_pixels: width * height,
      diff_base64: diffCanvas.toDataURL('image/png').split(',')[1],
    };
  }, { expectedBase64: expected.toString('base64'), actualBase64: actual.toString('base64') });
}

let exitError;
try {
  await launchBrowser();
  const page = await browser.newPage({
    viewport: { width: fixture.viewport.width, height: fixture.viewport.height },
    deviceScaleFactor: 1,
    colorScheme: fixture.configuration.appearance === 'light' ? 'light' : 'dark',
    reducedMotion: 'reduce',
  });
  page.on('console', (message) => {
    consoleMessages.push(`${message.type()}: ${message.text()}`);
    if (message.type() === 'error') runtimeErrors.push(`console: ${message.text()}`);
  });
  page.on('pageerror', (error) => {
    pageErrors.push(error.stack ?? String(error));
    runtimeErrors.push(`pageerror: ${error.stack ?? error}`);
  });
  await page.goto(`http://127.0.0.1:${address.port}`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => document.body.classList.contains('ready')
    && window.__POLYORAMA_GALLERY_HANDLE?.snapshot().frame > 0, null, { timeout: 30_000 });
  const before = await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot());
  await page.evaluate(({ configuration, story }) => {
    window.__POLYORAMA_GALLERY_HANDLE.set_configuration(configuration);
    window.__POLYORAMA_GALLERY_HANDLE.select_story(story);
  }, { configuration: fixture.configuration, story: fixture.story });
  await page.waitForFunction(({ story, frame }) => {
    const snapshot = window.__POLYORAMA_GALLERY_HANDLE.snapshot();
    return snapshot.story === story && snapshot.frame >= frame;
  }, { story: fixture.story, frame: before.frame }, { timeout: 10_000 });
  await page.waitForTimeout(200);
  const snapshot = await page.evaluate(() => window.__POLYORAMA_GALLERY_HANDLE.snapshot());
  if (snapshot.story !== fixture.story) throw new Error(`story did not settle to ${fixture.story}`);
  if (runtimeErrors.length) throw new Error(runtimeErrors.join('\n'));

  const metadata = {
    schema_version: 1,
    fixture_id: fixture.id,
    story: fixture.story,
    viewport: fixture.viewport,
    configuration: fixture.configuration,
    data_fixture: fixture.data_fixture,
    fonts: fixture.fonts,
    renderer: fixture.renderer,
    pixel_comparison: { colour_space: 'srgb8', tolerance: 0 },
  };
  await writeFile(join(output, 'metadata.json'), `${JSON.stringify(metadata, null, 2)}\n`);
  await writeFile(join(output, 'semantic.json'), `${JSON.stringify(stableSemantic(snapshot), null, 2)}\n`);
  await writeFile(join(output, 'text.json'), `${JSON.stringify(stableText(snapshot), null, 2)}\n`);
  const visualPath = join(output, 'visual.png');
  const visual = await page.screenshot({ path: visualPath, animations: 'disabled' });
  const visualStatistics = await pixelStatistics(page, visual);
  if (!visualStatistics.maximum.some(
    (maximum, channel) => maximum > visualStatistics.minimum[channel],
  )) {
    throw new Error(`UI capture has no spatial pixel variation: ${JSON.stringify(visualStatistics)}`);
  }

  if (request.expected_visual) {
    const comparison = await comparePixels(page, request.expected_visual, visualPath);
    await writeFile(join(output, 'visual-diff.png'), Buffer.from(comparison.diff_base64, 'base64'));
    delete comparison.diff_base64;
    await writeFile(join(output, 'visual-diff.json'), `${JSON.stringify(comparison, null, 2)}\n`);
  }
  await writeFile(join(logs, 'runtime.json'), `${JSON.stringify({
    schema_version: 1,
    browser: browser.version(),
    automation: 'Playwright 1.62.1',
    host: `${process.platform}/${process.arch}`,
    backend: fixture.renderer,
    frame_observed: snapshot.frame,
    pixel_statistics: visualStatistics,
  }, null, 2)}\n`);
  await page.close();
} catch (error) {
  exitError = error;
  await writeFile(join(logs, 'error.log'), `${error.stack ?? error}\n`).catch(() => {});
} finally {
  await writeFile(join(logs, 'browser-console.log'), `${consoleMessages.join('\n')}\n`).catch(() => {});
  await writeFile(join(logs, 'page-errors.log'), `${pageErrors.join('\n')}\n`).catch(() => {});
  await writeFile(join(logs, 'chromium.log'), browserDiagnostics).catch(() => {});
  await writeFile(join(logs, 'xvfb.log'), xvfbDiagnostics).catch(() => {});
  if (browser) {
    await Promise.race([
      browser.close().catch(() => {}),
      new Promise((resolve) => setTimeout(resolve, 2_000)),
    ]);
  }
  if (browserProcess?.exitCode === null) {
    browserProcess.kill('SIGTERM');
    await Promise.race([
      once(browserProcess, 'exit'),
      new Promise((resolve) => setTimeout(resolve, 2_000)),
    ]);
  }
  if (xvfbProcess?.exitCode === null) {
    xvfbProcess.kill('SIGTERM');
    await Promise.race([
      once(xvfbProcess, 'exit'),
      new Promise((resolve) => setTimeout(resolve, 2_000)),
    ]);
  }
  if (x11TemporaryDirectory) {
    await rm(x11TemporaryDirectory, {
      recursive: true, force: true, maxRetries: 5, retryDelay: 200,
    });
  } else if (browserProfile) {
    await rm(browserProfile, {
      recursive: true, force: true, maxRetries: 5, retryDelay: 200,
    });
  }
  await new Promise((resolve) => server.close(resolve));
}

if (exitError) throw exitError;
