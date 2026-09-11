#!/usr/bin/env python3
"""Assess the frozen native presentation probe using the benchmark owner's method."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import statistics

ENDPOINTS = ('warm_compressed_ms', 'first_useful_ms', 'whole_overview_ms')


def interval(values):
    if len(values) not in (5, 20) or any(not math.isfinite(x) or x <= 0 for x in values):
        raise ValueError('requires five screening or twenty qualification observations')
    # emuella-benchmark f78c9c4, src/compare.rs: conservative 99.5% marginals.
    critical = 5.598 if len(values) == 5 else 3.287
    mean = statistics.mean(values)
    margin = critical * statistics.stdev(values) / math.sqrt(len(values))
    return {'mean': mean, 'low': mean-margin, 'high': mean+margin,
            'minimum': min(values), 'maximum': max(values), 'values': values}


def assess(manifest, screening=False):
    pairs = manifest['pairs']
    if len(pairs) != (5 if screening else 20):
        raise ValueError('wrong frozen pair count; no successful subset is allowed')
    arms = {'A': [], 'B': []}
    identity = environment = None
    previous_start = 0
    for number, pair in enumerate(pairs, 1):
        order = 'AB' if number % 2 else 'BA'
        if pair['round'] != number or pair['order'] != order:
            raise ValueError('pairs must use the frozen alternating AB/BA order')
        for arm in order:
            reference = pair[arm]
            path = Path(reference['path'])
            payload = path.read_bytes()
            if hashlib.sha256(payload).hexdigest() != reference['sha256']:
                raise ValueError('trace identity changed')
            trace = json.loads(payload)
            run = json.loads((path.parent/'run-identity.json').read_text())
            admission = json.loads((path.parent/'benchmark-admission.json').read_text())
            if not trace['completed'] or trace['failures'] or admission['exit'] != 0:
                raise ValueError('failed journey or owner admission invalidates the comparison')
            if trace['started_unix_ms'] <= previous_start:
                raise ValueError('fresh processes must follow recorded execution order')
            previous_start = trace['started_unix_ms']
            if run['arguments']['native_present_mode'] != ('None' if arm == 'A' else 'auto-no-vsync'):
                raise ValueError('arm must match the actually invoked consumer configuration')
            if identity is None:
                identity, environment = trace['identity'], trace['environment']
            if trace['identity'] != identity or trace['environment'] != environment:
                raise ValueError('matched source/build/input/workload/environment identities required')
            if not environment['hardware_gpu'] or not environment['runtime'].startswith('native '):
                raise ValueError('native hardware evidence required')
            if trace['observations']['worker_concurrency']['value'] != 1:
                raise ValueError('one-worker contract changed')
            arms[arm].append(trace)
    results = {}
    for endpoint in ENDPOINTS:
        a, b = (interval([t['observations'][endpoint]['value'] for t in arms[arm]]) for arm in 'AB')
        if min(a['low'], b['low']) <= 0:
            raise ValueError('nonpositive confidence interval is inconclusive')
        ratio = [b['low']/a['high']-1, b['high']/a['low']-1]
        results[endpoint] = {'baseline': a, 'candidate': b, 'relative_99_percent_interval': ratio,
                             'mean_relative_change': b['mean']/a['mean']-1}
    if screening:
        passed = results['warm_compressed_ms']['mean_relative_change'] < -.05
        passed &= all(results[k]['mean_relative_change'] <= .05 for k in ENDPOINTS[1:])
    else:
        passed = results['warm_compressed_ms']['relative_99_percent_interval'][1] < -.05
        passed &= all(results[k]['relative_99_percent_interval'][1] <= .05 for k in ENDPOINTS[1:])
    return {'passed': passed, 'screening_only': screening, 'endpoints': results,
            'method': 'benchmark-owner conservative99.5% marginal Student t intervals; Bonferroni99% ratio bounds per endpoint; no outlier removal; no family-wide claim',
            'identity': identity, 'environment': environment}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--screen', action='store_true')
    args = parser.parse_args()
    try:
        result = assess(json.loads(args.manifest.read_text()), args.screen)
    except (KeyError, ValueError, OSError, TypeError) as error:
        result = {'passed': False, 'invalid': str(error)}
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 4)


if __name__ == '__main__':
    main()
