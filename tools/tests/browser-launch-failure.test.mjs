// A browser smoke whose Chromium fails to start reports the launch error, not
// a TypeError from its failure handler reading a page that never opened.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

const root = resolve(import.meta.dirname, '../..');
const smokes = ['chat', 'copy', 'dialog', 'icon-actions', 'markdown', 'navigation', 'reading-text', 'responsive-navigation', 'status-chip'];

function run(script, environment) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(process.execPath, [script], { cwd: root, env: { ...process.env, ...environment }, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    child.stdout.on('data', chunk => { output += chunk; });
    child.stderr.on('data', chunk => { output += chunk; });
    child.on('error', reject);
    child.on('close', code => resolvePromise({ code, output }));
  });
}

for (const smoke of smokes) {
  test(`${smoke} browser smoke reports a Chromium launch failure`, async () => {
    const scratch = await mkdtemp(join(tmpdir(), 'stobie-launch-'));
    try {
      const missing = join(scratch, 'missing-chromium');
      const { code, output } = await run(`tools/${smoke}-browser-smoke.mjs`, {
        STOBIE_CHROMIUM: missing, STOBIE_EVIDENCE_DIR: join(scratch, 'evidence'),
      });
      assert.notEqual(code, 0);
      assert.match(output, /missing-chromium/);
      assert.doesNotMatch(output, /Cannot read properties of undefined/);
    } finally {
      await rm(scratch, { recursive: true, force: true });
    }
  });
}
