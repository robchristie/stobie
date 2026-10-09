import assert from 'node:assert/strict';

export function interfaceSelection(arguments_) {
  const allowed = { apps: ['record-desk', 'lab'], hosts: ['browser', 'native'] };
  const selection = { ...allowed };
  const seen = new Set();
  for (let i = 0; i < arguments_.length; i += 2) {
    const key = arguments_[i].slice(2);
    assert.ok(arguments_[i].startsWith('--') && Object.hasOwn(allowed, key), `Unknown interface selector: ${arguments_[i]}`);
    assert.ok(!seen.has(key), `Repeated interface selector: ${key}`);
    seen.add(key);
    const values = arguments_[i + 1]?.split(',');
    assert.ok(values?.length && values.every(value => allowed[key].includes(value)), `Invalid interface ${key}`);
    assert.equal(new Set(values).size, values.length, `Repeated interface ${key}`);
    selection[key] = values;
  }
  return selection;
}

export function assertInterfaceCoverage(reports, selection) {
  const expected = selection.apps.flatMap(app => selection.hosts.map(host => `${app}/${host}`)).sort();
  const observed = reports.map(report => `${report.app}/${report.host}`).sort();
  assert.deepEqual(observed, expected, 'Every selected application/host journey must complete exactly once');
}
