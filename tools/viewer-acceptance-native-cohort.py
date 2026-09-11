#!/usr/bin/env python3
"""Fixed native cohort wrapper. Execution requires a later, committed grant."""
import argparse
import ctypes
import fcntl
import hashlib
import importlib.util
import json
import multiprocessing
import os
from pathlib import Path
import platform
import resource
import signal
import socket
import subprocess
import sys
import time
import traceback
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
STORE = Path('/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1')
PROTOCOL = ROOT / 'docs/viewer-acceptance-native-cohort.json'
REPAIRED_PROTOCOL = ROOT / 'docs/viewer-acceptance-native-repaired-cohort.json'
REPAIRED_OUTPUT = 'viewer-acceptance-native-cohort-mansfield-repaired-01'
ORIGINAL_SHA256 = '68dbbdee8032f9261c527e41c5814fea371024ae40e880662fbe5252ce8c9816'
IDENTITY_FIELDS = {'status', 'cohort_id', 'runtime_revision', 'source_build_receipt_commit',
                   'harness_identity', 'native', 'build_receipt', 'web_root', 'web_files'}
OWNED = ['tools/viewer-acceptance-native-cohort.py',
         'tools/tests/test_viewer_acceptance_native_cohort.py',
         'docs/viewer-acceptance-native-cohort.md',
         'docs/viewer-acceptance-native-cohort.json']
OFFSETS = (2, 5)
CEILING = 243269632


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path, limit=64 << 20):
    with Path(path).open('rb') as stream:
        raw = stream.read(limit + 1)
    require(len(raw) <= limit, 'bounded input exceeded: ' + str(path))
    return raw


def load(path, limit=64 << 20):
    return json.loads(read(path, limit))


def write(path, value):
    raw = json.dumps(value, indent=2, allow_nan=False) + '\n'
    require(len(raw.encode()) <= 16 << 20, 'bounded summary exceeded')
    with Path(path).open('x') as stream:
        stream.write(raw)


def identity(path):
    path = Path(path)
    with path.open('rb') as stream:
        sha = hashlib.file_digest(stream, 'sha256').hexdigest()
    return dict(path=str(path), resolved_path=str(path.resolve()),
                bytes=path.stat().st_size, sha256=sha)


