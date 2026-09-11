#!/usr/bin/env python3
"""Prepare or execute the committed eleven-slot native screen; no infrastructure setup."""
import argparse
import fcntl
import importlib.util
import json
import math
import multiprocessing
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('native_cohort', ROOT / 'tools/viewer-acceptance-native-cohort.py')
cohort = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cohort)
PROTOCOL = ROOT / 'docs/viewer-acceptance-scheduling-build.json'
OWNED = ['docs/viewer-acceptance-scheduling-build.json',
         'tools/viewer-acceptance-scheduling-screen.py',
         'tools/tests/test_viewer_acceptance_scheduling_screen.py']
PAIRS = ['AB', 'BA', 'AB', 'BA', 'AB']
require, load, write = cohort.require, cohort.load, cohort.write


def slots():
    result = [dict(slot='conditioning-A', pair=None, arm='A', completion_pump='false')]
    for index, order in enumerate(PAIRS, 1):
        result += [dict(slot=f'pair-{index:02}-{arm}', pair=index, arm=arm,
                        completion_pump='false' if arm == 'A' else 'true') for arm in order]
    return result


def validate_contract(p):
    require(p['schema'] == 'viewer-acceptance-scheduling-build/1', 'wrong protocol schema')
    require(p['screen']['slots'] == slots(), 'frozen slot order changed')
    require(p['screen']['budget'] == dict(fresh_starts=11, conditioning=1, measured_pairs=5,
        warmups=0, retries=0, phase_seconds=60, outer_seconds=660), 'budget drift')
    require(p['screen']['captures'] == 0, 'screen has no optional captures')
    require(p['screen']['decision'] == dict(statistic='arithmetic mean of five paired B/A minus one ratios',
        warm_compressed_ratio_strictly_below=-0.05, other_ratio_at_most=0.05,
        absolute_gates='all eleven cells; inherited benchmark plus all three RSS observations'), 'decision drift')
    require(p['unchanged_ceiling_bytes'] == cohort.CEILING and
            p['original_observation_bytes'] == 278794240, 'historical resource bound drift')
    require(len(load(p['workload']['path'])) == 29, 'normal 29-action workload required')
    threshold = load(p['thresholds']['path'])
    require(threshold['workload_sha256'] == p['workload']['sha256'], 'workload binding drift')
    require(threshold['bounds']['worker_concurrency']['maximum'] == 1, 'one execution worker required')
    require(threshold['bounds']['process_peak_rss_bytes']['maximum'] == cohort.CEILING, 'RSS drift')


def arguments(p, group, slot, url):
    args = cohort.journey_arguments(p, group / slot['slot'], url, 1 if slot['pair'] is None else 2)
    return args + ['--completion-pump', slot['completion_pump']]


def validate_grant(g, commit, p):
    require(g.get('schema') == 'viewer_scheduling_screen_grant/1', 'screen grant required')
    for key, expected in dict(protocol_commit=commit, execute=True, screen_id=p['screen_id'],
                              no_concurrent_builds=True, resource_compile_complete=True,
                              authored_probe_owner_stopped=True).items():
        require(g.get(key) == expected, 'grant binding missing or different: ' + key)
    require(bool(g.get('granted_by')) and bool(g.get('granted_utc')), 'main attribution required')
    name = g['output_name']
    require(name.startswith('viewer-acceptance-scheduling-screen-') and Path(name).name == name,
            'fresh approved screen child required')
    require(g['display'].startswith(':') and g['display'][1:].isdigit(), 'local display required')
    require(g['url'].startswith('http://127.0.0.1:') and g['url'][17:].isdigit() and
            1024 <= int(g['url'][17:]) <= 65535, 'explicit local service URL required')


def build_processes():
    found = []
    for path in Path('/proc').glob('[0-9]*/comm'):
        try:
            if path.read_text().strip() in {'cargo', 'rustc', 'cc', 'cc1', 'cc1plus', 'c++', 'clang',
                                             'clang++', 'gcc', 'g++', 'ld', 'ld.lld', 'rust-lld', 'cmake', 'ninja', 'make'}:
                found.append(int(path.parent.name))
        except OSError:
            pass
    return found


