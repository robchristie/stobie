#!/usr/bin/env python3
"""Frozen tile-support quality screen and complete-scene display coverage."""
import argparse
import copy
import importlib.util
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

import numpy as np
from osgeo import gdal


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


source = module('viewer-real-scene-source')
quality = module('viewer-real-scene-quality')


def screen_rect(asset):
    views = asset['views']
    x = min(v['x'] for v in views) // 512 * 512
    y = min(v['y'] for v in views) // 512 * 512
    right = min(asset['image']['width'], ((max(v['x'] + v['width'] for v in views) + 511) // 512) * 512)
    bottom = min(asset['image']['height'], ((max(v['y'] + v['height'] for v in views) + 511) // 512) * 512)
    return x, y, right - x, bottom - y


def tile_supports(asset, rect):
    x, y, w, h = rect
    if x % 512 or y % 512:
        raise ValueError('screen origin must preserve original tile grid')
    rows = []
    for dy in range(0, h, 512):
        for dx in range(0, w, 512):
            original = [x + dx, y + dy, min(512, asset['image']['width'] - x - dx), min(512, asset['image']['height'] - y - dy)]
            if original[2:] != [min(512, w - dx), min(512, h - dy)]:
                raise ValueError('derivative truncates an original tile support')
            rows.append(original)
    return rows


def descriptor_attribution(path, profile):
    """Read the owner-defined EHTIDX01 envelope; never retain packet-header bytes."""
    data = path.read_bytes()
    if len(data) > 1 << 20 or data[:8] != b'EHTIDX01':
        raise ValueError('unsupported descriptor')
    ordinal, offset, length = struct.unpack_from('<HQI', data, 8)
    header = data[22:22 + length]
    qcd = header[-(7 + 6 * profile['decomposition_levels']):]
    if qcd[:2] != b'\xff\x5c' or qcd[4] != 0x62:
        raise ValueError('unsupported owner quantiser family')
    steps = [int.from_bytes(qcd[i:i + 2], 'big') for i in range(5, len(qcd), 2)]
    cursor = 22 + length
    components = [{'component': c, 'packet_header_bytes': 0, 'packet_body_bytes': 0} for c in range(profile['components'])]
    for _ in range(profile['decomposition_levels'] + 1):
        for c in components:
            header_bytes, body_bytes = struct.unpack_from('<II', data, cursor)
            cursor += 8 + header_bytes
            if cursor > len(data):
                raise ValueError('truncated packet metadata')
            c['packet_header_bytes'] += header_bytes
            c['packet_body_bytes'] += body_bytes
    if cursor != len(data):
        raise ValueError('trailing descriptor data')
    return {'tile': ordinal, 'payload_offset': offset, 'descriptor_bytes': len(data),
            'qcd_guard_bits': qcd[4] >> 5, 'qcd_style': qcd[4] & 31,
            'qcd_steps': [{'exponent': v >> 11, 'mantissa': v & 2047} for v in steps],
            'components': components, 'quantiser_search_visits': None,
            'search_observation': 'Final shared-component QCD and packet sizes only; encoder does not expose search visits.'}


def storage(representation):
    manifest = json.loads((representation / 'manifest.json').read_text())
    profile = manifest['identity']['profile']
    tiles = []
    for i, expected in enumerate(manifest['descriptor_sha256']):
        path = representation / 'descriptors' / f'{i}.bin'
        if source.digest(path) != expected:
            raise ValueError('descriptor identity mismatch')
        tiles.append(descriptor_attribution(path, profile))
    payload = representation / 'payload.j2c'
    if source.digest(payload) != manifest['identity']['payload_sha256']:
        raise ValueError('payload identity mismatch')
    budgets = [int(min(512, profile['width'] - x) * min(512, profile['height'] - y) * profile['bits_per_pixel'] / 8) + 128
               for y in range(0, profile['height'], 512) for x in range(0, profile['width'], 512)]
    # Owner emits the shared main header before its tile parts and two-byte EOC after them.
    payload_bound = sum(budgets) + manifest['main_header_bytes'] + 2
    actual = {'payload': payload.stat().st_size, 'descriptors': sum(t['descriptor_bytes'] for t in tiles),
              'manifest': (representation / 'manifest.json').stat().st_size, 'masks': 0}
    packet_bytes = sum(c['packet_header_bytes'] + c['packet_body_bytes'] for t in tiles for c in t['components'])
    return {'actual_bytes': actual, 'total_bytes': sum(actual.values()), 'payload_bound': payload_bound,
            'tile_budget_sum': sum(budgets), 'shared_framing_bytes': manifest['main_header_bytes'] + 2,
            'payload_eligible': actual['payload'] <= payload_bound,
            'descriptors_eligible': all(t['descriptor_bytes'] <= 1 << 20 for t in tiles),
            'manifest_eligible': actual['manifest'] <= 1 << 20,
            'mask_eligibility': 'Not evaluated; this quality-only representation has no validity sidecars.',
            'packet_bytes': packet_bytes, 'codestream_framing_bytes': actual['payload'] - packet_bytes,
            'delivered_bytes': None, 'delivery_boundary': 'Direct-file reference export; application delivery requires separate evidence.',
            'tiles': tiles, 'manifest_sha256': source.digest(representation / 'manifest.json')}


def box_average(values, scale):
    if scale == 1:
        return values
    y = np.arange(0, values.shape[1], scale)
    x = np.arange(0, values.shape[2], scale)
    sums = np.add.reduceat(np.add.reduceat(values.astype(float), y, axis=1), x, axis=2)
    counts = np.minimum(scale, values.shape[1] - y)[:, None] * np.minimum(scale, values.shape[2] - x)[None, :]
    return sums / counts


def histogram_metrics(histogram):
    samples = int(histogram.sum())
    if not samples:
        return None
    magnitudes = np.arange(256)
    # NumPy's frozen method='higher' selects ceil(.99*(n-1)), zero-based.
    rank = int(np.ceil(.99 * (samples - 1))) + 1
    return {'samples': samples, 'rmse': float(np.sqrt(np.dot(histogram, magnitudes ** 2) / samples)),
            'p99_absolute': int(np.searchsorted(histogram.cumsum(), rank))}


def full_coverage(args, asset, representation, output, views):
    ds = gdal.Open(str(args.store / 'source' / asset['source_path']), gdal.GA_ReadOnly)
    histograms = {(scale, name, c, subset): np.zeros(256, dtype=np.int64) for subset in ['any_valid', 'all_valid', 'partial_valid'] for scale in views['scales'] for name in asset['stretches'] for c in range(len(asset['bands']))}
    references = []
    for y in range(0, asset['image']['height'], 512):
        for x in range(0, asset['image']['width'], 512):
            w, h = min(512, asset['image']['width'] - x), min(512, asset['image']['height'] - y)
            raw = output / f'coverage-{x}-{y}.u16le'
            command = [str(args.tool), 'reference-export', '--representation', str(representation), '--output', str(raw), '--x', str(x), '--y', str(y), '--width', str(w), '--height', str(h)]
            completed = subprocess.run(command, capture_output=True, text=True, timeout=120, check=True)
            reference = json.loads(completed.stdout)
            references.append({'x': x, 'y': y, 'width': w, 'height': h, 'reference': reference, 'raw_sha256': source.digest(raw)})
            original = np.stack([ds.GetRasterBand(b).ReadAsArray(x, y, w, h) for b in asset['bands']])
            decoded = np.fromfile(raw, dtype='<u2').reshape(h, w, len(asset['bands'])).transpose(2, 0, 1)
            for scale in views['scales']:
                orig = box_average(original, scale)
                dec = box_average(decoded, scale)
                for name, ranges in asset['stretches'].items():
                    delta = abs(source.stretch(orig, ranges).astype(np.int16) - source.stretch(dec, ranges).astype(np.int16))
                    for c, stats in enumerate(asset['statistics']):
                        valid = np.ones((h, w), bool) if stats['nodata'] is None else original[c] != stats['nodata']
                        # Any valid contributing original sample keeps the cell, including valid-side ringing.
                        fraction = box_average(valid[None], scale)[0]
                        for subset, selected in [('any_valid', fraction > 0), ('all_valid', fraction == 1), ('partial_valid', (fraction > 0) & (fraction < 1))]:
                            histograms[scale, name, c, subset] += np.bincount(delta[:, :, c][selected], minlength=256)
    cells = []
    for (scale, name, c, subset), histogram in histograms.items():
        if subset != 'any_valid':
            continue
        metrics = histogram_metrics(histogram)
        cells.append({'scale': scale, 'stretch': name, 'source_band': asset['bands'][c], 'metrics': metrics,
                      'all_valid': histogram_metrics(histograms[scale, name, c, 'all_valid']),
                      'partial_valid': histogram_metrics(histograms[scale, name, c, 'partial_valid']),
                      'passed': metrics is None or (metrics['rmse'] <= 3 and metrics['p99_absolute'] <= 12)})
    report = {'complete_source_geometry': True, 'cells': cells, 'passed': all(c['passed'] for c in cells),
              'references': references, 'validity': 'Original per-band nodata only; reduced cell retained if any contributing source sample is valid; averaging remains identical over all contributors. Partial scene-edge blocks use their actual contributor count.'}
    source.write_json(output / 'full-coverage.json', report)
    return report


def validate_invocation(args, build, views_sha256):
    if source.digest(args.tool) != build['tool']['sha256']:
        raise ValueError('tool does not match immutable build identity')
    if source.digest(args.gdal_library) != build['gdal']['sha256']:
        raise ValueError('GDAL does not match immutable build identity')
    lock = Path(__file__).resolve().parents[1] / 'Cargo.lock'
    if source.digest(lock) != build['cargo_lock_sha256']:
        raise ValueError('checkout lock does not match immutable build identity')
    reserved = args.asset.startswith('106_')
    if reserved:
        if args.phase != 'full' or args.selection is None:
            raise ValueError('reserved validation requires a frozen development selection receipt')
        selection = json.loads(args.selection.read_text())
        if (selection.get('schema') != 'viewer-acceptance-quality-selection/1'
                or selection.get('views_sha256') != views_sha256
                or selection.get('development_complete_passed') is not True
                or selection.get('tool_sha256') != build['tool']['sha256']):
            raise ValueError('selection receipt does not bind this completed development configuration')
        selected_rate = selection['rgb16_bpp'] if args.asset.endswith('RGB16') else 4
        if args.bpp != selected_rate:
            raise ValueError('reserved validation cannot retune the selected rate')
    elif args.asset not in ['94_104001000B823500-RGB16', '94_104001000B823500-PAN16', '105_104001002F92BB00-RGB8']:
        raise ValueError('undeclared development product')
    elif args.bpp != 4 and args.asset != '94_104001000B823500-RGB16':
        raise ValueError('undeclared rate/product combination')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['store', 'tool', 'gdal-library', 'views', 'timing-grant', 'build-identity']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--selection', type=Path, help='Frozen completed-development receipt required for reserved validation')
    parser.add_argument('--asset', required=True)
    parser.add_argument('--bpp', type=int, choices=[4, 8, 12], required=True)
    parser.add_argument('--phase', choices=['screen', 'full'], required=True)
    parser.add_argument('--output-name', required=True)
    args = parser.parse_args()
    if not args.timing_grant.is_file():
        raise ValueError('coordinator timing grant is absent')
    views = json.loads(args.views.read_text())
    asset = next(a for a in views['assets'] if a['id'] == args.asset)
    build = json.loads(args.build_identity.read_text())
    validate_invocation(args, build, source.digest(args.views))
    gdal.UseExceptions()
    gdal.SetConfigOption('GDAL_PAM_ENABLED', 'NO')
    gdal.SetCacheMax(64 << 20)
    original_path = args.store / 'source' / asset['source_path']
    if source.digest(original_path) != asset['source_sha256']:
        raise ValueError('original source identity mismatch')
    output = source.fresh_group(args.store, args.output_name, 'Frozen viewer acceptance quality-only screen or complete-scene reference derivatives. Original sample arrays, display stretches and source-coordinate view supports unchanged; no visual acceptance claimed.')
    protocol = {'source_revision': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                'scripts': {p.name: source.digest(p) for p in [Path(__file__), *[Path(__file__).with_name('viewer-real-scene-' + n + '.py') for n in ['prepare', 'source', 'quality']]]},
                'views_sha256': source.digest(args.views), 'build_identity': build,
                'selection': None if args.selection is None else {'path': str(args.selection), 'sha256': source.digest(args.selection), 'receipt': json.loads(args.selection.read_text())},
                'tool_sha256': source.digest(args.tool), 'gdal_sha256': source.digest(args.gdal_library),
                'asset': asset, 'phase': args.phase, 'bpp': args.bpp,
                'arguments': {k: str(v) for k, v in vars(args).items()}}
    source.write_json(output / 'protocol.json', protocol)
    if args.phase == 'screen':
        rect = screen_rect(asset)
        supports = tile_supports(asset, rect)
        derivative = output / 'source.tif'
        ds = gdal.Translate(str(derivative), str(original_path), format='GTiff', srcWin=list(rect), creationOptions=['TILED=YES', 'BLOCKXSIZE=512', 'BLOCKYSIZE=512'])
        ds = None
        frozen = copy.deepcopy(views)
        selected = next(a for a in frozen['assets'] if a['id'] == args.asset)
        selected['original_source'] = copy.deepcopy(asset)
        selected['source_path'] = '../' + output.name + '/source.tif'
        selected['source_sha256'] = source.digest(derivative)
        selected['image']['width'], selected['image']['height'] = rect[2:]
        for v in selected['views']:
            v['x'] -= rect[0]
            v['y'] -= rect[1]
        # Verify exact source arrays one bounded original tile at a time.
        original_ds, crop_ds = gdal.Open(str(original_path)), gdal.Open(str(derivative))
        for x, y, w, h in supports:
            for band in asset['bands']:
                if not np.array_equal(original_ds.GetRasterBand(band).ReadAsArray(x, y, w, h), crop_ds.GetRasterBand(band).ReadAsArray(x - rect[0], y - rect[1], w, h)):
                    raise ValueError('derivative changed original samples')
        source.write_json(output / 'crop-lineage.json', {'source': asset, 'rect': rect, 'tile_supports': supports, 'exact_selected_samples_verified': True, 'derivative_sha256': selected['source_sha256']})
        source.write_json(output / 'views.json', frozen)
        measure_views = output / 'views.json'
    else:
        measure_views = args.views
    encode_name = args.output_name + '-encode'
    command = [sys.executable, str(Path(__file__).with_name('viewer-real-scene-prepare.py'))]
    for name, value in {'store': args.store, 'tool': args.tool, 'gdal-library': args.gdal_library, 'views': measure_views,
                        'timing-grant': args.timing_grant, 'asset': args.asset, 'output-name': encode_name, 'bpp': args.bpp,
                        'codec-revision': '6586e3d50f95429b242cb2e3535742b002784f2d'}.items():
        command += ['--' + name, str(value)]
    started = time.perf_counter()
    with (output / 'prepare.stdout').open('w') as out, (output / 'prepare.stderr').open('w') as err:
        subprocess.run(command, stdout=out, stderr=err, timeout=1500, check=True)
    representation = args.store / encode_name / 'representation'
    args.representation = representation
    args.views = measure_views
    args.output_name = output.name + '-views'
    args.source_valid_gate = True
    quality.evaluate(args)
    frozen_result = json.loads((args.store / args.output_name / 'quality.json').read_text())
    result = {'phase': args.phase, 'asset': args.asset, 'bpp': args.bpp, 'wall_seconds': time.perf_counter() - started,
              'frozen_views_passed': frozen_result['passed'], 'storage': storage(representation),
              'frozen_quality': {'path': str(args.store / args.output_name / 'quality.json'), 'sha256': source.digest(args.store / args.output_name / 'quality.json')},
              'full_coverage': None, 'numerical_only': True}
    if args.phase == 'full':
        coverage = full_coverage(args, asset, representation, output, views)
        result['full_coverage'] = {'passed': coverage['passed'], 'path': str(output / 'full-coverage.json'), 'sha256': source.digest(output / 'full-coverage.json')}
    result['source_after_sha256'] = source.digest(original_path)
    if result['source_after_sha256'] != asset['source_sha256']:
        raise ValueError('original source changed')
    source.write_json(output / 'result.json', result)
    print(json.dumps({'output': str(output), 'phase': args.phase, 'asset': args.asset, 'bpp': args.bpp, 'frozen_views_passed': result['frozen_views_passed'], 'full_coverage': result['full_coverage'], 'actual_bytes': result['storage']['actual_bytes']}))


if __name__ == '__main__':
    main()
