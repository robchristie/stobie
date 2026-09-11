#!/usr/bin/env python3
"""One frozen OpenJPH comparison cohort; execution requires a committed protocol grant.

Only installed command-line tools are used. No codec implementation is imported.
All raster derivatives, process output and run evidence stay in the approved store.
"""
import argparse
from decimal import Decimal, ROUND_HALF_EVEN, localcontext
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import resource
import struct
import subprocess
import sys

import numpy as np
from osgeo import gdal

ROOT = Path(__file__).resolve().parents[1]
STORE = Path('/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1')
PROTOCOL = ROOT / 'docs/viewer-acceptance-independent.json'
GROUP = 'viewer-acceptance-independent-cohort-01'
COMPRESS = Path('/usr/bin/ojph_compress')
EXPAND = Path('/usr/bin/ojph_expand')
MAX_TRIALS = 8


def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'tools' / (name + '.py'))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


source = module('viewer-real-scene-source')
quality = module('viewer-real-scene-quality')


def matched(size, target):
    """Inclusive 1%, using integer arithmetic with no rounded percentage."""
    return size > 0 and abs(size - target) * 100 <= target


def midpoint(lower, upper):
    with localcontext() as context:
        context.prec = 50
        context.rounding = ROUND_HALF_EVEN
        # Seventeen significant decimal digits, ties to even, are the actual CLI input.
        return format((lower * upper).sqrt(), '.16E')


def rate_search(target, invoke):
    """The callback receives only ordinal/qstep; quality never enters this loop."""
    lower, upper = Decimal('0.00001'), Decimal('0.5')
    trials = []
    for ordinal in range(1, MAX_TRIALS + 1):
        qstep = midpoint(lower, upper)
        trial = invoke(ordinal, qstep)
        trials.append(trial)
        if trial['status'] != 'completed':
            return {'status': 'unsupported', 'trials': trials, 'selected': None}
        size = trial['payload_bytes']
        if matched(size, target):
            return {'status': 'matched', 'trials': trials, 'selected': ordinal}
        if size > target:
            lower = Decimal(qstep)
        else:
            upper = Decimal(qstep)
    return {'status': 'unmatched', 'trials': trials, 'selected': None}


def encoder_command(input_path, output_path, qstep):
    return [str(COMPRESS), '-i', str(input_path), '-o', str(output_path),
            '-num_decomps', '6', '-qstep', qstep, '-reversible', 'false',
            '-colour_trans', 'false', '-prog_order', 'LRCP', '-block_size', '{64,64}',
            '-tile_size', '{512,512}', '-tile_offset', '{0,0}',
            '-image_offset', '{0,0}', '-tlm_marker', 'false']


def cod_contract(body):
    # Bounded marker metadata only; never inspect packet or entropy coding content.
    if len(body) < 10:
        raise ValueError('truncated COD')
    flags, progression, layers, mct, levels, bw, bh, style, transform = struct.unpack_from('>BBHBBBBBB', body)
    if (flags, progression, layers, mct, levels, bw, bh, style, transform) != (0, 0, 1, 0, 6, 4, 4, 64, 0) or len(body) != 10:
        raise ValueError('unsupported COD: require LRCP/one-layer/no-MCT/D6/block64/HT/9-7/default precincts')
    return {'progression': 'LRCP', 'layers': layers, 'mct': False, 'decompositions': levels,
            'block': [64, 64], 'block_style': style, 'transform': 'irreversible97',
            'precincts': 'default', 'sop': False, 'eph': False}


