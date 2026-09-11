// Validate the inherited temporary environment before the journey imports Playwright.
// Main supplies a pinned alias and grants execution; this module grants nothing.
import assert from 'node:assert/strict';
import { readFile, realpath, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { STORE, beneath, sha256, temporaryDirectory } from './viewer-acceptance-browser-masks.mjs';

const capsule = process.env.VIEWER_MERGED_ALIAS;
const pin = process.env.VIEWER_MERGED_ALIAS_SHA256;
assert(capsule && beneath(STORE, capsule) && /^[a-f0-9]{64}$/.test(pin ?? ''),
  'missing approved pinned merged temporary alias');
const entries = ['viewer-composed-browser.mjs', 'viewer-browser-recovery.mjs']
  .map(name => join(import.meta.dirname, name));
assert(entries.includes(resolve(process.argv[1] ?? '')), 'unexpected merged journey entry point');
assert.equal(await realpath(capsule), capsule, 'alias capsule path changed');
const bytes = await readFile(capsule);
assert.equal(sha256(bytes), pin, 'alias capsule digest changed');
const alias = JSON.parse(bytes);
const group = dirname(alias.backing);
assert.equal(dirname(capsule), group, 'alias capsule outside execution group');
assert.equal(process.env.TMPDIR, '/tmp', 'TMPDIR must precede Playwright import');
const output = await realpath(process.argv[3]);
assert(beneath(group, output), 'journey output outside execution group');
for (const key of ['XDG_CACHE_HOME', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME']) {
  const path = process.env[key];
  assert(path && beneath(group, path) && await realpath(path) === path,
    `${key} must resolve inside execution group`);
}
const temporary = await temporaryDirectory(alias, output);
await writeFile(join(output, 'temporary-alias.json'),
  JSON.stringify(temporary.provenance, null, 2) + '\n', { flag: 'wx' });