def preflight(p, g, commit):
    """All identities fail closed before consuming the screen or starting a viewer."""
    validate_contract(p)
    validate_grant(g, commit, p)
    require(cohort.git(ROOT, 'rev-parse', 'HEAD').decode().strip() == commit, 'main must commit before execution')
    for name in OWNED:
        require(cohort.git(ROOT, 'show', commit + ':' + name) == cohort.read(ROOT / name),
                'uncommitted orchestration: ' + name)
    require(not cohort.git(ROOT, 'diff', 'HEAD'), 'execute from clean committed tracked source')
    actual = []
    records = [p[k] for k in ('native', 'service', 'benchmark', 'workload', 'thresholds', 'catalogue_contract')]
    records += p['frozen_helpers'] + p['graphics_files'] + p['web_files'] + p['linked_libraries']
    records += list(p['inputs'].values()) + [p['source']['archive']]
    records += p['build']['toolchain_files']
    records += list(p['orchestration'].values())
    for record in records:
        actual.append(cohort.verify(record))
    require(cohort.git(ROOT, 'rev-parse', p['source']['revision'] + '^{tree}').decode().strip() ==
            p['source']['tree'], 'candidate tree mismatch')
    for label, owner in p['owners'].items():
        require(cohort.git(owner['path'], 'rev-parse', 'HEAD').decode().strip() == owner['revision'], label + ' revision drift')
        require(not cohort.git(owner['path'], 'diff', 'HEAD'), label + ' tracked source dirty')
    require(cohort.STORE.resolve() == cohort.STORE, 'approved store alias drift')
    require(not (cohort.STORE / g['output_name']).exists(), 'output exists; no resume or rerun')
    for prior in cohort.STORE.glob('viewer-acceptance-scheduling-screen-*/execution.json'):
        require(load(prior)['screen_id'] != p['screen_id'], 'screen already consumed; no retries')
    for rep in p['representations']:
        for key in ('manifest', 'notice', 'lineage'):
            cohort.verify(rep[key])
    cohort.verify(p['historical_input_inventory'])
    inventory = cohort.input_inventory(p)
    require(inventory == load(p['historical_input_inventory']['path'])['input_inventory'],
            'representation payload or validity identity drift')
    for key, value in p['runtime_environment'].items():
        require(os.environ.get(key) == value, 'runtime environment drift: ' + key)
    require(os.environ.get('DISPLAY') == g['display'], 'display grant mismatch')
    require(not any(os.environ.get(k) for k in p['forbidden_environment']), 'unfrozen runtime override')
    require(not build_processes(), 'finish all builds before screen; no concurrent builds')
    for label, binary in [('service', p['service']['path']), ('display', p['xvfb'])]:
        record = g[label + '_identity']
        require(cohort.alive(record), label + ' lifetime unavailable')
        require(cohort.identity(f'/proc/{record["pid"]}/exe')['sha256'] == cohort.identity(binary)['sha256'],
                label + ' executable drift')
    service_args = ['serve']
    for rep in p['representations']:
        service_args += ['--representation', rep['path']]
    service_args += ['--listen', g['url'][7:], '--verify-payload', 'true', '--web', p['web_root']]
    actual_args = cohort.read(f'/proc/{g["service_identity"]["pid"]}/cmdline', 65536).decode().split('\0')
    require(actual_args[1:-1] == service_args, 'service input/listener/options drift')
    require(cohort.display_geometry(g['display']) == [1440, 900, 24], 'display geometry drift')
    with cohort.urllib.request.urlopen(g['url'] + '/catalogue', timeout=5) as response:
        catalogue = json.loads(response.read((4 << 20) + 1))
    require([dict(source_sha256=m['identity']['source_sha256'], bands=m['identity']['bands'],
                  width=m['identity']['profile']['width'], height=m['identity']['profile']['height'])
             for m in catalogue] == load(p['catalogue_contract']['path'])['sources'], 'live catalogue drift')
    return dict(files=actual, inputs=inventory,
                runtime_environment={k: os.environ.get(k) for k in p['runtime_environment']},
                main_commit=commit, orchestration_files=[cohort.identity(ROOT / n) for n in OWNED])


