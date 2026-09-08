#!/usr/bin/env python3
"""Prepare the public viewing workload once and check its frozen large-image bounds."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
import tomllib


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tool', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    limits_path = root / 'apps/emuella-viewer/qualification/preparation-thresholds.json'
    limits = json.loads(limits_path.read_text())
    pin = tomllib.loads((root / 'crates/emuella-viewer-source/Cargo.toml').read_text())['dependencies']['emuella-j2k-codestream']['rev']
    catalogue = json.loads((root / 'docs/emuella-viewer-evidence/synthetic-input-catalogue.json').read_text())
    args.output.mkdir(parents=True, exist_ok=False)
    report = dict(schema='viewer-preparation-result/1', started_unix_ms=int(time.time()*1000),
                  thresholds_sha256=digest(limits_path), codec_revision=pin,
                  tool_sha256=digest(args.tool), representations=[], completed=False, failures=[])
    try:
        if report['started_unix_ms'] <= limits['frozen_unix_ms']:
            raise ValueError('preparation must follow the threshold freeze')
        for source in catalogue:
            target, profile = source['target'], source['identity']['profile']
            output = args.output / target
            command = [str(args.tool.resolve()), 'fixture', '--output', str(output.resolve()),
                       '--target', target, '--codec-revision', pin]
            for option, field in [('width','width'), ('height','height'), ('tile','tile_edge'),
                                  ('levels','decomposition_levels'), ('bits','bits_per_sample'),
                                  ('components','components'), ('bpp','bits_per_pixel')]:
                command += ['--'+option, str(profile[field])]
            start = time.monotonic()
            result_path = args.output / (target+'.json')
            with result_path.open('wb') as stdout, (args.output/(target+'.stderr')).open('wb') as stderr:
                subprocess.run(command, stdout=stdout, stderr=stderr, check=True, timeout=600)
            manifest, metrics = json.loads(result_path.read_text())
            if manifest['identity']['profile'] != profile or manifest['identity']['codec_revision'] != pin:
                raise ValueError('prepared profile or codec identity mismatch')
            report['representations'].append(dict(target=target, tid=manifest['tid'],
                result_sha256=digest(result_path), wall_ms=(time.monotonic()-start)*1000,
                encoded_bytes=manifest['encoded_bytes'], metrics=metrics))
            if target == 'large-scene':
                expected = limits['workload']
                for field in ['width', 'height', 'components', 'bits_per_sample', 'decomposition_levels']:
                    if profile[field] != expected[field]:
                        raise ValueError('large preparation workload differs from freeze')
                if profile['tile_edge'] != expected['tile_width'] or profile['bits_per_pixel'] != expected['target_bpp']:
                    raise ValueError('large preparation encoding policy differs from freeze')
                bounds = limits['bounds']
                observed = dict(metrics, encoded_bytes=manifest['encoded_bytes'],
                                descriptor_to_encoded_ratio=metrics['descriptor_bytes']/manifest['encoded_bytes'])
                for name, bound in bounds.items():
                    field, comparison = name.rsplit('_', 1)
                    value = observed[field]
                    if (comparison == 'maximum' and value > bound) or (comparison == 'exact' and value != bound):
                        report['failures'].append(f'{field}: {value} violates {comparison} {bound}')
        report['completed'] = not report['failures']
    except Exception as error:
        report['failures'].append(str(error))
    finally:
        (args.output/'preparation-result.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({'completed':report['completed'], 'failures':report['failures']}))
    return 0 if report['completed'] else 4


if __name__ == '__main__':
    raise SystemExit(main())
