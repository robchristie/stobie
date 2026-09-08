#!/usr/bin/env python3
"""Summarise five independent traces; emit proposals, never frozen thresholds."""
import argparse
import hashlib
import json
import math
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('traces', nargs='+', type=Path)
args = parser.parse_args()
if len(args.traces) < 5:
    parser.error('at least five independent runs are required')
traces = [json.loads(path.read_text()) for path in args.traces]
if any(not trace['completed'] or trace['failures'] or trace['thresholds_sha256'] for trace in traces):
    parser.error('only completed, failure-free calibration traces can propose thresholds')
if len({trace['started_unix_ms'] for trace in traces}) != len(traces):
    parser.error('duplicate run identity')
if len({trace['identity']['workload_sha256'] for trace in traces}) != 1:
    parser.error('mixed workload identities')
names = ['first_useful_ms', 'whole_overview_ms', 'target_detail_ms', 'visible_thumbnail_completion_ms', 'warm_revisit_ms', 'warm_compressed_ms', 'process_peak_rss_bytes', 'wasm_linear_bytes', 'process_group_sampled_peak_rss_bytes', 'observed_process_high_water_sum_bytes']
summary = {}
for name in names:
    observations = [trace['observations'].get(name, {'value': None}) for trace in traces]
    if any(observation['value'] is None for observation in observations):
        continue
    if len({(o['unit'], o['boundary']) for o in observations}) != 1:
        parser.error('incomparable measurement ' + name)
    values = sorted(o['value'] for o in observations)
    p95 = values[math.ceil(0.95 * len(values)) - 1]
    quantum = 10 if observations[0]['unit'] == 'ms' else 1 << 20
    summary[name] = {'values': values, 'nearest_rank_p95': p95, 'proposed_maximum': math.ceil(p95 * 1.25 / quantum) * quantum, 'unit': observations[0]['unit'], 'boundary': observations[0]['boundary']}
print(json.dumps({'status': 'proposal-only; coordinator must select and freeze before final runs', 'method': 'nearest-rank p95 across independent fresh clients; predeclared 25% allowance, rounded upwards to 10 ms or 1 MiB', 'workload_sha256': traces[0]['identity']['workload_sha256'], 'baseline_evidence_sha256': {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in args.traces}, 'measurements': summary}, indent=2))