def one_cell(p, g, group, slot):
    """Observe the existing composed harness; preserve its measurements and admission."""
    args = arguments(p, group, slot, g['url'])
    context = multiprocessing.get_context('fork')
    receiver, sender = context.Pipe(duplex=False)
    worker = context.Process(target=cohort.journey_worker, args=(args, sender,
        group / (slot['slot'] + '-harness.log'), p['native']['path']))
    launch, maps, failures = None, [], []
    worker.start()
    sender.close()
    start = time.monotonic_ns()
    try:
        while True:
            now = time.monotonic_ns()
            if launch is None and receiver.poll():
                try:
                    launch = receiver.recv()
                    write(group / (slot['slot'] + '-launch.json'), launch)
                except EOFError:
                    pass
            if launch:
                if now >= launch['launch_monotonic_ns'] + 660 * 10**9 and cohort.alive(launch['identity']):
                    cohort.kill_owned(launch['identity'])
                    failures.append('fixed 660-second native deadline; incomplete evidence retained')
                if not maps and now >= launch['launch_monotonic_ns'] + 2 * 10**9:
                    maps.append(cohort.mapped_files(launch['identity']))
            if not worker.is_alive():
                break
            limit = launch['launch_monotonic_ns'] + 780 * 10**9 if launch else start + 120 * 10**9
            if now >= limit:
                failures.append('bounded harness setup/export failure')
                break
            time.sleep(0.02)
    finally:
        if launch:
            cohort.kill_owned(launch['identity'])
        if worker.is_alive():
            try:
                cohort.kill_owned(cohort.process_identity(worker.pid))
            except (OSError, ValueError):
                worker.kill()
        worker.join(timeout=3)
        receiver.close()
    if launch is None:
        failures.append('no observed native launch; slot consumed without replacement')
    return dict(**slot, arguments=args, launch=launch, harness_exit=worker.exitcode,
                failures=failures, mapped_files=maps, library_files=cohort.bind_mapped_files(maps))


def frozen_slots(run, retain):
    rows = []
    for slot in slots():
        try:
            row = run(slot)
        except Exception as error:
            row = dict(**slot, failures=[type(error).__name__ + ': ' + str(error)], launch=None)
        retain(row)
        rows.append(row)
    return rows


def diagnostic_summary(stages):
    phases, pacing, seen = [], [], set()
    intervals = [('dispatch_to_worker_ms', 'dispatch_ms', 'worker_started_ms'),
                 ('worker_execution_ms', 'worker_started_ms', 'worker_finished_ms'),
                 ('finish_to_publish_ms', 'worker_finished_ms', 'published_ms'),
                 ('publish_to_receive_ms', 'published_ms', 'received_ms'),
                 ('receive_to_ui_ms', 'received_ms', 'ui_drained_ms')]
    for stage in stages:
        phases.append({key: stage.get(key) for key in ('phase_label', 'runtime_epoch', 'completion_pump',
            'diagnostic_options', 'phase_first_useful_ms', 'phase_settled_ms', 'phase_data_ready_ms',
            'cancelled', 'stale', 'in_flight', 'peak_in_flight', 'decoded_accounted_bytes',
            'decoded_peak_bytes', 'gpu_bytes', 'gpu_peak_bytes', 'process_peak_rss_bytes',
            'worker', 'pacing', 'diagnostic_trace_dropped', 'events_dropped',
            'completion_pump_max_batch', 'completion_pump_max_turn_ms', 'completion_pump_max_receive_ms')})
        for event in stage.get('events', []):
            sample = event.get('pacing')
            key = json.dumps([stage.get('runtime_epoch'), event], sort_keys=True)
            if sample is None or key in seen:
                continue
            seen.add(key)
            differences = {}
            for name, start, end in intervals:
                a, b = sample.get(start), sample.get(end)
                differences[name] = b - a if all(isinstance(v, (int, float)) and math.isfinite(v)
                                                for v in (a, b)) else None
            pacing.append(dict(runtime_epoch=stage.get('runtime_epoch'), token=event.get('token'),
                               differences_ms=differences))
    return dict(phases=phases, retained_same_clock_differences=pacing,
        boundary='Native shared pacing clock only; deduplicated epoch/event snapshots. Negative differences remain unclipped. Missing or overwritten events unavailable; no inference of settlement, causality or complete trace coverage.')