def verify(record):
    actual = identity(record['path'])
    require(actual['sha256'] == record['sha256'], 'identity drift: ' + record['path'])
    return actual


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT / 'tools' / (name + '.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args], timeout=30)


def committed(commit, protocol_path=PROTOCOL):
    require(git(ROOT, 'rev-parse', 'HEAD').decode().strip() == commit, 'grant must bind current full HEAD')
    names = OWNED + (['docs/viewer-acceptance-native-repaired-cohort.json',
                     'docs/viewer-acceptance-native-repaired-cohort.md',
                     'docs/viewer-acceptance-repaired-build.json']
                    if protocol_path == REPAIRED_PROTOCOL else [])
    for name in names:
        require(git(ROOT, 'show', commit + ':' + name) == read(ROOT / name), 'uncommitted protocol: ' + name)
    return {name: identity(ROOT / name) for name in names}


def validate_grant(grant, commit, group, display, port, protocol_path=PROTOCOL):
    require(grant.get('schema') == 'viewer_native_cohort_grant/1', 'execution grant schema required')
    for key, expected in dict(protocol_commit=commit, output_name=group,
                              display=display, port=port, execute=True,
                              authored_probe_owner_stopped=True).items():
        require(grant.get(key) == expected, 'grant binding missing or different: ' + key)
    require(bool(grant.get('granted_by')) and bool(grant.get('granted_utc')), 'coordinator attribution required')
    if protocol_path == REPAIRED_PROTOCOL:
        require(group == REPAIRED_OUTPUT, 'fixed repaired output required')
        require(grant.get('protocol_path') == str(protocol_path.relative_to(ROOT)), 'selected protocol path binding required')
        require(grant.get('protocol_sha256') == identity(protocol_path)['sha256'], 'selected protocol digest binding required')


def validate_contract(protocol):
    require(protocol['budget'] == dict(fresh_starts=5, warmups=0, retries=0,
        cycles_per_start=10, phase_seconds=60, outer_seconds=660), 'fixed budget drift')
    require(protocol['captures']['offset_seconds'] == list(OFFSETS), 'capture schedule drift')
    thresholds = load(protocol['thresholds']['path'])
    inherited = load(protocol['inherited_thresholds']['path'])
    for field in ('bounds', 'require_hardware_gpu', 'required_events', 'baseline_evidence_sha256'):
        require(thresholds[field] == inherited[field], 'inherited threshold changed: ' + field)
    require(thresholds['bounds']['process_peak_rss_bytes']['maximum'] == CEILING, 'RSS ceiling drift')
    workload = load(protocol['workload']['path'])
    ordinary = load(ROOT / 'apps/emuella-viewer/real-scene-workload.json')
    require(workload[:len(ordinary)] == ordinary, 'ordinary actions changed')
    require([s.get('diagnostic_cycle') for s in workload[len(ordinary):]] == list(range(1, 11)),
            'ten consecutive cycles required')
    require(all(s['intent'] == {'kind': 'clear_display_cache'} for s in workload[len(ordinary):]), 'release action drift')
    require(thresholds['workload_sha256'] == identity(protocol['workload']['path'])['sha256'], 'workload binding drift')


def select_protocol(path=PROTOCOL):
    path = Path(path).absolute()
    require(path in (PROTOCOL, REPAIRED_PROTOCOL) and path.resolve() == path,
            'only explicitly pinned repository protocols are supported')
    require(identity(PROTOCOL)['sha256'] == ORIGINAL_SHA256, 'original protocol identity drift')
    protocol = load(path)
    if path == REPAIRED_PROTOCOL:
        original = load(PROTOCOL)
        require(protocol.keys() == original.keys(), 'alternate protocol fields drift')
        for key in original.keys() - IDENTITY_FIELDS:
            expected = original[key]
            if key == 'frozen_helpers':
                # The committed optional scheduling argument is never supplied by
                # this cohort. Pin the five-line additive helper change exactly.
                expected = [dict(row, bytes=28425,
                    sha256='706232cf5cac2c39deb0124aa7e0c40fb078298f42946e4b10044cd0e34d7a1f')
                    if row['path'] == str(ROOT / 'tools/viewer-composed-journey.py') else row
                    for row in expected]
            require(protocol[key] == expected, 'original policy drift: ' + key)
        require(protocol['runtime_revision'] == '8633754fa28f2ca34f159a7368e0b8e7e953d205',
                'repaired runtime identity drift')
        require(protocol['cohort_id'] == 'repaired-frozen-v1-mansfield-memory-five-capture-v1',
                'repaired cohort identity drift')
        require(protocol['build_receipt']['path'] == str(ROOT / 'docs/viewer-acceptance-repaired-build.json'),
                'repaired build receipt path drift')
        verify(protocol['build_receipt'])
        build = load(protocol['build_receipt']['path'])
        require(build['runtime_source']['revision'] == protocol['runtime_revision']
                and build['native'] == protocol['native']
                and build['web']['root'] == protocol['web_root'], 'repaired build binding drift')
        verify(build['web']['manifest'])
        manifest = load(build['web']['manifest']['path'])
        require(protocol['web_files'] == [dict(entry, path=str(Path(protocol['web_root']) / entry['path']))
                for entry in manifest], 'repaired static manifest drift')
    validate_contract(protocol)
    return path, protocol


def preflight(protocol):
    validate_contract(protocol)
    records = [protocol[k] for k in ('native', 'service', 'workload', 'thresholds',
               'inherited_thresholds', 'catalogue_contract', 'build_receipt', 'benchmark')]
    records += protocol['frozen_helpers'] + protocol['graphics_files'] + [protocol['captures']['tool']]
    for record in records:
        verify(record)
    for name, owner in protocol['owners'].items():
        require(git(owner['path'], 'rev-parse', 'HEAD').decode().strip() == owner['revision'], name + ' revision drift')
        require(not git(owner['path'], 'diff', 'HEAD'), name + ' tracked source dirty')
    actual_sources = []
    for item in protocol['representations']:
        verify(item['manifest'])
        verify(item['notice'])
        verify(item['lineage'])
        m = load(item['manifest']['path'])['identity']
        actual_sources.append(dict(source_sha256=m['source_sha256'], bands=m['bands'],
                                   width=m['profile']['width'], height=m['profile']['height']))
    require(actual_sources == load(protocol['catalogue_contract']['path'])['sources'], 'source/band/geometry mismatch')
    require(STORE.resolve() == STORE, 'approved store alias changed')
    for item in protocol['web_files']:
        verify(item)
    return dict(python=sys.version, executable=identity(sys.executable), platform=platform.platform(),
                uname=list(os.uname()), environment={k: os.environ.get(k) for k in
                ('LD_PRELOAD', 'LD_LIBRARY_PATH', 'WGPU_BACKEND', 'VK_ICD_FILENAMES',
                 'WAYLAND_DISPLAY', 'DISPLAY', 'XDG_RUNTIME_DIR', 'MALLOC_ARENA_MAX', 'GLIBC_TUNABLES')})


def input_inventory(protocol):
    """Stream hashes only; never rewrite/reprepare input or allocate a complete raster."""
    result = []
    for rep in protocol['representations']:
        root = Path(rep['path'])
        require(root.resolve() == root and root.is_relative_to(STORE), 'representation store alias changed')
        files = sorted(p for p in root.rglob('*') if p.is_file())
        require(len(files) <= 10000, 'representation file cap exceeded')
        require(all(p.resolve().is_relative_to(root) for p in files), 'representation file escaped approved input')
        result.append(dict(path=str(root), files=[identity(p) for p in files]))
    return result


def process_identity(pid):
    raw = read(Path('/proc') / str(pid) / 'stat', 16384).decode()
    tail = raw[raw.rfind(')') + 2:].split()
    return dict(pid=pid, start_time_ticks=int(tail[19]), process_group=int(tail[2]))


def alive(record):
    try:
        raw = read(Path('/proc') / str(record['pid']) / 'stat', 16384).decode()
        state = raw[raw.rfind(')') + 2:].split()[0]
        return state not in ('Z', 'X') and process_identity(record['pid']) == record
    except (OSError, ValueError):
        return False


def kill_owned(record, sig=signal.SIGKILL):
    if record and alive(record):
        require(record['process_group'] == record['pid'], 'refuse to signal an unowned group')
        try:
            os.killpg(record['pid'], sig)
        except ProcessLookupError:
            pass


def stop(process, record):
    if process is None:
        return
    if record:
        kill_owned(record, signal.SIGTERM)
    elif process.poll() is None:
        # A child that failed its initial /proc read is still owned by Popen.
        process.terminate()
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        if record:
            kill_owned(record)
        else:
            process.kill()
        process.wait(timeout=3)


def mapped_files(record):
    started = time.monotonic_ns()
    try:
        require(alive(record), 'native process exited before library observation')
        paths = set()
        for line in read(Path('/proc') / str(record['pid']) / 'maps', 2 << 20).decode().splitlines():
            fields = line.split(None, 5)
            if len(fields) == 6 and fields[5].startswith('/'):
                paths.add(fields[5])
        require(len(paths) <= 512, 'mapped object cap exceeded')
        rows = []
        for name in sorted(paths):
            try:
                stat = Path(name).stat()
                rows.append(dict(path=name, stat=[stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns]))
            except OSError as error:
                rows.append(dict(path=name, unavailable=str(error)))
        require(alive(record), 'native identity changed during library observation')
        return dict(identity=record, files=rows, started_monotonic_ns=started,
                    finished_monotonic_ns=time.monotonic_ns())
    except (OSError, ValueError) as error:
        return dict(unavailable=str(error), started_monotonic_ns=started,
                    finished_monotonic_ns=time.monotonic_ns())


def bind_mapped_files(observations):
    """Hash observed libraries after native exit, preserving the earlier stat binding."""
    results = {}
    for observation in observations:
        for row in observation.get('files', []):
            name = row['path']
            if name in results:
                continue
            try:
                require('stat' in row, 'mapping unavailable at observation')
                stat = Path(name).stat()
                require(row['stat'] == [stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns], 'mapped file changed')
                # Representation mappings are already bound by the input inventory.
                require(not Path(name).is_relative_to(STORE), 'protected mapping bound in input inventory')
                results[name] = identity(name)
            except (OSError, ValueError) as error:
                results[name] = dict(path=name, unavailable=str(error))
    return list(results.values())


def display_geometry(display):
    x = ctypes.CDLL('libX11.so.6')
    x.XOpenDisplay.argtypes, x.XOpenDisplay.restype = [ctypes.c_char_p], ctypes.c_void_p
    for name in ('XDisplayWidth', 'XDisplayHeight', 'XDefaultDepth'):
        getattr(x, name).argtypes = [ctypes.c_void_p, ctypes.c_int]
        getattr(x, name).restype = ctypes.c_int
    x.XCloseDisplay.argtypes = [ctypes.c_void_p]
    handle = x.XOpenDisplay(display.encode())
    require(bool(handle), 'owned display not ready')
    try:
        return [x.XDisplayWidth(handle, 0), x.XDisplayHeight(handle, 0), x.XDefaultDepth(handle, 0)]
    finally:
        x.XCloseDisplay(handle)


def capture_command(tool, display, destination):
    return [tool, '-display', display, '-window', 'root', '-silent', '-snaps', '1',
            '-limit', 'thread', '1', '-limit', 'memory', '64MiB', '-limit', 'map', '0',
            '-limit', 'disk', '0', '-depth', '8', 'PNG24:' + str(destination)]


def capture_limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (8 << 20, 8 << 20))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def begin_capture(protocol, output, display, launch, offset):
    prefix = output / f'capture-{offset:02}s'
    result = dict(offset_seconds=offset, due_monotonic_ns=launch['launch_monotonic_ns'] + offset * 10**9,
                  attempted_monotonic_ns=time.monotonic_ns(), attempted_unix_ns=time.time_ns(),
                  native_identity=launch['identity'], image=str(prefix.with_suffix('.png')),
                  visual_inspection='pending; image existence is not native visual acceptance')
    result['lateness_ns'] = result['attempted_monotonic_ns'] - result['due_monotonic_ns']
    if not alive(launch['identity']):
        result['failure'] = 'native exited before scheduled capture; no retry'
        return result, None
    env = os.environ.copy()
    env.update(MAGICK_TMPDIR=str(output), TMPDIR=str(output), MAGICK_DISK_LIMIT='0', MAGICK_THREAD_LIMIT='1')
    command = capture_command(protocol['captures']['tool']['path'], display, prefix.with_suffix('.png'))
    result['command'] = command
    process = None
    try:
        with prefix.with_suffix('.log').open('xb') as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=env,
                                       start_new_session=True, preexec_fn=capture_limits)
        result['capture_identity'] = process_identity(process.pid)
        result['mapped_files'] = mapped_files(result['capture_identity'])
        return result, process
    except (OSError, ValueError) as error:
        if process is not None:
            stop(process, result.get('capture_identity'))
        result['failure'] = str(error)
        return result, None


