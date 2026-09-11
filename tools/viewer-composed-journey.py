#!/usr/bin/env python3
"""Run and export one immutable composed-journey observation; never freeze limits."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import socket
import signal
import socketserver
import select
import threading
from urllib.parse import urlsplit
import time
import urllib.request


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(path.read_text())


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2) + '\n')


def get(url):
    with urllib.request.urlopen(url, timeout=30) as response:
        return json.load(response)


def recovery_counter(states, field, worker=True):
    """Sum sampled cumulative maxima without crossing fresh-context resets."""
    maxima = {}
    for state in states:
        context = state.get('context_id')
        if not context or not state.get('page_id'):
            return None
        snapshot = state['snapshot']
        value = (snapshot.get('worker', {}) if worker else snapshot).get(field)
        maxima.setdefault(context, None)
        if value is not None:
            maxima[context] = max(maxima[context] or 0, value)
    return sum(maxima.values()) if maxima and all(v is not None for v in maxima.values()) else None


def worker_observation(field, snapshots, recovery=None):
    boundary = 'shared worker ' + field
    if recovery is not None:
        if not field.startswith('peak_') and field != 'wasm_linear_bytes':
            return recovery_counter(recovery.get('states', []), field), (
                'sum of per-context sampled cumulative maxima across sequential fresh browser contexts; ' + boundary)
        boundary = 'maximum across sampled sequential fresh browser contexts; ' + boundary
    values = [s['worker'][field] for s in snapshots if s.get('worker', {}).get(field) is not None]
    return max(values) if values else None, boundary


def browser_attribution(mode, browser, recovery):
    if mode == 'recovery':
        return (mode + ' ' + recovery.get('browser_version', 'browser version unavailable'),
                recovery.get('workers'),
                'total Playwright Worker creation events across sequential fresh browser contexts; not concurrent workers',
                'recovery browser did not report Worker creation events')
    return (mode + ' ' + browser.get('browser_version', 'native eframe/wgpu'),
            browser.get('workers'), 'Playwright Worker creation events',
            'native executor; browser measurement not applicable')


class WireProxy(socketserver.ThreadingTCPServer):
    """Count actual TCP application bytes; HTTP headers and retries remain included."""
    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, url):
        target = urlsplit(url)
        if target.scheme != 'http' or target.hostname not in ('localhost', '127.0.0.1'):
            raise ValueError('calibration proxy requires a loopback HTTP service')
        self.target = (target.hostname, target.port or 80)
        self.received = 0
        self.sent = 0
        self.connections = 0
        self.lock = threading.Lock()
        super().__init__(('127.0.0.1', 0), WireHandler)


class WireHandler(socketserver.BaseRequestHandler):
    def handle(self):
        server = self.server
        with server.lock:
            server.connections += 1
        with socket.create_connection(server.target, timeout=30) as upstream:
            peers = {self.request: upstream, upstream: self.request}
            try:
                while True:
                    readable, _, _ = select.select(list(peers), [], [], 30)
                    if not readable:
                        return
                    for source in readable:
                        data = source.recv(65536)
                        if not data:
                            return
                        peers[source].sendall(data)
                        with server.lock:
                            if source is upstream:
                                server.received += len(data)
                            else:
                                server.sent += len(data)
            except (ConnectionError, TimeoutError, OSError):
                return


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=['native', 'browser', 'recovery'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--url', required=True)
    parser.add_argument('--native-bin', type=Path, required=True)
    parser.add_argument('--web-root', type=Path, required=True)
    parser.add_argument('--codec-repo', type=Path, required=True)
    parser.add_argument('--benchmark-repo', type=Path, required=True)
    parser.add_argument('--server-cache-state', choices=['uncontrolled-first-observation', 'warm-server'], required=True)
    parser.add_argument('--thresholds', type=Path)
    parser.add_argument('--recovery-pressure', choices=['image-gallery', 'real-scene-pan-sweep'], default='image-gallery')
    parser.add_argument('--workload', type=Path, help='explicit actions; preserves default workload when omitted')
    parser.add_argument('--catalogue-contract', type=Path, help='exact ordered source hashes, bands and full geometry for real scenes')
    args = parser.parse_args()
    if args.mode != 'recovery' and args.recovery_pressure != 'image-gallery':
        parser.error('--recovery-pressure requires --mode recovery')
    # Exclusive creation preserves every failed probe and prevents accidental replacement.
    args.output.mkdir(parents=True, exist_ok=False)
    root = Path(__file__).resolve().parents[1]
    started = time.time_ns() // 1_000_000
    for relative in ['apps/emuella-viewer/qualification-workload.json', 'tools/viewer-composed-journey.py', 'tools/viewer-composed-browser.mjs', 'tools/viewer-browser-recovery.mjs', 'tools/viewer-recovery-workload.mjs']:
        (args.output / Path(relative).name).write_bytes((root / relative).read_bytes())
    workload_path = args.workload or root / 'apps/emuella-viewer/qualification-workload.json'
    (args.output / 'qualification-workload.json').write_bytes(workload_path.read_bytes())
    catalogue = get(args.url + '/catalogue')
    contract = read_json(args.catalogue_contract) if args.catalogue_contract else None
    if contract is not None:
        write_json(args.output / 'catalogue-contract.json', contract)
        actual = [{'source_sha256': m['identity']['source_sha256'], 'bands': m['identity']['bands'], 'width': m['identity']['profile']['width'], 'height': m['identity']['profile']['height']} for m in catalogue]
        if actual != contract['sources']:
            raise ValueError('catalogue does not match frozen full-scene source contract')
    before = get(args.url + '/metrics')
    write_json(args.output / 'catalogue.json', catalogue)
    write_json(args.output / 'service-before.json', before)
    revisions = {}
    for name, repo in [('polyorama', root), ('codec', args.codec_repo), ('benchmark', args.benchmark_repo)]:
        revisions[name] = subprocess.check_output(['git', '-C', str(repo), 'rev-parse', 'HEAD'], text=True).strip()
        (args.output / f'{name}.diff').write_bytes(subprocess.check_output(['git', '-C', str(repo), 'diff', 'HEAD']))
    builds = {'native': digest(args.native_bin.read_bytes())}
    for path in sorted(args.web_root.rglob('*')):
        if path.is_file():
            builds['web/' + str(path.relative_to(args.web_root))] = digest(path.read_bytes())
    write_json(args.output / 'run-identity.json', {'revisions': revisions, 'builds': builds, 'started_unix_ms': started, 'arguments': {k: str(v) for k, v in vars(args).items()}})
    failures = []
    proxy = WireProxy(args.url)
    threading.Thread(target=proxy.serve_forever, daemon=True).start()
    app_url = 'http://127.0.0.1:' + str(proxy.server_address[1])
    command = ([str(args.native_bin), '--server', app_url, '--script-output', str(args.output / 'app.json')]
               if args.mode == 'native' else ['node', str(root / ('tools/viewer-browser-recovery.mjs' if args.mode == 'recovery' else 'tools/viewer-composed-browser.mjs')), app_url, str(args.output)])
    if args.mode == 'recovery':
        command += ['--pressure-workload', args.recovery_pressure]
    if args.workload and args.mode != 'recovery':
        command += (['--workload', str(args.workload)] if args.mode == 'native' else [str(args.workload)])
    memory_samples = []
    observed_pid_high_water = {}
    with (args.output / 'process.log').open('w') as log:
        process = subprocess.Popen(command, cwd=root, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        deadline = time.monotonic() + 660
        while process.poll() is None and time.monotonic() < deadline:
            if platform.system() == 'Linux':
                processes = {}
                for path in Path('/proc').glob('[0-9]*/status'):
                    try:
                        status = dict(line.split(':', 1) for line in path.read_text().splitlines() if ':' in line)
                        processes[int(path.parent.name)] = (int(status['PPid']), int(status.get('VmRSS', '0').split()[0]) * 1024, int(status.get('VmHWM', '0').split()[0]) * 1024)
                    except (OSError, ValueError, KeyError, ProcessLookupError):
                        continue
                descendants = {process.pid}
                while True:
                    children = {pid for pid, (parent, _, _) in processes.items() if parent in descendants}
                    if children <= descendants:
                        break
                    descendants |= children
                # Exclude the Node harness: include Chromium and all its child processes.
                if args.mode != 'native':
                    descendants.discard(process.pid)
                for pid in descendants:
                    if pid in processes:
                        observed_pid_high_water[pid] = max(observed_pid_high_water.get(pid, 0), processes[pid][2])
                memory_samples.append({'at_ms': time.time_ns() // 1_000_000 - started, 'rss_bytes': sum(processes[pid][1] for pid in descendants if pid in processes), 'processes': sorted(descendants)})
            time.sleep(0.05)
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGKILL)
            failures.append('application exceeded predeclared 660 second harness deadline')
        result_code = process.wait()
        if result_code:
            failures.append(f'application exit {result_code}')
    write_json(args.output / 'process-memory.json', {'sampling_interval_ms': 50, 'boundary': 'sum of current Linux VmRSS across application descendant processes; excludes Node harness; shared mappings may be counted multiple times; sampled peak is not continuous high water', 'samples': memory_samples, 'observed_pid_high_water_bytes': observed_pid_high_water, 'exited_process_treatment': 'retain maximum observed VmHWM per PID after exit; processes that start and exit between samples may be absent; PID reuse is not expected in this bounded run'})
    proxy.shutdown()
    proxy.server_close()
    wire = dict(received_bytes=proxy.received, sent_bytes=proxy.sent, connections=proxy.connections)
    write_json(args.output / 'wire.json', wire)
    after = get(args.url + '/metrics')
    write_json(args.output / 'service-after.json', after)
    final = read_json(args.output / 'app.json') if (args.output / 'app.json').exists() else {}
    stages = read_json(args.output / 'app.json.stages.json') if (args.output / 'app.json.stages.json').exists() else []
    browser = read_json(args.output / 'browser.json') if (args.output / 'browser.json').exists() else {}
    recovery = read_json(args.output / 'browser-recovery.json') if (args.output / 'browser-recovery.json').exists() else {}
    if args.mode == 'recovery':
        final = recovery.get('states', [{}])[-1].get('snapshot', {})
        if not recovery.get('completed'):
            failures.append(recovery.get('error', 'recovery journey incomplete'))
    failures += final.get('errors', [])
    observations = {}
    def observe(name, value, unit, boundary, reason='not exposed by this instrumented boundary'):
        observations[name] = {'value': value, 'unavailable_reason': reason if value is None else None, 'unit': unit, 'boundary': boundary}
    snapshots = stages + ([final] if final else []) + browser.get('samples', []) + [state['snapshot'] for state in recovery.get('states', [])]
    observe('observed_process_high_water_sum_bytes', sum(observed_pid_high_water.values()) if observed_pid_high_water else None, 'bytes', 'sum of maximum observed Linux VmHWM per application PID; Node excluded; includes nonsimultaneous peaks/shared mappings; unobserved short-lived processes excluded')
    observe('process_group_sampled_peak_rss_bytes', max([s['rss_bytes'] for s in memory_samples], default=None), 'bytes', '50 ms sampled sum of Linux application descendant VmRSS; Node excluded; shared mappings may count repeatedly')
    for field in ['decoded_peak_bytes', 'gpu_peak_bytes', 'process_peak_rss_bytes']:
        values = [s[field] for s in snapshots if s.get(field) is not None]
        observe(field, max(values) if values else None, 'bytes', 'application ' + field)
    for field in ['peak_compressed_bytes', 'peak_descriptor_bytes', 'peak_codec_workspace_bytes', 'wasm_linear_bytes', 'selected_code_blocks', 'selected_block_coefficients', 'decoded_pixels', 'compressed_read_bytes', 'received_jpp_bytes', 'received_descriptor_bytes', 'requests', 'retries', 'cache_hits', 'representation_evictions', 'compressed_bin_evictions', 'synthesis_coefficients_loaded', 'synthesis_horizontal_values', 'synthesis_vertical_values', 'synthesis_lifting_updates', 'synthesis_output_samples']:
        value, boundary = worker_observation(field, snapshots, recovery if args.mode == 'recovery' else None)
        unit = 'bytes' if 'bytes' in field else 'count'
        observe(field, value, unit, boundary)
    synthesis_boundary = 'actual indexed codec synthesis output samples across successful decodes'
    if args.mode == 'recovery':
        synthesis_boundary += '; sum of per-context sampled cumulative maxima across sequential fresh browser contexts'
    observe('synthesis_work', observations['synthesis_output_samples']['value'], 'samples', synthesis_boundary, 'this provisional build predates the additive actual synthesis report API')
    observe('worker_concurrency', max([s.get('peak_in_flight', 0) for s in snapshots], default=0), 'count', 'application outstanding worker reservations')
    runtime, workers, workers_boundary, workers_unavailable = browser_attribution(args.mode, browser, recovery)
    observe('actual_browser_workers', workers, 'count', workers_boundary, workers_unavailable)
    for field in ['logical_read_bytes', 'logical_read_operations', 'descriptor_read_bytes', 'descriptor_read_operations', 'jpp_bytes', 'descriptor_bytes', 'process_read_bytes']:
        observe('service_' + field, after[field] - before[field] if before.get(field) is not None and after.get(field) is not None else None, 'bytes' if 'bytes' in field else 'count', 'service process counters during complete journey: ' + field)
    observe('client_tcp_received_bytes', wire['received_bytes'], 'bytes', 'actual application TCP receive including HTTP headers, catalogue, static assets and retries; excludes TCP/IP framing')
    observe('client_tcp_sent_bytes', wire['sent_bytes'], 'bytes', 'actual application TCP send including request headers and retries; excludes TCP/IP framing')
    observe('client_http_received_bytes', sum(t['encoded_data_length'] for t in browser.get('transfers', [])) if browser else None, 'bytes', 'CDP Network.loadingFinished encodedDataLength across page network targets', 'recovery harness does not collect CDP Network.loadingFinished; body and outer TCP counters are separate observations' if args.mode == 'recovery' else 'native transport exposes descriptor/JPP bodies; complete HTTP wire byte counter unavailable')
    observe('original_storage_read_bytes', 0, 'bytes', 'prepared representation service never opens original source during viewing')
    observe('original_preparation_read_bytes', None, 'bytes', 'original instrumented storage handle during preparation', 'separate preparation evidence; never infer physical reads from returned pixels')
    by_label = {s['phase_label']: s for s in stages}
    def latency(name, labels, field, boundary):
        values = [by_label[label].get(field) for label in labels if label in by_label]
        observe(name, max(values) if values and all(v is not None for v in values) and len(values) == len(labels) else None, 'ms', boundary)
    latency('first_useful_ms', ['empty-client-overview'], 'phase_first_useful_ms', 'application startup to first primary-view region GPU resident')
    latency('whole_overview_ms', ['empty-client-overview'], 'phase_settled_ms', 'application startup to all overview and visible gallery demands GPU resident')
    latency('target_detail_ms', ['clustered-detail', 'scattered-detail', 'scattered-detail-far'], 'phase_settled_ms', 'worst detection-open intent to all current demands GPU resident')
    latency('visible_thumbnail_completion_ms', ['clustered-gallery', 'scattered-gallery'], 'phase_settled_ms', 'worst gallery-scroll intent to all current demands GPU resident')
    latency('warm_revisit_ms', ['warm-gpu-revisit'] + [f'recall-{i}' for i in range(5)], 'phase_settled_ms', 'worst GPU fit or bookmark recall intent to all current demands GPU resident')
    latency('warm_compressed_ms', ['warm-compressed'], 'phase_settled_ms', 'clear settled decoded/GPU state to all current demands GPU resident with shared compressed cache retained')
    if args.mode != 'recovery':
        expected = ['empty-client-overview'] + [step['label'] for step in read_json(workload_path)]
        if [s['phase_label'] for s in stages] != expected:
            failures.append('missing, reordered or repeated workload phases')
        if stages and ((not contract and stages[0].get('primary_desired') != 121) or stages[0].get('primary_desired', 0) < 1 or stages[0].get('primary_ready') != stages[0].get('primary_desired')):
            failures.append('all primary overview chunks must be resident (121 for the inherited workload)')
        if final.get('bookmarks', 0) < 5 or final.get('comparison_image') is None or final.get('comparison_image') == final.get('image'):
            failures.append('five bookmarks and simultaneous distinct images required')
    if not contract and (len(catalogue) < 9 or catalogue[0]['identity']['profile']['width'] < 43008):
        failures.append('representative workload requires one >=43008-wide image and eight additional images')
    if any(s.get('logical_detections') != 10000 or s.get('materialised_detections', 0) > 64 for s in stages):
        failures.append('logical/virtualised detection invariant failed')
    for name, limit in [('peak_compressed_bytes', 64 << 20), ('peak_descriptor_bytes', 16 << 20), ('peak_codec_workspace_bytes', 64 << 20), ('decoded_peak_bytes', 16 << 20), ('gpu_peak_bytes', 64 << 20), ('worker_concurrency', 1)]:
        if observations[name]['value'] is None or observations[name]['value'] > limit:
            failures.append('resource bound failed: ' + name)
    if all(label in by_label for label in ['empty-client-overview', 'warm-compressed', 'warm-gpu-revisit']):
        cold, compressed, gpu = [by_label[label] for label in ['empty-client-overview', 'warm-compressed', 'warm-gpu-revisit']]
        if cold['worker']['requests'] != compressed['worker']['requests']:
            failures.append('warm compressed reconstruction requested additional JPP data')
        if compressed['worker']['decode_count'] != gpu['worker']['decode_count'] or compressed['gpu_uploads'] != gpu['gpu_uploads']:
            failures.append('warm GPU revisit decoded or uploaded again')
    events = list(recovery.get('events', []))
    for s in stages:
        label = s['phase_label']
        start = s['phase_started_ms']
        observe(label + '_latency_ms', s.get('phase_settled_ms'), 'ms', 'app phase intent to all distinct desired regions GPU resident: ' + label)
        observe(label + '_first_useful_ms', s.get('phase_first_useful_ms'), 'ms', 'app phase intent to first desired region GPU resident: ' + label)
        events.append(dict(at_ms=start, kind='demand', consumer=label, source=catalogue[s['image']]['target'], generation=s['generation'], detail='phase intent; see immutable workload and stage snapshot'))
        if s.get('phase_settled_ms') is not None:
            events.append(dict(at_ms=start + s['phase_settled_ms'], kind='presentation_ready', consumer=label, source=catalogue[s['image']]['target'], generation=s['generation'], detail=f'all {s["desired"]} distinct demands GPU resident; GPU execution timestamp unavailable'))
        if label == 'warm-compressed':
            events.append(dict(at_ms=start,kind='cache_initialisation',consumer=label,source=catalogue[s['image']]['target'],generation=s['generation'],detail='settled runtime and app GPU renderer replaced; compressed executor retained; source storage uncontrolled'))
        if label == 'warm-gpu-revisit':
            events.append(dict(at_ms=start,kind='cache_initialisation',consumer=label,source=catalogue[s['image']]['target'],generation=s['generation'],detail='same-camera fit with current GPU textures and compressed state retained'))
    seen = set()
    for s in ([] if args.mode == 'recovery' else snapshots):
        for e in s.get('events', []):
            key = json.dumps(e, sort_keys=True)
            if key not in seen:
                seen.add(key)
                events.append(dict(at_ms=e['at_ms'],kind=e['kind'],consumer='regional-runtime',source=next(m['target'] for m in catalogue if m['tid'] == bytes(e['key']['representation']).hex()),generation=e['token']['demand_epoch'],detail=key))
    observe('retained_work_events', len(seen), 'count', 'deduplicated bounded app snapshots captured at phases and browser sampling')
    dropped = recovery_counter(recovery.get('states', []), 'events_dropped', worker=False) if args.mode == 'recovery' else max([s.get('events_dropped', 0) for s in snapshots], default=0)
    dropped_boundary = 'app 128-event ring cumulative overwritten records; sampled export is not guaranteed complete'
    if args.mode == 'recovery':
        dropped_boundary += '; sum of per-context sampled cumulative maxima across sequential fresh browser contexts'
    observe('app_event_history_dropped', dropped, 'count', dropped_boundary)
    gpu = json.dumps(recovery.get('adapters')) if recovery else json.dumps(browser.get('adapters')) if browser else final.get('gpu_adapter', 'unavailable')
    hardware_gpu = ('DiscreteGpu' in gpu or 'IntegratedGpu' in gpu or 'nvidia' in gpu.lower() or 'intel' in gpu.lower() or 'amd' in gpu.lower()) and not any(x in gpu.lower() for x in ['swiftshader', 'llvmpipe', 'software'])
    trace = dict(schema='composed_journey_trace/1', started_unix_ms=started, completed=bool(recovery.get('completed')) if args.mode == 'recovery' else bool(final.get('script_complete')) and len(stages) == 1 + len(read_json(workload_path)), failures=failures,
        identity=dict(revisions=revisions,builds=builds,inputs={m['target']:m['tid'] for m in catalogue},workload_sha256=(digest((root / 'tools/viewer-browser-recovery.mjs').read_bytes() + (root / 'tools/viewer-recovery-workload.mjs').read_bytes() + args.recovery_pressure.encode()) if args.mode == 'recovery' else digest(workload_path.read_bytes()))),
        environment=dict(hardware=platform.machine() + ' ' + platform.processor() + '; ' + os.uname().nodename,operating_system=platform.platform(),runtime=runtime,gpu=gpu,hardware_gpu=hardware_gpu),
        cache_state=dict(source_storage='uncontrolled; no OS or NFS cache conditioning',server=args.server_cache_state,client_compressed='empty fresh executor',client_decoded='empty fresh runtime',gpu='empty fresh renderer',initialisation='three sequential fresh browser contexts: transport retry/reconnect, stale completion, cancellation/cache pressure; each starts with empty client caches' if args.mode == 'recovery' else 'fresh process/context; warm compressed and warm GPU phases explicitly recorded; warm-server baseline is a second fresh client after the prior complete run'),
        evidence_sha256={p.name:digest(p.read_bytes()) for p in sorted(args.output.iterdir()) if p.is_file()}, observations=observations,events=sorted(events,key=lambda e:e['at_ms']),thresholds_sha256=digest(args.thresholds.read_bytes()) if args.thresholds else None)
    write_json(args.output / 'composed-trace.json', trace)
    print(json.dumps({'output':str(args.output),'completed':trace['completed'],'failures':failures,'hardware_gpu':hardware_gpu,'stages':len(stages)}))
    return 0 if trace['completed'] and not failures else 4


if __name__ == '__main__':
    raise SystemExit(main())