def assess(p, group, row):
    output = group / row['slot']
    report = dict(**row, absolute_pass=False)
    trace = load(output / 'composed-trace.json')
    app = load(output / 'app.json')
    stages = load(output / 'app.json.stages.json')
    identity = load(output / 'run-identity.json')
    require(identity['arguments']['completion_pump'] == row['completion_pump'] and
            identity['builds']['native'] == p['native']['sha256'], 'actual arm/binary mismatch')
    require(all(s.get('completion_pump') is (row['arm'] == 'B') for s in [app, *stages]),
            'actual snapshot arm missing or wrong')
    gpu = app.get('gpu_adapter', '')
    require(all(value in gpu for value in ('NVIDIA GeForce RTX 3090', '610.43.03', 'Vulkan')),
            'actual adapter/driver/backend differs from freeze')
    command = [p['benchmark']['path'], 'journey', str(output / 'composed-trace.json'), p['thresholds']['path']]
    with (group / (row['slot'] + '-benchmark.json')).open('xb') as out, \
            (group / (row['slot'] + '-benchmark.log')).open('xb') as err:
        result = subprocess.run(command, stdout=out, stderr=err, timeout=30, check=False)
    observations = trace['observations']
    rss = {k: observations.get(k, {}).get('value') for k in
           ('process_peak_rss_bytes', 'process_group_sampled_peak_rss_bytes', 'observed_process_high_water_sum_bytes')}
    # The original sampler HWM sum is a separate boundary, never added to RSS.
    if rss['observed_process_high_water_sum_bytes'] is None:
        memory = load(output / 'process-memory.json')
        values = memory.get('observed_pid_high_water_bytes', {})
        rss['observed_process_high_water_sum_bytes'] = sum(values.values()) if values else None
    report.update(benchmark=dict(command=command, exit_code=result.returncode), observations=observations,
                  rss=rss, actual_arm=app['completion_pump'], actual_configuration=identity,
                  diagnostics=diagnostic_summary(stages), actual_gpu=app.get('gpu_adapter'),
                  diagnostic_evidence={name: cohort.identity(output / name) for name in
                    ('app.json', 'app.json.stages.json', 'process-memory.json', 'process-memory-diagnostics.json',
                     'memory-phase-markers.jsonl', 'memory-phase-markers.allocations.jsonl',
                     'service-before.json', 'service-after.json', 'run-identity.json')},
                  absolute_pass=bool(row.get('launch') and row.get('harness_exit') == 0 and not row['failures'] and
                    result.returncode == 0 and trace['completed'] and not trace['failures'] and
                    all(v is not None and v <= cohort.CEILING for v in rss.values())))
    return report


def decision(rows, thresholds):
    """Descriptive screen only. Recovery is deliberately never manufactured here."""
    result = dict(native_screen_promising=False, acceptance=False, confirmation_authorised=False,
                  recovery='separate, unproved; mandatory inherited seven-event gate',
                  resource_cohort='new binary still requires five starts / ten cycles if retained', ratios={})
    if len(rows) != 11 or any(not r.get('absolute_pass') for r in rows):
        return dict(result, disposition='reject', reason='failed or unavailable declared cell; all retained')
    fields = set(thresholds['bounds'])
    fields.update(k for r in rows for k, v in r['observations'].items() if v.get('unit') == 'ms')
    fields.update(rows[0]['rss'])
    for field in sorted(fields):
        ratios = []
        for index in range(1, 6):
            pair = {r['arm']: r for r in rows if r['pair'] == index}
            values = [pair[arm]['rss'].get(field, pair[arm]['observations'].get(field, {}).get('value'))
                      for arm in ('A', 'B')]
            if any(not isinstance(v, (int, float)) or not math.isfinite(v) or v < 0 for v in values):
                return dict(result, disposition='reject', reason='missing comparable observation: ' + field)
            a, b = values
            if a == 0 and b != 0:
                return dict(result, disposition='reject', reason='zero baseline increased: ' + field)
            ratios.append((b - a) / a if a else 0.0)
        result['ratios'][field] = dict(paired=ratios, mean=sum(ratios) / 5)
    promising = result['ratios']['warm_compressed_ms']['mean'] < -0.05 and all(
        row['mean'] <= 0.05 for field, row in result['ratios'].items() if field != 'warm_compressed_ms')
    return dict(result, native_screen_promising=promising,
                disposition='native-screen-promising-recovery-unproved' if promising else 'reject',
                reason='descriptive five-pair screen; no confirmation or acceptance grant')