def finish_capture(row, process):
    if process is None:
        return True
    if process.poll() is None:
        if time.monotonic_ns() < row['attempted_monotonic_ns'] + 2 * 10**9:
            return False
        kill_owned(row['capture_identity'])
        row['failure'] = 'capture exceeded two-second deadline; partial output retained'
    row['exit_code'] = process.wait(timeout=3)
    row['finished_monotonic_ns'] = time.monotonic_ns()
    row['finished_unix_ns'] = time.time_ns()
    row['elapsed_ns'] = row['finished_monotonic_ns'] - row['attempted_monotonic_ns']
    if row['exit_code']:
        row.setdefault('failure', 'capture process failed; no retry')
    try:
        # Validate the PNG header without decoding or making another capture.
        with Path(row['image']).open('rb') as stream:
            header = stream.read(24)
        require(header[:8] == b'\x89PNG\r\n\x1a\n' and header[12:16] == b'IHDR', 'PNG header unavailable')
        row['dimensions'] = [int.from_bytes(header[16:20], 'big'), int.from_bytes(header[20:24], 'big')]
        require(row['dimensions'] == [1440, 900], 'capture geometry differs from declared display')
        require(0 < Path(row['image']).stat().st_size <= 8 << 20, 'capture byte cap exceeded')
    except (OSError, ValueError) as error:
        row.setdefault('failure', str(error))
    return True