def inspect_stream(path, image):
    """Check main/tile coding metadata and tile-part extents, skipping packet data.

    This deliberately narrow recogniser rejects unhandled coding overrides. It is
    a format-contract guard, not a general codestream conformance validator.
    """
    size = path.stat().st_size
    tiles, segments = [], []
    saw_siz = saw_cod = saw_qcd = False
    tile_end = None
    with path.open('rb') as stream:
        if stream.read(2) != b'\xff\x4f':
            raise ValueError('missing SOC')
        while True:
            offset = stream.tell()
            marker = stream.read(2)
            if marker == b'\xff\xd9':
                if tile_end is not None or stream.tell() != size:
                    raise ValueError('invalid EOC or trailing bytes')
                break
            if marker == b'\xff\x93':
                if tile_end is None or stream.tell() >= tile_end:
                    raise ValueError('invalid tile-part extent')
                stream.seek(tile_end)
                tile_end = None
                continue
            if len(marker) != 2 or marker[0] != 255:
                raise ValueError('invalid marker boundary')
            length_bytes = stream.read(2)
            if len(length_bytes) != 2:
                raise ValueError('truncated marker length')
            length = int.from_bytes(length_bytes, 'big')
            if length < 2:
                raise ValueError('invalid marker length')
            body = stream.read(length - 2)
            if len(body) != length - 2 or (tile_end is not None and stream.tell() > tile_end):
                raise ValueError('truncated marker body')
            code = marker[1]
            if code == 0x51:
                if saw_siz or tiles or len(body) != 45:
                    raise ValueError('unsupported SIZ')
                _, width, height, x, y, tw, th, tx, ty, components = struct.unpack_from('>H8IH', body)
                if (width, height, x, y, tw, th, tx, ty, components) != (image['width'], image['height'], 0, 0, 512, 512, 0, 0, 3):
                    raise ValueError('geometry or tile support mismatch')
                if body[36:] != bytes([image['precision'] - 1, 1, 1]) * 3:
                    raise ValueError('precision, signedness or sampling mismatch')
                saw_siz = True
            elif code == 0x52:
                cod_contract(body)
                if not tiles:
                    saw_cod = True
            elif code == 0x5C:
                if not body or body[0] & 31 != 2 or len(body) != 39:
                    raise ValueError('unsupported quantisation family')
                if not tiles:
                    saw_qcd = True
            elif code == 0x90:
                if len(body) != 8 or tile_end is not None or not (saw_siz and saw_cod and saw_qcd):
                    raise ValueError('unsupported tile-part header')
                tile, extent, part, parts = struct.unpack('>HIBB', body)
                if tile != len(tiles) or part != 0 or parts not in (0, 1) or extent < 14 or offset + extent > size - 2:
                    raise ValueError('require one complete raster-order tile part per tile')
                tile_end = offset + extent
                tiles.append({'tile': tile, 'bytes': extent})
            elif code not in (0x50, 0x64):  # CAP and descriptive COM only.
                raise ValueError('unsupported coding marker or override')
            segments.append({'marker': f'{code:02x}', 'bytes': length + 2,
                             'sha256': hashlib.sha256(body).hexdigest()})
    expected = ((image['width'] + 511) // 512) * ((image['height'] + 511) // 512)
    if not (saw_siz and saw_cod and saw_qcd) or len(tiles) != expected:
        raise ValueError('incomplete stream geometry')
    return {'contract_verified': True, 'tiles': tiles, 'marker_metadata': segments,
            'payload_bytes': size, 'shared_bytes': size - sum(t['bytes'] for t in tiles)}


def identity(path):
    return {'path': str(path), 'sha256': source.digest(path), 'bytes': path.stat().st_size}


def check_identity(item):
    path = Path(item['path'])
    if str(path).startswith(str(STORE) + '/') and path.resolve() != path:
        raise ValueError('protected input indirection is unsupported')
    if source.digest(path) != item['sha256'] or path.stat().st_size != item['bytes']:
        raise ValueError('frozen file identity mismatch')


def check_revision(revision, paths):
    actual = subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != revision or len(revision) != 40:
        raise ValueError('execution revision must be the exact committed HEAD')
    for relative in paths:
        committed = subprocess.check_output(['git', '-C', str(ROOT), 'show', f'{revision}:{relative}'], stderr=subprocess.DEVNULL)
        if (ROOT / relative).read_bytes() != committed:
            raise ValueError('protocol-bearing file differs from committed revision')


def check_grant(grant, revision, protocol_sha256):
    if (grant.get('schema') != 'viewer-acceptance-independent-grant/1'
            or grant.get('revision') != revision
            or grant.get('protocol_sha256') != protocol_sha256
            or grant.get('output_group') != GROUP
            or grant.get('exclusive_measurement_window') is not True
            or not isinstance(grant.get('measurement_owner'), str)
            or not grant['measurement_owner'].strip()):
        raise ValueError('coordinator grant does not bind this exact cohort/revision/window')


def no_core():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run_process(command, output, label):
    record = {'command': command, 'timeout_seconds': 180, 'status': 'started'}
    source.write_json(output / (label + '-process.json'), record)
    env = dict(os.environ, TMPDIR=str(output / 'tmp'), GDAL_PAM_ENABLED='NO',
               GDAL_DRIVER_PATH='disable', OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1')
    try:
        with (output / (label + '.stdout')).open('wb') as out, (output / (label + '.stderr')).open('wb') as err:
            result = subprocess.run(command, cwd=output, env=env, stdout=out, stderr=err,
                                    timeout=180, check=False, preexec_fn=no_core)
        record.update(status='completed' if result.returncode == 0 else 'failed', exit=result.returncode)
    except (OSError, subprocess.TimeoutExpired) as exc:
        record.update(status='failed', failure_type=type(exc).__name__)
    record['logs'] = [identity(output / (label + suffix)) for suffix in ['.stdout', '.stderr']]
    source.write_json(output / (label + '-process.json'), record)
    return record


def selected_input(crop, asset, output):
    """Copy native selected bands in bounded strips; reopen and prove exact equality."""
    ds = gdal.Open(str(crop), gdal.GA_ReadOnly)
    image = asset['image']
    width, height = image['width'], image['height']
    dtype = gdal.GDT_UInt16 if image['precision'] == 16 else gdal.GDT_Byte
    if (ds.RasterXSize, ds.RasterYSize) != (width, height):
        raise ValueError('crop geometry mismatch')
    for band in asset['bands']:
        src = ds.GetRasterBand(band)
        if src.DataType != dtype or src.GetScale() not in (None, 1) or src.GetOffset() not in (None, 0):
            raise ValueError('non-native source samples')
    out = gdal.GetDriverByName('GTiff').Create(str(output), width, height, 3, dtype,
                                             options=['COMPRESS=NONE', 'INTERLEAVE=PIXEL'])
    for y in range(0, height, 128):
        h = min(128, height - y)
        for c, band in enumerate(asset['bands']):
            out.GetRasterBand(c + 1).WriteArray(ds.GetRasterBand(band).ReadAsArray(0, y, width, h), 0, y)
    out = None
    reopened = gdal.Open(str(output), gdal.GA_ReadOnly)
    for y in range(0, height, 128):
        h = min(128, height - y)
        for c, band in enumerate(asset['bands']):
            if not np.array_equal(ds.GetRasterBand(band).ReadAsArray(0, y, width, h), reopened.GetRasterBand(c + 1).ReadAsArray(0, y, width, h)):
                raise ValueError('selected TIFF changed samples')
    return dict(identity(output), bands=asset['bands'], precision=image['precision'],
                exact_samples_verified=True, stretch_applied=False)


def read_decoded(path, asset):
    ds = gdal.Open(str(path), gdal.GA_ReadOnly)
    image = asset['image']
    dtype = gdal.GDT_UInt16 if image['precision'] == 16 else gdal.GDT_Byte
    if ds is None or (ds.RasterXSize, ds.RasterYSize, ds.RasterCount) != (image['width'], image['height'], 3):
        raise ValueError('decoded geometry mismatch')
    for c in range(1, 4):
        band = ds.GetRasterBand(c)
        if band.DataType != dtype or band.GetScale() not in (None, 1) or band.GetOffset() not in (None, 0):
            raise ValueError('decoded precision or scaling mismatch')
    return ds


def score_views(crop, decoded_path, asset, scales):
    original_ds = gdal.Open(str(crop), gdal.GA_ReadOnly)
    decoded_ds = read_decoded(decoded_path, asset)
    records = []
    for i, view in enumerate(asset['views']):
        x, y, w, h = [view[k] for k in ['x', 'y', 'width', 'height']]
        original = np.stack([original_ds.GetRasterBand(b).ReadAsArray(x, y, w, h) for b in asset['bands']])
        decoded = np.stack([decoded_ds.GetRasterBand(c).ReadAsArray(x, y, w, h) for c in [1, 2, 3]])
        for scale in scales:
            orig = original if scale == 1 else original.reshape(3, h // scale, scale, w // scale, scale).mean(axis=(2, 4))
            dec = decoded if scale == 1 else decoded.reshape(3, h // scale, scale, w // scale, scale).mean(axis=(2, 4))
            for name, ranges in asset['stretches'].items():
                populations = quality.display_populations(original, source.stretch(orig, ranges), source.stretch(dec, ranges), asset['statistics'], scale)
                for c, band in enumerate(asset['bands']):
                    metric = populations['any_valid'][c]
                    records.append({'view': i, 'scale': scale, 'stretch': name, 'source_band': band,
                                    'populations': {key: value[c] for key, value in populations.items()},
                                    'passed': metric is None or (metric['rmse'] <= 3 and metric['p99_absolute'] <= 12)})
    return {'gate_population': 'source_any_valid', 'cells': records,
            'passed': all(r['passed'] for r in records), 'failed_cells': sum(not r['passed'] for r in records)}


def reconstruction_agreement(decoded_path, asset, references):
    ds = read_decoded(decoded_path, asset)
    records = []
    for view, reference in zip(asset['views'], references, strict=True):
        check_identity(reference)
        x, y, w, h = [view[k] for k in ['x', 'y', 'width', 'height']]
        emuella = np.fromfile(reference['path'], dtype='<u2').reshape(h, w, 3).transpose(2, 0, 1)
        independent = np.stack([ds.GetRasterBand(c).ReadAsArray(x, y, w, h) for c in [1, 2, 3]])
        records.append({'equal_samples': bool(np.array_equal(emuella, independent)),
                        'bands': [quality.errors(emuella[c], independent[c], np.ones((h, w), bool)) for c in range(3)]})
    return {'claim': 'OpenJPH versus retained Emuella reconstruction of the existing Emuella stream, frozen views only',
            'exact_agreement': all(r['equal_samples'] for r in records), 'views': records}


def compare_quality(independent, baseline, asset):
    """Pair frozen valid display cells; this descriptive table makes no cause claim."""
    previous = {}
    for i, record in enumerate(baseline['records']):
        for display in record['display']:
            for c, band in enumerate(asset['bands']):
                previous[i, display['scale'], display['stretch'], band] = display['source_any_valid'][c]
    comparisons = []
    for cell in independent['cells']:
        key = tuple(cell[k] for k in ['view', 'scale', 'stretch', 'source_band'])
        old, new = previous.pop(key), cell['populations']['any_valid']
        if (old is None) != (new is None) or (old and old['samples'] != new['samples']):
            raise ValueError('quality population mismatch')
        comparisons.append({**{k: cell[k] for k in ['view', 'scale', 'stretch', 'source_band']},
                            'baseline': old, 'independent': new,
                            'rmse_delta': None if old is None else new['rmse'] - old['rmse'],
                            'p99_delta': None if old is None else new['p99_absolute'] - old['p99_absolute']})
    if previous:
        raise ValueError('incomplete frozen quality cell pairing')
    return comparisons


def execute_product(product, output):
    output.mkdir()
    (output / 'tmp').mkdir()
    asset = product['asset']
    crop = Path(product['files']['crop']['path'])
    result = {'asset': asset['id'], 'target_bytes': product['target_bytes'], 'status': 'started',
              'quality': None, 'agreement': None, 'encoding_comparison_eligible': False}
    source.write_json(output / 'result.json', result)
    try:
        for item in [*product['files'].values(), *product['references']]:
            check_identity(item)
        manifest = json.loads(Path(product['files']['manifest']['path']).read_text())
        if (manifest['encoded_bytes'] != product['target_bytes']
                or manifest['identity']['source_sha256'] != product['files']['crop']['sha256']
                or manifest['identity']['bands'] != asset['bands']
                or manifest['identity']['payload_sha256'] != product['files']['payload']['sha256']):
            raise ValueError('baseline representation identity mismatch')
        result['baseline_format'] = inspect_stream(Path(product['files']['payload']['path']), asset['image'])
        result['input'] = selected_input(crop, asset, output / 'selected.tif')

        def invoke(ordinal, qstep):
            label = f'trial-{ordinal:02d}'
            payload = output / (label + '.j2c')
            trial = {'ordinal': ordinal, 'qstep': qstep, 'status': 'started'}
            source.write_json(output / (label + '.json'), trial)
            trial['process'] = run_process(encoder_command(output / 'selected.tif', payload, qstep), output, label)
            trial['status'] = trial['process']['status']
            if payload.is_file():
                trial['payload'] = identity(payload)
                trial['payload_bytes'] = payload.stat().st_size
            if trial['status'] == 'completed':
                try:
                    trial['format'] = inspect_stream(payload, asset['image'])
                except (ValueError, OSError, struct.error) as exc:
                    trial.update(status='unsupported', reason=str(exc))
            source.write_json(output / (label + '.json'), trial)
            return trial

        result['rate'] = rate_search(product['target_bytes'], invoke)
        result['status'] = result['rate']['status']
        source.write_json(output / 'result.json', result)
        if result['status'] == 'matched':
            selected = result['rate']['trials'][result['rate']['selected'] - 1]
            result['byte_delta'] = selected['payload_bytes'] - product['target_bytes']
            result['byte_delta_percent'] = result['byte_delta'] * 100 / product['target_bytes']
            for label, payload in [('independent', selected['payload']['path']), ('emuella', product['files']['payload']['path'])]:
                process = run_process([str(EXPAND), '-i', str(payload), '-o', str(output / (label + '.tif')), '-resilient', 'false'], output, label + '-decode')
                result[label + '_decode'] = process
                if process['status'] != 'completed':
                    result['status'] = 'unsupported-reconstruction'
                    break
                result[label + '_decoded'] = identity(output / (label + '.tif'))
                if label == 'independent':
                    result['quality'] = score_views(crop, output / 'independent.tif', asset, product['scales'])
                    baseline = json.loads(Path(product['files']['quality']['path']).read_text())
                    result['quality_comparison'] = compare_quality(result['quality'], baseline, asset)
                else:
                    result['agreement'] = reconstruction_agreement(output / 'emuella.tif', asset, product['references'])
            # Independent reconstruction agreement is reported separately; it is not
            # the encoding-efficiency predicate for two source-matched streams.
            result['encoding_comparison_eligible'] = result['status'] == 'matched'
    except (ValueError, OSError, RuntimeError, struct.error) as exc:
        result.update(status='unsupported', failure_type=type(exc).__name__)
        # Detailed diagnostics stay inside the attributed store.
        (output / 'failure.txt').write_text(str(exc) + '\n')
    finally:
        result['input_identities_unchanged'] = True
        for item in [*product['files'].values(), *product['references']]:
            try:
                check_identity(item)
            except (OSError, ValueError):
                result.update(input_identities_unchanged=False, status='invalid-input-identity', encoding_comparison_eligible=False)
        source.write_json(output / 'result.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', required=True, help='Exact committed Polyorama protocol HEAD')
    parser.add_argument('--timing-grant', required=True, type=Path)
    args = parser.parse_args()
    protocol = json.loads(PROTOCOL.read_text())
    check_revision(args.revision, protocol['committed_files'])
    grant = json.loads(args.timing_grant.read_text())
    check_grant(grant, args.revision, source.digest(PROTOCOL))
    if STORE.resolve() != STORE or args.timing_grant.is_symlink():
        raise ValueError('unexpected store or grant indirection')
    for item in protocol['environment_files']:
        check_identity(item)
    for item in protocol['dependency_files']:
        check_identity(item)
    # mkdir(exist_ok=False) is a permanent single-cohort claim, including failures.
    output = source.fresh_group(STORE, GROUP, 'One bounded OpenJPH independent-encoder screen, exact native selected bands; all rate trials, independent reconstructions and numerical evidence retained. No full-scene or visual acceptance claim.')
    no_core()
    (output / 'tmp').mkdir()
    os.environ['TMPDIR'] = str(output / 'tmp')
    # Capture Python and native-library diagnostics at the file-descriptor level.
    terminal_out = os.dup(1)
    with (output / 'run.stdout').open('wb') as out, (output / 'run.stderr').open('wb') as err:
        os.dup2(out.fileno(), 1)
        os.dup2(err.fileno(), 2)
    gdal.UseExceptions()
    gdal.SetConfigOption('GDAL_PAM_ENABLED', 'NO')
    gdal.SetCacheMax(64 << 20)
    source.write_json(output / 'protocol.json', {'revision': args.revision, 'protocol': protocol,
                      'protocol_sha256': source.digest(PROTOCOL), 'grant': grant,
                      'grant_sha256': source.digest(args.timing_grant),
                      'committed_file_sha256': {p: source.digest(ROOT / p) for p in protocol['committed_files']},
                      'python': sys.version, 'numpy': np.__version__, 'gdal': gdal.VersionInfo()})
    records = []
    for product in protocol['products']:
        records.append(execute_product(product, output / product['key']))
        source.write_json(output / 'cohort.json', {'status': 'in-progress', 'products': records})
    headroom = all(r['encoding_comparison_eligible'] and r['quality']['passed'] for r in records)
    result = {'status': 'completed', 'products': records, 'numerical_only': True,
              'hypothesis': 'Current Emuella per-tile quantiser/rate allocation leaves recoverable display distortion at this aggregate payload.',
              'hypothesis_disposition': 'supports a separate investigation, not a causal policy repair' if headroom else 'not supported by this bounded cohort',
              'minimal_codec_change_justified': False,
              'limitations': 'Black-box comparison cannot isolate quantisation, allocation, transform or entropy implementation causes. Exact reconstruction agreement is a separate frozen-view claim. No second sweep, timing claim, full-scene acceptance, visual acceptance or automatic codec change.'}
    source.write_json(output / 'cohort.json', result)
    sys.stdout.flush()
    os.dup2(terminal_out, 1)
    os.close(terminal_out)
    print(json.dumps({'output_group': GROUP, 'status': 'completed',
                      'products': [{'asset': r['asset'], 'status': r['status']} for r in records]}))


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        # Do not expose protected diagnostics through a caller's terminal/log store.
        print(f'independent comparison stopped ({type(error).__name__}); no retry is authorised', file=sys.stderr)
        sys.exit(1)
