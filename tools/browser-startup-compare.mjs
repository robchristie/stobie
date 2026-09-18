// Opt-in comparison using the gates declared before candidate screening.
// Never invoked as a CI wall-clock gate.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { gunzipSync } from 'node:zlib';
const read = path => JSON.parse(path.endsWith('.gz') ? gunzipSync(readFileSync(path)) : readFileSync(path, 'utf8'));
const [baselinePath, finalPath] = process.argv.slice(2);
assert(baselinePath && finalPath, 'usage: node tools/browser-startup-compare.mjs BASELINE.json[.gz] FINAL.json[.gz]');
const baseline = read(baselinePath), finalist = read(finalPath);
assert(!baseline.failure && !finalist.failure, 'cannot qualify a failed benchmark');
assert.deepEqual(baseline.browser.launch, finalist.browser.launch, 'browser flags differ');
assert.equal(baseline.browser.version, finalist.browser.version, 'browser versions differ');
assert.deepEqual(baseline.browser.viewport, finalist.browser.viewport, 'viewports differ');
assert.equal(baseline.host.cpu, finalist.host.cpu, 'hardware differs');
const comparisons = [];
for (const [group, metrics] of Object.entries(finalist.summary)) {
  const [app, profile, visit] = group.split('/');
  const settings = report => report.samples.filter(s => s.app === app && s.profile === profile && s.visit === visit).map(s => JSON.stringify(s.network));
  assert.deepEqual([...new Set(settings(baseline))], [...new Set(settings(finalist))], `network profile differs for ${group}`);
  for (const metric of ['rendering_opportunity_proxy', 'first_useful_content']) {
    const before = baseline.summary[group]?.[metric], after = metrics[metric];
    assert(before?.count > 0 && after?.count > 0, `missing ${group}/${metric}`);
    const allowedRegressionMs = Math.max(25, before.max - before.min, before.median * 0.15);
    const reductionMs = before.median - after.median;
    comparisons.push({ group, metric, before, after, reductionMs, reductionPercent: 100 * reductionMs / before.median,
      allowedRegressionMs, regressionGatePassed: -reductionMs <= allowedRegressionMs,
      meaningfulNetworkGain: group === 'lab/network/cold' ? reductionMs >= 500 && reductionMs / before.median >= 0.2 : null });
  }
}
const result = { schema: 1, baselineManifestSha256: baseline.manifestSha256, finalManifestSha256: finalist.manifestSha256,
  timingMeaning: 'CPU/frame opportunity and application content proxies, independently corroborated by real input/readback; not GPU presentation',
  comparisons, regressionGatesPassed: comparisons.every(c => c.regressionGatePassed),
  primaryGainPassed: comparisons.some(c => c.meaningfulNetworkGain !== null) ? comparisons.filter(c => c.meaningfulNetworkGain !== null).every(c => c.meaningfulNetworkGain) : null };
console.log(JSON.stringify(result, null, 2));