class LaunchObserver:
    """Module-local subprocess facade; the frozen harness bytes are unchanged."""
    def __init__(self, connection, native):
        self.connection, self.native, self.launches = connection, native, 0

    def __getattr__(self, name):
        return getattr(subprocess, name)

    def Popen(self, command, **kwargs):
        require(command[0] == self.native and self.launches == 0, 'one native launch only')
        self.launches += 1
        before = time.monotonic_ns()
        process = subprocess.Popen(command, **kwargs)
        record = dict(identity=process_identity(process.pid), launch_monotonic_ns=before,
                      popen_return_monotonic_ns=time.monotonic_ns(), launch_unix_ns=time.time_ns(), command=command)
        self.connection.send(record)
        return process


def journey_worker(arguments, connection, log_path, native):
    os.setsid()
    with open(log_path, 'xb', buffering=0) as log:
        os.dup2(log.fileno(), 1)
        os.dup2(log.fileno(), 2)
        try:
            harness = module('viewer-composed-journey')
            harness.subprocess = LaunchObserver(connection, native)
            sys.argv = ['viewer-composed-journey.py', *arguments]
            code = harness.main()
        except BaseException:
            traceback.print_exc()
            code = 1
        finally:
            connection.close()
    raise SystemExit(code)


def journey_arguments(protocol, output, url, index):
    return ['--mode', 'native', '--output', str(output), '--url', url,
            '--native-bin', protocol['native']['path'], '--web-root', protocol['web_root'],
            '--codec-repo', protocol['owners']['codec']['path'],
            '--benchmark-repo', protocol['owners']['benchmark']['path'],
            '--server-cache-state', 'uncontrolled-first-observation' if index == 1 else 'warm-server',
            '--catalogue-contract', protocol['catalogue_contract']['path'],
            '--workload', protocol['workload']['path'], '--thresholds', protocol['thresholds']['path'],
            '--memory-diagnostics', '--native-diagnostics']


