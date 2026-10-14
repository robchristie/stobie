// Authored environments, temporary sysroot directories and source text only: no Playwright.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { uiLibraryEnvironment, uiLibrarySource } from '../browser-launch.mjs';

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
