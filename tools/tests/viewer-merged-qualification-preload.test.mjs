// Rejections must precede the entry point; no service, browser, mounts or protected reads.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const preload = fileURLToPath(new URL('../viewer-merged-qualification-preload.mjs', import.meta.url));
for (const [name, values, error] of [
  ['missing pin', {}, 'missing approved pinned merged temporary alias'],
  ['outside store', { VIEWER_MERGED_ALIAS: '/tmp/alias.json', VIEWER_MERGED_ALIAS_SHA256: 'a'.repeat(64) },
    'missing approved pinned merged temporary alias'],
  ['unexpected entry', {
    VIEWER_MERGED_ALIAS: '/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1/unused/alias.json',
    VIEWER_MERGED_ALIAS_SHA256: 'a'.repeat(64),
  }, 'unexpected merged journey entry point'],
]) {
  test(name + ' rejects before entry evaluation', () => {
    const env = { ...process.env };
    for (const key of ['NODE_OPTIONS', 'VIEWER_MERGED_ALIAS', 'VIEWER_MERGED_ALIAS_SHA256']) delete env[key];
    const result = spawnSync(process.execPath,
      ['--import', preload, '--eval', 'process.stdout.write("ENTRY EXECUTED")'],
      { env: { ...env, ...values }, encoding: 'utf8' });
    assert.equal(result.status, 1);
    assert.match(result.stderr, new RegExp(error));
    assert.equal(result.stdout, '');
  });
}