def one_run(protocol, group, index, display, url):
    output = group / f'native-{index:02}'
    captures = group / f'native-{index:02}-captures'
    captures.mkdir()
    arguments = journey_arguments(protocol, output, url, index)
    context = multiprocessing.get_context('fork')
    receiver, sender = context.Pipe(duplex=False)
    worker = context.Process(target=journey_worker, args=(arguments, sender,
        group / f'native-{index:02}-harness.log', protocol['native']['path']))
    worker.start()
    sender.close()
    start = time.monotonic_ns()
    launch, active, rows, maps, failures = None, [], [], [], []
    finished_at = None
    try:
        while True:
            now = time.monotonic_ns()
            if launch is None and receiver.poll():
                try:
                    launch = receiver.recv()
                    write(group / f'native-{index:02}-launch.json', launch)
                except EOFError:
                    pass
            if launch:
                if now >= launch['launch_monotonic_ns'] + 660 * 10**9 and alive(launch['identity']):
                    kill_owned(launch['identity'])
                    failures.append('native reached fixed 660-second outer deadline')
                for offset in OFFSETS[len(rows):]:
                    if now < launch['launch_monotonic_ns'] + offset * 10**9:
                        break
                    row, process = begin_capture(protocol, captures, display, launch, offset)
                    rows.append(row)
                    active.append((row, process))
                    maps.append(mapped_files(launch['identity']))
            active = [(r, p) for r, p in active if not finish_capture(r, p)]
            if not worker.is_alive() and finished_at is None:
                finished_at = now
            if finished_at is not None and (launch is None or len(rows) == 2) and not active:
                break
            # Setup/final export time never extends the native deadline.
            limit = (launch['launch_monotonic_ns'] + 780 * 10**9) if launch else start + 120 * 10**9
            if now > limit:
                failures.append('harness setup/export deadline; incomplete output retained')
                break
            time.sleep(0.02)
    finally:
        if launch:
            kill_owned(launch['identity'])
        for row, process in active:
            if process:
                kill_owned(row['capture_identity'])
                process.wait(timeout=3)
                row['failure'] = 'capture interrupted by wrapper cleanup'
        if worker.is_alive():
            # The worker owns its own session and its native child is separately owned.
            try:
                record = process_identity(worker.pid)
                kill_owned(record)
            except (OSError, ValueError):
                worker.kill()
        worker.join(timeout=3)
        receiver.close()
    if launch is None:
        failures.append('harness did not launch native; slot consumed, no replacement')
    for offset in OFFSETS[len(rows):]:
        rows.append(dict(offset_seconds=offset, failure='native launch unavailable; no capture or retry'))
    for row in rows:
        if row.get('image') and Path(row['image']).exists():
            row['file'] = identity(row['image'])
        if row.get('mapped_files'):
            row['library_files'] = bind_mapped_files([row['mapped_files']])
    result = dict(index=index, arguments=arguments, harness_exit=worker.exitcode, launch=launch,
                  failures=failures, captures=rows, mapped_files=maps,
                  library_files=bind_mapped_files(maps),
                  server_cache_state_limitation='same service persists; warm-server enum does not establish full warming after a failed prior slot; see actual service-before/service-after counters')
    write(group / f'native-{index:02}-wrapper.json', result)
    return result


def five_slots(run):
    rows = []
    for index in range(1, 6):
        try:
            rows.append(run(index))
        except Exception as error:
            rows.append(dict(index=index, failures=[type(error).__name__ + ': ' + str(error)],
                             launch=None, harness_exit=None, captures=[]))
    return rows


