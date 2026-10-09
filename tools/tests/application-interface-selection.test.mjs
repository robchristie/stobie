import assert from 'node:assert/strict';
import test from 'node:test';
import { interfaceSelection, assertInterfaceCoverage } from '../application-interface-selection.mjs';

test('default qualification covers both applications on both hosts', () => {
  const selection = interfaceSelection([]);
  assert.deepEqual(selection, { apps: ['record-desk', 'lab'], hosts: ['browser', 'native'] });
  const reports = selection.apps.flatMap(app => selection.hosts.map(host => ({ app, host })));
  assertInterfaceCoverage(reports, selection);
  assert.throws(() => assertInterfaceCoverage(reports.slice(1), selection));
  assert.throws(() => assertInterfaceCoverage([...reports, reports[0]], selection));
});

test('parallel stage selections partition the complete four-journey surface', () => {
  const selections = [
    interfaceSelection(['--apps', 'lab', '--hosts', 'native']),
    interfaceSelection(['--apps', 'lab', '--hosts', 'browser']),
    interfaceSelection(['--apps', 'record-desk']),
  ];
  const reports = selections.flatMap(selection => selection.apps.flatMap(app => selection.hosts.map(host => ({ app, host }))));
  assertInterfaceCoverage(reports, interfaceSelection([]));
});

test('invalid, empty, duplicate and incomplete selectors fail closed', () => {
  for (const arguments_ of [
    ['--unknown', 'lab'], ['apps', 'lab'], ['--apps'], ['--apps', ''],
    ['--apps', 'lab,'], ['--apps', 'gallery'], ['--hosts', 'headless'],
    ['--apps', 'lab,lab'], ['--apps', 'lab', '--apps', 'record-desk'],
  ]) assert.throws(() => interfaceSelection(arguments_));
});
