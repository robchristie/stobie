// Authored environments, platforms, GPU reports, temporary sysroot directories and source text only: no Playwright.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { assertGpuCompositing, expectedBrowserRenderer, hostedLinuxWebGpuLaunchOptions, rendererFlags, swiftShaderWebGpuFlags, uiLibraryEnvironment, uiLibrarySource } from '../browser-launch.mjs';

function launchOptions(headful, platform = 'linux') {
  const previous = process.env.STOBIE_BROWSER_HEADFUL;
  if (headful === undefined) delete process.env.STOBIE_BROWSER_HEADFUL;
  else process.env.STOBIE_BROWSER_HEADFUL = headful;
  try {
    return hostedLinuxWebGpuLaunchOptions(platform);
  } finally {
    if (previous === undefined) delete process.env.STOBIE_BROWSER_HEADFUL;
    else process.env.STOBIE_BROWSER_HEADFUL = previous;
  }
}

test('headless Linux launch selects ANGLE SwiftShader instead of the software-compositing default', () => {
  const options = launchOptions(undefined);
  assert.equal(options.headless, true);
  assert.ok(options.args.includes('--use-gl=angle'));
  assert.equal(options.args.filter(arg => arg.startsWith('--use-angle=')).join(), '--use-angle=swiftshader');
  assert.ok(!options.args.includes('--ozone-platform=x11'));
});

test('headful launch keeps the X11 route and the browser GL default', () => {
  const options = launchOptions('1');
  assert.equal(options.headless, false);
  assert.ok(options.args.includes('--ozone-platform=x11'));
  assert.ok(!options.args.some(arg => arg.startsWith('--use-gl=') || arg.startsWith('--use-angle=')));
});

test('headless launch elsewhere keeps the browser GL default', () => {
  const options = launchOptions(undefined, 'darwin');
  assert.equal(options.headless, true);
  assert.ok(!options.args.some(arg => arg.startsWith('--use-gl=') || arg.startsWith('--use-angle=')));
});

test('a WebGL2 run withholds only the WebGPU adapter flags', () => {
  assert.equal(expectedBrowserRenderer({}), 'webgpu');
  assert.equal(expectedBrowserRenderer({ STOBIE_BROWSER_RENDERER: 'webgl2' }), 'webgl2');
  assert.throws(() => expectedBrowserRenderer({ STOBIE_BROWSER_RENDERER: 'webgl' }), /webgpu or webgl2/);
  const flags = swiftShaderWebGpuFlags();
  assert.deepEqual(rendererFlags(flags, 'webgpu'), flags);
  const webgl2 = rendererFlags(flags, 'webgl2');
  assert.ok(!webgl2.some(flag => /webgpu|dawn/i.test(flag)), webgl2.join(' '));
  assert.ok(webgl2.includes('--use-vulkan=swiftshader') && webgl2.includes('--enable-unsafe-swiftshader'));
});

test('GPU compositing check accepts enabled compositing', () => {
  assert.doesNotThrow(() => assertGpuCompositing({ featureStatus: { gpu_compositing: 'enabled' } }));
});

test('GPU compositing check rejects the headless software fallback', () => {
  assert.throws(() => assertGpuCompositing({
    featureStatus: { gpu_compositing: 'disabled_software', vulkan: 'disabled_off' },
    auxAttributes: { skiaBackendType: 'None' },
  }), /GPU compositing is disabled_software \(Vulkan disabled_off, Skia backend None\)/);
  assert.throws(() => assertGpuCompositing({}), /GPU compositing is unreported/);
});

async function withRoot(provisioned, check) {
  const root = await mkdtemp(join(tmpdir(), 'stobie-browser-launch-'));
  try {
    if (provisioned) await mkdir(join(root, '.tools/sysroot/usr/lib'), { recursive: true });
    await check(root, join(root, '.tools/sysroot/usr/lib'));
  } finally { await rm(root, { recursive: true, force: true }); }
}

test('a provisioned sysroot precedes inherited library paths without mutating the caller', () => withRoot(true, (root, libraries) => {
  const inherited = { HOME: '/authored/home', LD_LIBRARY_PATH: '/authored/lib' };
  const environment = uiLibraryEnvironment(inherited, root);
  assert.equal(environment.LD_LIBRARY_PATH, `${libraries}:/authored/lib`);
  assert.equal(environment.HOME, '/authored/home');
  assert.equal(inherited.LD_LIBRARY_PATH, '/authored/lib');
  assert.equal(uiLibrarySource(inherited, root), 'repository-sysroot');
}));

test('an unset library path does not gain an empty current-directory entry', () => withRoot(true, (root, libraries) => {
  assert.equal(uiLibraryEnvironment({}, root).LD_LIBRARY_PATH, libraries);
  assert.equal(uiLibraryEnvironment({ LD_LIBRARY_PATH: '' }, root).LD_LIBRARY_PATH, libraries);
}));

test('system UI libraries leave the inherited environment unchanged', () => withRoot(true, root => {
  const inherited = { STOBIE_USE_SYSTEM_UI_LIBS: '1', LD_LIBRARY_PATH: '/authored/lib' };
  assert.deepEqual(uiLibraryEnvironment(inherited, root), inherited);
  assert.equal(uiLibrarySource(inherited, root), 'system');
}));

test('an unprovisioned sysroot is reported as system libraries and not added', () => withRoot(false, root => {
  const inherited = { LD_LIBRARY_PATH: '/authored/lib' };
  assert.deepEqual(uiLibraryEnvironment(inherited, root), inherited);
  assert.equal(uiLibrarySource(inherited, root), 'system');
}));

test('production startup smoke and benchmark launch Chromium with the UI library environment', async () => {
  // The production verification stage runs these directly, outside the sysroot shell wrappers.
  for (const script of ['browser-startup-smoke.mjs', 'browser-startup-benchmark.mjs']) {
    const source = await readFile(new URL(`../${script}`, import.meta.url), 'utf8');
    const launches = source.match(/chromium\.launch\w*\([^;]*\)/g) ?? [];
    assert.ok(launches.length > 0, `${script} must launch Chromium`);
    for (const launch of launches) assert.match(launch, /env: uiLibraryEnvironment\(\)/, `${script}: ${launch}`);
  }
});