def summarise(output):
    result = {}
    for name in ('composed-trace.json', 'process-memory-diagnostics.json', 'app.json', 'process-memory.json'):
        try:
            result[name] = load(output / name)
        except (OSError, ValueError) as error:
            result[name] = {'unavailable': str(error)}
    trace, diagnostic, app = [result[n] for n in ('composed-trace.json', 'process-memory-diagnostics.json', 'app.json')]
    cycles = []
    samples = diagnostic.get('samples', [])
    for attachment in diagnostic.get('phase_attachments', []):
        if attachment['marker']['kind'] != 'cycle-revisited':
            continue
        row = dict(marker=attachment['marker'])
        for side in ('before_sample', 'after_sample'):
            index = attachment[side]['value']
            row[side] = dict(index=index, unavailable=attachment[side].get('unavailable_reason'))
            if index is not None:
                sample = samples[index]
                row[side].update(status=sample['status'], identity=sample['identity'],
                                 started_monotonic_ns=sample['read_started_monotonic_ns'],
                                 finished_monotonic_ns=sample['read_finished_monotonic_ns'])
        cycles.append(row)
    growth = None
    if len(cycles) == 10 and [c['marker']['phase_label'] for c in cycles] == [f'cycle-{i:02}' for i in range(1, 11)]:
        identities = {(c['marker']['pid'], c['marker']['start_time_ticks']) for c in cycles}
        values = []
        for cycle in cycles:
            sides = [cycle[s].get('status', {}).get('value') for s in ('before_sample', 'after_sample')]
            if all(s and s['VmRSS']['value'] is not None for s in sides):
                values.append(sides[1]['VmRSS']['value'])
        if len(identities) == 1 and len(values) == 10:
            growth = dict(after_bracket_current_rss_bytes=values, last_minus_first_bytes=values[-1] - values[0])
    try:
        allocations = [json.loads(line) for line in read(output / 'memory-phase-markers.allocations.jsonl', 256 << 10).splitlines()]
        require(len(allocations) <= 256, 'allocation row cap')
    except (OSError, ValueError) as error:
        allocations = {'unavailable': str(error)}
    deep = [dict(sample_index=i, identity=s['identity'], smaps_rollup=s['smaps_rollup'],
                 mapping_categories=s['mapping_categories']) for i, s in enumerate(samples)
            if s.get('smaps_rollup', {}).get('value') is not None]
    observations = trace.get('observations', {})
    memory = result['process-memory.json']
    sampled = [s['rss_bytes'] for s in memory.get('samples', [])]
    hwms = memory.get('observed_pid_high_water_bytes', {})
    fallback = dict(process_peak_rss_bytes=app.get('process_peak_rss_bytes'),
                    process_group_sampled_peak_rss_bytes=max(sampled) if sampled else None,
                    observed_process_high_water_sum_bytes=sum(hwms.values()) if hwms else None)
    gates = {}
    for field in ('process_peak_rss_bytes', 'process_group_sampled_peak_rss_bytes', 'observed_process_high_water_sum_bytes'):
        value = observations.get(field, {}).get('value')
        boundary = 'existing composed trace observation'
        if value is None:
            value = fallback[field]
            boundary = 'retained app final HWM or original process-memory sampler; no synthetic trace or benchmark input'
        gates[field] = dict(bytes=value, ceiling_bytes=CEILING,
                            boundary=boundary,
                            within_ceiling=value <= CEILING if value is not None else None)
    return dict(trace_completed=trace.get('completed'), failures=trace.get('failures', []),
                unavailable=[v['unavailable'] for v in result.values() if 'unavailable' in v],
                environment=trace.get('environment'), observations=observations, rss=gates,
                diagnostic_cycles_completed=app.get('diagnostic_cycles_completed'),
                cycle_brackets=cycles, current_growth=growth,
                current_growth_unavailable=None if growth else 'ten complete same-lifetime before/after current-RSS brackets unavailable',
                smaps_first_last=deep[:1] + deep[-1:] if len(deep) > 1 else deep,
                all_smaps_evidence=str(output / 'process-memory-diagnostics.json'),
                allocation_observations=allocations,
                phase_observations=diagnostic.get('application_phase_observations'),
                diagnostic_skipped=diagnostic.get('skipped'),
                recovery='not exercised by this native cohort; inherited seven-event pressure/recovery proof remains separately mandatory',
                boundary='RSS/HWM acceptance unchanged. Proc, smaps, allocator and logical resources overlap; never add or subtract. No cause, repair, quality or speed claim.')