def execute(p, g, commit, grant_path):
    binding = preflight(p, g, commit)
    source = cohort.module('viewer-real-scene-source')
    group = source.fresh_group(cohort.STORE, g['output_name'],
        'One A conditioning journey then AB BA AB BA AB native scheduling screen; diagnostic quality-rejected Mansfield PAN4/RGB12; inputs unchanged; no captures.')
    write(group / 'execution.json', dict(screen_id=p['screen_id'], protocol=p, grant=g,
        grant_identity=cohort.identity(grant_path), binding=binding, budget_started_monotonic_ns=time.monotonic_ns(),
        boundary='all declared slots consumed even on failure; service warm after conditioning, OS uncontrolled'))
    os.environ['TMPDIR'] = str(group)
    def run(slot):
        require(not build_processes(), 'concurrent build detected; slot unavailable, no retry')
        for label in ('service', 'display'):
            require(cohort.alive(g[label + '_identity']), label + ' lifetime changed; no replacement')
        cohort.verify(p['native'])
        return one_cell(p, g, group, slot)
    rows = frozen_slots(run, lambda r: write(group / (r['slot'] + '-wrapper.json'), r))
    reports = []
    # All resource compilation predates timing; benchmark analysis runs after the eleven slots.
    for row in rows:
        try:
            report = assess(p, group, row)
        except Exception as error:
            report = dict(**row, absolute_pass=False, assessment_failure=type(error).__name__ + ': ' + str(error))
        write(group / (row['slot'] + '-summary.json'), report)
        reports.append(report)
    result = decision(reports, load(p['thresholds']['path']))
    try:
        after = cohort.input_inventory(p)
        write(group / 'inputs-after.json', dict(unchanged=after == binding['inputs'], inventory=after))
        require(after == binding['inputs'], 'input identity changed during screen')
    except Exception as error:
        result.update(disposition='reject', native_screen_promising=False, input_failure=str(error))
    write(group / 'summary.json', dict(result, slots=[r['slot'] for r in reports],
        native_launches_observed=sum(bool(r.get('launch')) for r in reports),
        same_clock_differences='Use retained same-domain request stamps and monotonic marker/read brackets only; preserve negative differences and ring loss. Never subtract Unix from monotonic, GPU or browser clocks.',
        instrument='diagnostic only; original three-frame settlement unchanged',
        historical_rss_failure_bytes=p['original_observation_bytes'], ceiling_bytes=cohort.CEILING))
    return 0 if result['native_screen_promising'] else 4


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--grant', type=Path)
    parser.add_argument('--protocol-commit')
    args = parser.parse_args()
    p = load(PROTOCOL)
    validate_contract(p)
    if not args.execute:
        print('Prepared only: conditioning A; AB BA AB BA AB; 11 starts; no captures/retries; no launches.')
        for slot in slots():
            print(slot['slot'], ' '.join(arguments(p, Path('APPROVED_FRESH_GROUP'), slot, 'FROZEN_SERVICE_URL')))
        return 0
    require(args.grant and args.protocol_commit, 'later main grant and committed protocol required')
    # Lock the immutable binary inode without writing it: two screen wrappers cannot race preflight.
    with Path(p['native']['path']).open('rb') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return execute(p, load(args.grant), args.protocol_commit, args.grant)


if __name__ == '__main__':
    raise SystemExit(main())
