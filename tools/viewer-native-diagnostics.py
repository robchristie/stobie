#!/usr/bin/env python3
"""Authored native fixture and bounded diagnostic coverage audit; no timing runner."""
import argparse
import hashlib
import importlib.util
import json
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
AUTHORED = 'authored-completion-diagnostic-v1'


def compact(value):
    return json.dumps(value, separators=(',', ':'), ensure_ascii=False).encode()


def catalogue():
    result = []
    for components, precision in [(1, 16), (3, 16), (3, 8)]:
        profile = dict(width=43008, height=43008, tile_edge=512,
                       decomposition_levels=6, bits_per_sample=precision,
                       components=components, bits_per_pixel=4.0)
        identity = dict(source_sha256=AUTHORED, bands=list(range(components)), profile=profile,
                        codec_revision='authored-no-codec', encoding_contract=AUTHORED,
                        spatial_policy_sha256='authored', payload_sha256='authored-no-payload',
                        descriptor_format='authored-no-descriptors')
        descriptors = ['authored'] * (84 * 84)
        tid = hashlib.sha256(compact([identity, 1024, 128, descriptors])).hexdigest()
        result.append(dict(target=f'authored-{components}band-{precision}bit', tid=tid,
                           identity=identity, encoded_bytes=1024, main_header_bytes=128,
                           descriptor_sha256=descriptors))
    return result


def bounded_json(path, limit=64 << 20):
    with path.open('rb') as source:
        data = source.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f'{path.name}: diagnostic input byte bound exceeded')
    return json.loads(data)


def audit(run):
    spec = importlib.util.spec_from_file_location('memory_diagnostics', ROOT / 'tools/viewer-memory-diagnostics.py')
    memory = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(memory)
    parsed = memory.read_markers(run / 'memory-phase-markers.jsonl')
    if parsed['value'] is None:
        raise ValueError(parsed['unavailable_reason'])
    markers = parsed['value']
    identities = {(m['pid'], m['start_time_ticks']) for m in markers}
    if len(identities) != 1 or [m['sequence'] for m in markers] != list(range(len(markers))):
        raise ValueError('one native lifetime and contiguous marker sequence required')
    if [(m['kind'], m['phase_label']) for m in markers[:2]] != [('startup', 'startup'), ('graphics-ready', 'graphics')]:
        raise ValueError('startup/graphics coverage unavailable')
    stages = bounded_json(run / 'app.json.stages.json')
    final = bounded_json(run / 'app.json')
    if not final.get('script_complete') or final.get('errors') or final.get('diagnostic_cycles_completed') != 10:
        raise ValueError('application failed or ten cycles incomplete')
    expected_phases = [s['phase_label'] for s in stages]
    for kind in ('phase-start', 'phase-settled'):
        if [m['phase_label'] for m in markers if m['kind'] == kind] != expected_phases:
            raise ValueError(f'{kind}: actual phase coverage mismatch')
    expected = [(kind, f'cycle-{i:02}') for i in range(1, 11) for kind in ('cycle-evicted', 'cycle-revisited')]
    if [(m['kind'], m['phase_label']) for m in markers if m['kind'].startswith('cycle-')] != expected:
        raise ValueError('actual cycle marker coverage mismatch')
    allocation_path = run / 'memory-phase-markers.allocations.jsonl'
    with allocation_path.open('rb') as source:
        raw = source.read((256 << 10) + 1)
    if len(raw) > 256 << 10 or len(raw.splitlines()) > 256:
        raise ValueError('allocation observation bound exceeded')
    allocations = [json.loads(line) for line in raw.splitlines()]
    if len(allocations) != len(markers) or any(a.get('marker') != m for a, m in zip(allocations, markers)):
        raise ValueError('allocation/marker coverage mismatch')
    for row in allocations:
        if row.get('schema') != 'viewer_native_allocation_marker/1' or row.get('observation_finished_monotonic_ns', -1) < row['marker']['monotonic_ns']:
            raise ValueError('allocation schema or observation clock invalid')
    return dict(schema='viewer_native_diagnostic_coverage/1', instrument=final.get('instrument'),
                phase_count=len(stages), cycle_count=10, marker_count=len(markers),
                trace_ring_dropped=final.get('diagnostic_trace_dropped'),
                allocator_unavailable=sum('unavailable' in a.get('allocator', {}) for a in allocations),
                boundary='coverage only; ring loss remains explicit; no RSS, recovery, quality or speed acceptance')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    author = commands.add_parser('author')
    author.add_argument('--output', type=Path, required=True)
    serve = commands.add_parser('serve')
    serve.add_argument('--catalogue', type=Path, required=True)
    serve.add_argument('--port', type=int, default=8195)
    check = commands.add_parser('audit')
    check.add_argument('--run', type=Path, required=True)
    args = parser.parse_args()
    if args.command == 'author':
        args.output.mkdir(parents=True, exist_ok=False)
        values = catalogue()
        files = {'catalogue.json': compact(values),
                 'workload.json': (ROOT / 'apps/emuella-viewer/authored-completion-workload.json').read_bytes(),
                 'catalogue-contract.json': compact(dict(sources=[dict(source_sha256=m['identity']['source_sha256'],
                     bands=m['identity']['bands'], width=m['identity']['profile']['width'],
                     height=m['identity']['profile']['height']) for m in values]))}
        for name, data in files.items():
            (args.output / name).write_bytes(data)
        (args.output / 'instrument.json').write_bytes(compact(dict(instrument=AUTHORED,
            files={name: hashlib.sha256(data).hexdigest() for name, data in files.items()},
            boundary='authored metadata only; constant results are neither reconstruction nor quality/production performance evidence')))
    elif args.command == 'serve':
        values = bounded_json(args.catalogue, 16 << 20)
        if values != catalogue():
            raise ValueError('serve accepts only the exact authored catalogue')
        payload = compact(values)

        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                if self.path not in ('/catalogue', '/metrics'):
                    self.send_error(404, 'authored fixture has no source payload')
                    return
                body = payload if self.path == '/catalogue' else b'{}'
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        HTTPServer(('127.0.0.1', args.port), Handler).serve_forever()
    else:
        print(json.dumps(audit(args.run), indent=2))


if __name__ == '__main__':
    main()