def assess(protocol, group, index):
    output = group / f'native-{index:02}'
    report = summarise(output)
    try:
        report['coverage'] = module('viewer-native-diagnostics').audit(output)
    except (OSError, ValueError, KeyError) as error:
        report['coverage'] = dict(unavailable=str(error))
    trace = output / 'composed-trace.json'
    if trace.exists():
        command = [protocol['benchmark']['path'], 'journey', str(trace), protocol['thresholds']['path']]
        try:
            with (group / f'native-{index:02}-benchmark.json').open('xb') as out, \
                    (group / f'native-{index:02}-benchmark.log').open('xb') as err:
                result = subprocess.run(command, stdout=out, stderr=err, timeout=30, check=False)
            report['benchmark'] = dict(command=command, exit_code=result.returncode)
        except (OSError, subprocess.TimeoutExpired) as error:
            report['benchmark'] = dict(command=command, unavailable=str(error))
    else:
        report['benchmark'] = dict(unavailable='no trace; no synthetic benchmark input')
    write(group / f'native-{index:02}-summary.json', report)
    return report


def execute(args):
    protocol_path, protocol = select_protocol(args.protocol)
    grant = load(args.grant)
    validate_grant(grant, args.protocol_commit, args.output_name, args.display, args.port, protocol_path)
    bindings = committed(args.protocol_commit, protocol_path)
    require(args.output_name.startswith('viewer-acceptance-native-cohort-') and Path(args.output_name).name == args.output_name, 'fresh attributed cohort child required')
    require(args.display.startswith(':') and args.display[1:].isdigit(), 'explicit local display number required')
    require(1024 <= args.port <= 65535, 'unprivileged service port required')
    require(not (STORE / args.output_name).exists(), 'output exists; no resume or rerun')
    # Consume this immutable cohort only once, including infrastructure failure.
    for prior in STORE.glob('viewer-acceptance-native-cohort-*/execution.json'):
        require(load(prior)['cohort_id'] != protocol['cohort_id'], 'cohort already attempted; no reruns')
    environment = preflight(protocol)
    try:
        environment['loaded_nvidia_kernel_driver'] = read('/proc/driver/nvidia/version', 16384).decode()
    except OSError as error:
        environment['loaded_nvidia_kernel_driver'] = dict(unavailable=str(error))
    for key, value in protocol['runtime_environment'].items():
        require(os.environ.get(key) == value, 'execution environment drift: ' + key)
    require(not os.environ.get('LD_PRELOAD') and not os.environ.get('WAYLAND_DISPLAY'), 'unfrozen preload/Wayland environment')
    number = args.display[1:]
    require(not Path('/tmp/.X' + number + '-lock').exists() and not Path('/tmp/.X11-unix/X' + number).exists(), 'display already allocated')
    with socket.socket() as probe:
        probe.bind(('127.0.0.1', args.port))
    before = input_inventory(protocol)
    source = module('viewer-real-scene-source')
    group = source.fresh_group(STORE, args.output_name,
        'Five fixed native diagnostic starts and two bounded X11 screenshots per declared run; quality-rejected Mansfield PAN16 4/RGB16 12; original source arrays, MSI, representations and defaults unchanged.')
    write(group / 'execution.json', dict(cohort_id=protocol['cohort_id'], protocol=protocol,
        protocol_identity=identity(protocol_path),
        protocol_commit=args.protocol_commit, harness_files=bindings, grant=grant,
        grant_identity=identity(args.grant), environment=environment, display=args.display,
        input_inventory=before, runtime_revision=protocol['runtime_revision'],
        visual_inspection='pending actual native screenshots; coordinator records observations in approved group'))
    os.environ['DISPLAY'] = args.display
    # Only the execution path installs signal handling; authored checks never do.
    def interrupted(signum, _frame):
        raise KeyboardInterrupt('coordinator interrupted execution: ' + str(signum))
    signal.signal(signal.SIGTERM, interrupted)
    service = display = service_id = display_id = None
    outcomes, infrastructure = [], {}
    url = f'http://127.0.0.1:{args.port}'
    try:
        display_command = [protocol['xvfb'], args.display, '-screen', '0', '1440x900x24', '-nolisten', 'tcp']
        with (group / 'display.log').open('xb') as log:
            display = subprocess.Popen(display_command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        display_id = process_identity(display.pid)
        infrastructure.update(display_command=display_command, display_identity=display_id)
        service_command = [protocol['service']['path'], 'serve']
        for rep in protocol['representations']:
            service_command += ['--representation', rep['path']]
        service_command += ['--listen', f'127.0.0.1:{args.port}', '--verify-payload', 'true', '--web', protocol['web_root']]
        with (group / 'service.log').open('xb') as log:
            service = subprocess.Popen(service_command, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        service_id = process_identity(service.pid)
        infrastructure.update(service_command=service_command, service_identity=service_id)
        deadline = time.monotonic() + 60
        while True:
            require(display.poll() is None and service.poll() is None, 'owned infrastructure exited during setup')
            try:
                geometry = display_geometry(args.display)
                with urllib.request.urlopen(url + '/catalogue', timeout=1) as response:
                    catalogue = json.loads(response.read((4 << 20) + 1))
                require([dict(source_sha256=m['identity']['source_sha256'], bands=m['identity']['bands'],
                    width=m['identity']['profile']['width'], height=m['identity']['profile']['height'])
                    for m in catalogue] == load(protocol['catalogue_contract']['path'])['sources'], 'service catalogue mismatch')
                require(geometry == [1440, 900, 24], 'actual display geometry mismatch')
                infrastructure['display_geometry'] = geometry
                infrastructure['catalogue'] = catalogue
                break
            except (OSError, ValueError) as error:
                if time.monotonic() >= deadline:
                    raise ValueError('bounded infrastructure readiness failed') from error
                time.sleep(0.1)
        infrastructure['service_maps'] = mapped_files(service_id)
        outcomes = five_slots(lambda index: one_run(protocol, group, index, args.display, url))
    except Exception as error:
        infrastructure['failure'] = type(error).__name__ + ': ' + str(error)
        if not outcomes:
            outcomes = [dict(index=i, launch=None, captures=[], failures=['infrastructure unavailable; no replacement start']) for i in range(1, 6)]
    finally:
        stop(service, service_id)
        stop(display, display_id)
        infrastructure['cleanup'] = dict(service_exit=service.returncode if service else None,
            display_exit=display.returncode if display else None, scope='only owned process groups; logs/images/inputs retained')
        write(group / 'infrastructure.json', infrastructure)
        write(group / 'outcomes.json', outcomes)
    reports = five_slots(lambda index: assess(protocol, group, index))
    after = input_inventory(protocol)
    write(group / 'inputs-after.json', dict(unchanged=before == after, inventory=after))
    write(group / 'summary.json', dict(cohort_id=protocol['cohort_id'], outcomes=outcomes,
        summaries=[str(group / f'native-{i:02}-summary.json') for i in range(1, 6)],
        summary_failures=[r for r in reports if 'index' in r], inputs_unchanged=before == after,
        native_launches_observed=sum(bool(r.get('launch')) for r in outcomes),
        original_observation_bytes=278794240, unchanged_ceiling_bytes=CEILING,
        status='diagnostic evidence retained; coordinator must inspect actual native captures and reconcile all predicates',
        acceptance=False, repair_implemented=False))
    return 0 if before == after and all(r.get('harness_exit') == 0 and not r.get('failures')
        and len(r.get('captures', [])) == 2 and all('failure' not in c for c in r['captures']) for r in outcomes) \
        and all(r.get('benchmark', {}).get('exit_code') == 0 and 'unavailable' not in r.get('coverage', {'unavailable': True})
                and r.get('current_growth') is not None and all(g['within_ceiling'] is True for g in r.get('rss', {}).values())
                for r in reports) else 4


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    check = sub.add_parser('check', help='read-only identities/contract checks; never starts viewer, display or service')
    run = sub.add_parser('execute', help='later coordinator-granted execution only')
    for command in (check, run):
        command.add_argument('--protocol', type=Path, default=PROTOCOL,
                             help='explicit original or repaired repository protocol; defaults to original')
    run.add_argument('--grant', type=Path, required=True)
    run.add_argument('--protocol-commit', required=True)
    run.add_argument('--output-name', required=True)
    run.add_argument('--display', required=True)
    run.add_argument('--port', type=int, required=True)
    args = parser.parse_args()
    if args.command == 'check':
        protocol_path, protocol = select_protocol(args.protocol)
        preflight(protocol)
        print(json.dumps(dict(status='read-only checks passed; execution unperformed', protocol=identity(protocol_path))))
        return 0
    # A nonblocking local orchestration lock prevents racing group scans; the
    # durable approved execution record prevents later reruns of this cohort.
    lock_path = Path('/nvme/development/emuella/.build-targets/viewer-acceptance/native-cohort-execution.lock')
    with lock_path.open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        return execute(args)


if __name__ == '__main__':
    raise SystemExit(main())
