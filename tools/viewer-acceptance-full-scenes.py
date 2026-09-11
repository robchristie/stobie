#!/usr/bin/env python3
"""Prepare frozen complete-scene diagnostic evidence; never select or accept assets."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
from types import SimpleNamespace

import numpy as np
from osgeo import gdal

REVISION = '4a1594b0840eadae26a4d6005d50d9201734a1a7'
CODEC = '6586e3d50f95429b242cb2e3535742b002784f2d'
STORE = Path('/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1')
BUILD = Path('/nvme/development/emuella/.build-targets/viewer-acceptance/full-scenes-build')
GDAL = Path('/home/rob/pixi/.pixi/envs/default/lib/libgdal.so.39.3.13.0')
REPO = Path(__file__).resolve().parents[1]
COHORT = {
    '94_104001000B823500-PAN16': (4, [1]),
    '94_104001000B823500-RGB16': (12, [5, 3, 2]),
    '106_10400100413CDF00-PAN16': (4, [1]),
    '106_10400100413CDF00-RGB16': (12, [5, 3, 2]),
    '105_104001002F92BB00-RGB8': (4, [1, 2, 3]),
}
DISPOSITION = 'quality-rejected-diagnostic-only'
VIEWS_SHA256 = '9ab71669f6e182e0883bc3a9bb29608dde5f4c5d3a575e9a27eaaf6c1af1b3ce'

# Authored executable adapter to the pinned public APIs, not a codec change.
# Repeated selection means repeated identical legal requests; duplicate component
# indices remain rejected by Region::validate.
NATIVE_HELPER = r'''
use anyhow::{ensure, Result};
use emuella_viewer_source::{Region, validity, ClientLimits, SharedClient, checked};
use emuella_viewer_tools::Service;
use emuella_viewer_tools::reference::{Reference, Metrics, checksum};
use std::{collections::BTreeMap, fs, io::Write, path::Path};
fn create(path: &Path, bytes: &[u8]) -> Result<()> {
    fs::OpenOptions::new().write(true).create_new(true).open(path)?.write_all(bytes)?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len() == 4, "representation requests output required");
    let root = Path::new(&args[1]);
    let out = Path::new(&args[3]);
    let reference = Reference::open(root)?;
    let m = &reference.manifest;
    let p = &m.identity.profile;
    let v = m.identity.validity.as_ref().ok_or_else(|| anyhow::anyhow!("mask required"))?;
    let mut checked_bytes = 0u64;
    let mut checked_files = 0u64;
    for d in 0..=p.decomposition_levels {
        for t in 0..p.tiles() {
            let bytes = fs::read(root.join(format!("masks/{d}/{t}.bin")))?;
            v.check(p, t as u16, d, &bytes)?;
            checked_bytes += bytes.len() as u64;
            checked_files += 1;
        }
    }
    let requests: Vec<Region> = serde_json::from_slice(&fs::read(&args[2])?)?;
    let mut records = Vec::new();
    let mut mask_read_bytes = 0u64;
    let mut service = Service::open(&[root.to_path_buf()], None, false)?;
    let mut client = SharedClient::new(ClientLimits::default());
    client.register(m.clone())?;
    for (i, r) in requests.iter().enumerate() {
        let mut masks = BTreeMap::new();
        for t in r.tiles(p)? {
            let bytes = fs::read(root.join(format!("masks/{}/{t}.bin", r.discard)))?;
            v.check(p, t, r.discard, &bytes)?;
            mask_read_bytes += bytes.len() as u64;
            masks.insert(t, bytes);
        }
        let valid = validity::combine_region(p, r, |t, _| Ok(masks[&t].as_slice()))?;
        let mut metrics = Metrics::default();
        let frame = reference.decode(r, &mut metrics)?;
        for t in client.missing_tiles(&m.tid, r)? {
            let b = service.route(&format!("/descriptor/{}/{t}?tid={}", m.target, m.tid))?.body;
            client.install_descriptor(&m.tid, t, &b)?;
        }
        for t in client.missing_masks(&m.tid, r)? {
            let b = service.route(&format!("/mask/{}/{}/{t}?tid={}", m.target, r.discard, m.tid))?.body;
            client.install_mask(&m.tid, t, r.discard, &b)?;
        }
        for _ in 0..64 {
            if client.ready(&m.tid, r)? { break; }
            let q = client.request(&m.tid, r, 256 << 10)?;
            let response = service.route(&format!("/jpip?{}", checked(q.query())?))?;
            let mut reader = client.begin_response_headers(&m.tid,
                response.headers.iter().map(|(k,v)| (k.as_str(),v.as_str())))?;
            client.receive(&mut reader, &response.body)?;
            client.finish(reader)?;
        }
        ensure!(client.ready(&m.tid, r)?, "regional dependencies did not converge");
        let decoded = client.decode(&m.tid, r)?;
        ensure!(decoded.width == frame.width && decoded.height == frame.height, "regional geometry mismatch");
        ensure!(decoded.validity.as_ref() == Some(&valid), "regional validity mismatch");
        for (c, plane) in decoded.planes.iter().enumerate() {
            for i in 0..(frame.width * frame.height) as usize {
                let value = if decoded.bits_per_sample == 8 { u16::from(plane[i]) }
                    else { u16::from_le_bytes([plane[2*i], plane[2*i+1]]) };
                ensure!(value == frame.samples[i * r.components.len() + c], "regional reference sample mismatch");
            }
        }
        create(&out.join(format!("region-{i}.validity")), &valid)?;
        let bytes: Vec<u8> = frame.samples.iter().flat_map(|v| v.to_le_bytes()).collect();
        create(&out.join(format!("region-{i}.u16le")), &bytes)?;
        records.push(serde_json::json!({"region": r, "width":frame.width,
            "height":frame.height, "fnv1a64_u16le":checksum(&frame.samples), "metrics":metrics,
            "cached_regional_samples_and_validity_match":true}));
    }
    create(&out.join("native.json"), &serde_json::to_vec_pretty(&serde_json::json!({
        "tid":m.tid, "checked_mask_bytes":checked_bytes, "checked_mask_files":checked_files,
        "regional_mask_read_bytes":mask_read_bytes, "records":records,
        "in_process_service_client_metrics":client.metrics
    }))?)?;
    Ok(())
}
'''


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def write_json(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def identity(path):
    path = Path(path).resolve(strict=True)
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest(path)}


def check_identity(record):
    require(identity(record['path']) == record, 'file byte/source identity changed: ' + record['path'])


def module(name):
    path = BUILD / 'source/tools' / (name + '.py')
    spec = importlib.util.spec_from_file_location(name, path)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


def reduce_mask(native, discard):
    """Independent original-grid clipped AND/OR; bounded to one tile or region."""
    scale = 1 << discard
    y = np.arange(0, native.shape[-2], scale)
    x = np.arange(0, native.shape[-1], scale)
    return tuple(op.reduceat(op.reduceat(native != 0, y, axis=-2), x, axis=-1)
                 for op in (np.logical_and, np.logical_or))


def unpack_mask(data, components, height, width, discard):
    pixels = height * width
    length = (pixels + 7) // 8
    planes = 1 if discard == 0 else 2
    require(len(data) == components * planes * length, 'mask length mismatch')
    values = np.unpackbits(np.frombuffer(data, dtype=np.uint8).reshape(components, planes, length),
                           axis=-1, bitorder='little')
    require(not values[:, :, pixels:].any(), 'noncanonical mask padding')
    values = values[:, :, :pixels].reshape(components, planes, height, width).astype(bool)
    all_valid, any_valid = values[:, 0], values[:, -1]
    require(not (all_valid & ~any_valid).any(), 'mask all without any')
    return all_valid, any_valid


def differences(expected, actual):
    require(expected.shape == actual.shape, 'mask geometry mismatch')
    return {'false_valid': int((actual & ~expected).sum()),
            'false_invalid': int((expected & ~actual).sum()),
            'samples': int(expected.size), 'valid': int(expected.sum()),
            'expected_sha256': hashlib.sha256(expected.astype('u1').tobytes()).hexdigest(),
            'actual_sha256': hashlib.sha256(actual.astype('u1').tobytes()).hexdigest()}


def frozen_files():
    return [Path(__file__), REPO / 'docs/viewer-acceptance-full-scenes.md',
            REPO / 'docs/viewer-acceptance-full-scenes-build.json',
            REPO / 'tools/tests/test_viewer_acceptance_full_scenes.py',
            REPO / 'docs/real-scene-viewing-source-views.json']


def committed_inputs(revision):
    require(subprocess.check_output(['git', '-C', str(REPO), 'rev-parse', revision], text=True).strip()
            == revision, 'full protocol commit required')
    records = {}
    for path in frozen_files():
        relative = str(path.relative_to(REPO))
        committed = subprocess.check_output(['git', '-C', str(REPO), 'show', f'{revision}:{relative}'])
        require(hashlib.sha256(committed).hexdigest() == digest(path), 'protocol input is not committed: ' + relative)
        records[relative] = digest(path)
    return records


def linked_libraries(path):
    result = subprocess.check_output(['ldd', str(path)], text=True)
    require('not found' not in result, 'unresolved linked dependency')
    paths = set()
    for line in result.splitlines():
        for word in line.split():
            if word.startswith('/'):
                paths.add(Path(word).resolve())
    return [identity(p) for p in sorted(paths)]


def build_record():
    """Build only authored code; no protected reads or measurement."""
    require(not (REPO / 'docs/viewer-acceptance-full-scenes-build.json').exists(),
            'build record already exists; do not overwrite a frozen build')
    source = BUILD / 'source'
    require(digest(source / 'Cargo.lock') == hashlib.sha256(subprocess.check_output(
        ['git', '-C', str(REPO), 'show', f'{REVISION}:Cargo.lock'])).hexdigest(), 'archive lock mismatch')
    require(digest(BUILD / 'source.tar') == hashlib.sha256(subprocess.check_output(
        ['git', '-C', str(REPO), 'archive', REVISION])).hexdigest(), 'archive revision mismatch')
    archived = set()
    with tarfile.open(BUILD / 'source.tar') as archive:
        for member in archive.getmembers():
            if member.isfile():
                archived.add(source / member.name)
                require(hashlib.file_digest(archive.extractfile(member), 'sha256').hexdigest()
                        == digest(source / member.name), 'archive source drift')
    require({p for p in source.rglob('*') if p.is_file()} == archived, 'unarchived build source')
    helper = BUILD / 'helper'
    helper.mkdir(exist_ok=True)
    (helper / 'src').mkdir(exist_ok=True)
    (helper / 'src/main.rs').write_text(NATIVE_HELPER)
    (helper / 'Cargo.toml').write_text(
        '[package]\nname="full-scenes-native"\nversion="0.0.0"\nedition="2024"\n'
        '[dependencies]\nanyhow="1.0.100"\nserde_json="1.0.149"\n'
        f'emuella-viewer-source={{path="{source}/crates/emuella-viewer-source"}}\n'
        f'emuella-viewer-tools={{path="{source}/apps/emuella-viewer-tools"}}\n'
        '[profile.release]\nlto="thin"\ncodegen-units=1\n')
    (helper / 'Cargo.lock').write_bytes((source / 'Cargo.lock').read_bytes())
    command = ['cargo', 'build', '--offline', '--release', '--manifest-path', str(helper / 'Cargo.toml')]
    with (BUILD / 'helper-build.log').open('w') as log:
        subprocess.run(command, env={**os.environ, 'CARGO_TARGET_DIR': str(BUILD / 'helper-cargo')},
                       stdout=log, stderr=subprocess.STDOUT, check=True)
    import tomllib
    lock = tomllib.loads((source / 'Cargo.lock').read_text())
    helper_lock = tomllib.loads((helper / 'Cargo.lock').read_text())
    allowed = {(p['name'], p['version'], p.get('source'), p.get('checksum')) for p in lock['package']}
    require(all((p['name'], p['version'], p.get('source'), p.get('checksum')) in allowed
                for p in helper_lock['package'] if p['name'] != 'full-scenes-native'), 'helper dependency drift')
    tool = BUILD / 'cargo/release/emuella-viewer-tools'
    native = BUILD / 'helper-cargo/release/full-scenes-native'
    paths = [tool, native, GDAL, source / 'Cargo.lock', helper / 'Cargo.lock',
             helper / 'src/main.rs', helper / 'Cargo.toml', BUILD / 'source.tar',
             BUILD / 'build.log', BUILD / 'helper-build.log']
    records = {str(p.relative_to(BUILD)) if p.is_relative_to(BUILD) else 'gdal': identity(p) for p in paths}
    deps = {r['path']: r for p in (tool, native, GDAL) for r in linked_libraries(p)}
    # Hash locally installed build inputs; do not inspect external codec text.
    inputs = set(archived)
    for depfile in [tool.with_suffix('.d'), native.with_suffix('.d')]:
        for word in depfile.read_text().split():
            if word.startswith('/') and Path(word).is_file():
                inputs.add(Path(word))
    input_path = BUILD / 'build-inputs.json'
    input_path.write_text(json.dumps([identity(p) for p in sorted(inputs)], indent=2) + '\n')
    records['build-inputs.json'] = identity(input_path)
    for line in Path('/proc/self/maps').read_text().splitlines():
        mapped = line.split()[-1]
        if mapped.startswith('/') and ('.so' in mapped or Path(mapped).resolve() == Path(sys.executable).resolve()):
            record = identity(mapped)
            deps[record['path']] = record
    for path in (source / 'tools').glob('viewer-*.py'):
        records[str(path.relative_to(BUILD))] = identity(path)
    write_json(REPO / 'docs/viewer-acceptance-full-scenes-build.json', {
        'schema': 'viewer-acceptance-full-scenes-build/1', 'source_revision': REVISION,
        'source_tree': subprocess.check_output(['git', '-C', str(REPO), 'rev-parse', REVISION + '^{tree}'], text=True).strip(),
        'source_method': 'clean git archive; offline locked native tool build; authored public-API adapter',
        'linked_codec_revision': CODEC, 'files': records, 'linked_libraries': list(deps.values()),
        'tool': identity(tool), 'native_helper': identity(native), 'gdal': identity(GDAL),
        'helper_command': command, 'tool_command': ['cargo', 'build', '--offline', '--locked', '--release', '-p', 'emuella-viewer-tools'],
        'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
        'cargo': subprocess.check_output(['cargo', '-V'], text=True),
        'python': sys.version, 'numpy': np.__version__, 'python_gdal': gdal.VersionInfo(),
        'measurement_performed': False,
    })


def validate_grant(args, build, bindings):
    grant = json.loads(args.grant.read_text())
    require(grant.get('schema') == 'viewer-acceptance-full-scenes-grant/1'
            and grant.get('disposition') == DISPOSITION
            and grant.get('protocol_commit') == args.protocol_commit
            and grant.get('protocol_files') == bindings
            and grant.get('build_sha256') == digest(args.build_identity)
            and grant.get('asset') == args.asset and grant.get('output_name') == args.output_name
            and grant.get('source_coverage_sha256') == digest(args.source_coverage)
            and grant.get('max_preparations') == 1
            and isinstance(grant.get('operation_owner'), str) and bool(grant['operation_owner'].strip()),
            'missing or mismatched coordinator diagnostic grant')
    require(build['source_revision'] == REVISION and build['linked_codec_revision'] == CODEC,
            'wrong diagnostic native build')
    return grant


def check_manifest(manifest, asset):
    bpp, bands = COHORT[asset['id']]
    p = manifest['identity']['profile']
    i = manifest['identity']
    require(i['source_sha256'] == asset['source_sha256'] and i['bands'] == bands
            and i['codec_revision'] == CODEC
            and i['encoding_contract'] == 'indexed-htonly-no-mct-irreversible97-one-layer-rate-search-v1'
            and p == {'width': asset['image']['width'], 'height': asset['image']['height'],
                      'tile_edge': 512, 'decomposition_levels': 6, 'bits_per_sample': asset['image']['precision'],
                      'components': len(bands), 'bits_per_pixel': bpp}, 'representation/configuration drift')
    v = i['validity']
    tiles = ((p['width'] + 511) // 512) * ((p['height'] + 511) // 512)
    require(v['source_sha256'] == asset['source_sha256'] and v['bands'] == bands
            and v['policy'] == 'source-validity-v1'
            and len(v['tile_sha256']) == 7 and all(len(level) == tiles for level in v['tile_sha256'])
            and len(manifest['descriptor_sha256']) == tiles, 'mask/source catalogue mismatch')


def check_masks(ds, asset, representation, output):
    manifest = json.loads((representation / 'manifest.json').read_text())
    check_manifest(manifest, asset)
    records, boundary_points = [], {}
    tiles = 0
    for y in range(0, ds.RasterYSize, 512):
        for x in range(0, ds.RasterXSize, 512):
            w, h = min(512, ds.RasterXSize - x), min(512, ds.RasterYSize - y)
            masks = np.stack([ds.GetRasterBand(b).GetMaskBand().ReadAsArray(x, y, w, h) != 0 for b in asset['bands']])
            for c, band in enumerate(asset['bands']):
                original = ds.GetRasterBand(band).ReadAsArray(x, y, w, h)
                nodata = asset['statistics'][c]['nodata']
                require(np.array_equal(masks[c], np.ones((h, w), bool) if nodata is None else original != nodata),
                        'existing quality population differs from original GDAL mask')
                # Include a one-pixel halo so transitions exactly on tile seams are observed.
                hx, hy = max(0, x - 1), max(0, y - 1)
                halo = ds.GetRasterBand(band).GetMaskBand().ReadAsArray(hx, hy, x + w - hx, y + h - hy) != 0
                transitions = np.zeros_like(halo)
                transitions[1:] |= halo[1:] != halo[:-1]
                transitions[:, 1:] |= halo[:, 1:] != halo[:, :-1]
                if c not in boundary_points and transitions.any():
                    by, bx = np.argwhere(transitions)[0]
                    boundary_points[c] = [int(hx + bx), int(hy + by)]
            for d in range(7):
                expected = reduce_mask(masks, d)
                path = representation / f'masks/{d}/{tiles}.bin'
                require(digest(path) == manifest['identity']['validity']['tile_sha256'][d][tiles], 'mask digest mismatch')
                actual = unpack_mask(path.read_bytes(), len(asset['bands']), *expected[0].shape[1:], d)
                for c, band in enumerate(asset['bands']):
                    for name, wanted, got in zip(('all', 'any'), expected, actual):
                        records.append({'tile': tiles, 'discard': d, 'band': band, 'plane': name,
                                        **differences(wanted[c], got[c])})
            tiles += 1
    report = {'complete_source_geometry': True, 'tiles': tiles, 'levels': 7,
              'quality_population_gdal_equality': True, 'boundary_points': boundary_points,
              'false_valid': sum(r['false_valid'] for r in records),
              'false_invalid': sum(r['false_invalid'] for r in records), 'records': records}
    write_json(output / 'masks.json', report)
    require(report['false_valid'] == report['false_invalid'] == 0, 'exact mask equality failed')
    return report


def regional_requests(asset, boundaries):
    width, height = asset['image']['width'], asset['image']['height']
    windows = [{k: v[k] for k in ('x', 'y', 'width', 'height')} for v in asset['views']]
    for px, py in [[0, 0], [511, 511], [width - 1, 0], [0, height - 1], [width - 1, height - 1], *boundaries.values()]:
        x, y = max(0, px - 65), max(0, py - 65)
        windows.append({'x': x, 'y': y, 'width': min(193, width - x), 'height': min(193, height - y)})
    selections = [[c] for c in range(len(asset['bands']))] + [list(range(len(asset['bands'])))]
    return [{**w, 'discard': d, 'components': c} for w in windows for d in range(7) for c in selections for _ in range(2)]


def regional_expected(ds, asset, request):
    scale = 1 << request['discard']
    x, y = ((request[k] + scale - 1) // scale * scale for k in ('x', 'y'))
    right = min(ds.RasterXSize, (request['x'] + request['width'] + scale - 1) // scale * scale)
    bottom = min(ds.RasterYSize, (request['y'] + request['height'] + scale - 1) // scale * scale)
    masks = np.stack([ds.GetRasterBand(asset['bands'][c]).GetMaskBand().ReadAsArray(x, y, right - x, bottom - y) != 0
                      for c in request['components']])
    return reduce_mask(masks, request['discard'])[0].all(axis=0)


def check_regions(ds, asset, output, requests, native):
    require(len(native['records']) == len(requests), 'incomplete native requests')
    records = []
    for i, (request, record) in enumerate(zip(requests, native['records'])):
        require(record['region'] == request, 'native request mismatch')
        expected = regional_expected(ds, asset, request)
        require(expected.shape == (record['height'], record['width']), 'regional dimensions differ')
        raw = output / f'region-{i}.validity'
        actual = np.fromfile(raw, dtype='u1').reshape(expected.shape)
        require(np.isin(actual, [0, 1]).all(), 'nonbinary native validity')
        comparison = differences(expected, actual.astype(bool))
        samples = output / f'region-{i}.u16le'
        require(samples.stat().st_size == expected.size * len(request['components']) * 2, 'native sample length')
        repeated = i % 2 == 1
        if repeated:
            require(digest(samples) == digest(output / f'region-{i-1}.u16le')
                    and digest(raw) == digest(output / f'region-{i-1}.validity'), 'repeated selection differs')
        records.append({'request': request, **comparison, 'samples': identity(samples),
                        'validity': identity(raw), 'repeated_selection_checked': repeated})
    # Single-band native reconstructions must equal the corresponding RGB plane.
    stride = (len(asset['bands']) + 1) * 2
    for start in range(0, len(records), stride):
        combined = np.fromfile(output / f'region-{start + stride - 2}.u16le', dtype='<u2').reshape(-1, len(asset['bands']))
        for c in range(len(asset['bands'])):
            require(np.array_equal(np.fromfile(output / f'region-{start + 2*c}.u16le', dtype='<u2'), combined[:, c]),
                    'selected-band sample mismatch')
    report = {'false_valid': sum(r['false_valid'] for r in records),
              'false_invalid': sum(r['false_invalid'] for r in records), 'records': records,
              'single_combined_sample_agreement': True, 'browser_agreement': 'not run'}
    write_json(output / 'regional-agreement.json', report)
    require(report['false_valid'] == report['false_invalid'] == 0, 'regional native mask mismatch')
    return report


def persistent_storage(quality, representation, asset):
    result = quality.storage(representation)
    manifest = json.loads((representation / 'manifest.json').read_text())
    masks = [representation / f'masks/{d}/{t}.bin' for d, level in enumerate(manifest['identity']['validity']['tile_sha256']) for t in range(len(level))]
    expected = {representation / 'payload.j2c', representation / 'manifest.json',
                *[representation / f'descriptors/{t}.bin' for t in range(len(manifest['descriptor_sha256']))], *masks}
    files = {p for p in representation.rglob('*') if p.is_file()}
    require(not any(p.is_symlink() for p in representation.rglob('*')), 'representation symlink forbidden')
    require(files == expected | {representation / 'preparation.json'}, 'unexpected persistent files')
    require(manifest['encoded_bytes'] == result['actual_bytes']['payload'], 'encoded byte identity mismatch')
    result['actual_bytes']['masks'] = sum(p.stat().st_size for p in masks)
    mask_bound = asset['image']['width'] * asset['image']['height'] * len(asset['bands']) + 4096 * len(masks)
    result.update(total_bytes=sum(result['actual_bytes'].values()), mask_bound=mask_bound,
                  mask_eligibility=result['actual_bytes']['masks'] <= mask_bound,
                  persistent_bound=result['payload_bound'] + (1 << 20) * (len(manifest['descriptor_sha256']) + 1) + mask_bound,
                  ancillary_preparation_bytes=(representation / 'preparation.json').stat().st_size,
                  delivered_bytes=None,
                  delivery_boundary='Direct-file logical reference reads recorded separately; no application/network delivery measured.')
    result['total_eligible'] = result['total_bytes'] <= result['persistent_bound']
    result['persistent_files'] = [identity(p) for p in sorted(expected)]
    return result


def retained_quality_processes(output):
    """Keep every reference process log in the approved output group."""
    serial = 0

    def execute(command, **kwargs):
        nonlocal serial
        prefix = output / f'quality-process-{serial}'
        serial += 1
        write_json(prefix.with_suffix('.command.json'), command)
        check = kwargs.pop('check', False)
        try:
            completed = subprocess.run(command, **kwargs)
        except subprocess.TimeoutExpired as error:
            for name, data in [('stdout', error.stdout), ('stderr', error.stderr)]:
                data = data or b''
                prefix.with_suffix('.' + name).write_bytes(data.encode() if isinstance(data, str) else data)
            raise
        for name in ('stdout', 'stderr'):
            data = getattr(completed, name) or ''
            prefix.with_suffix('.' + name).write_bytes(data.encode() if isinstance(data, str) else data)
        if check:
            completed.check_returncode()
        return completed

    return SimpleNamespace(run=execute, PIPE=subprocess.PIPE)


def run(args):
    require(args.asset in COHORT, 'undeclared diagnostic product')
    require(Path(args.output_name).name == args.output_name and args.output_name.startswith('viewer-acceptance-'), 'fresh direct output group required')
    build = json.loads(args.build_identity.read_text())
    bindings = committed_inputs(args.protocol_commit)
    grant = validate_grant(args, build, bindings)
    for record in [*build['files'].values(), *build['linked_libraries']]:
        check_identity(record)
    for record in json.loads((BUILD / 'build-inputs.json').read_text()):
        check_identity(record)
    require(args.build_identity.resolve() == (REPO / 'docs/viewer-acceptance-full-scenes-build.json').resolve(),
            'committed diagnostic build record required')
    for key, value in {'LD_LIBRARY_PATH': str(GDAL.parent), 'GDAL_DRIVER_PATH': 'disable',
                       'OMP_NUM_THREADS': '1', 'OPENBLAS_NUM_THREADS': '1'}.items():
        require(os.environ.get(key) == value, 'frozen environment required: ' + key)
    require(build['native_helper']['sha256'] == digest(BUILD / 'helper-cargo/release/full-scenes-native')
            and (BUILD / 'helper/src/main.rs').read_text() == NATIVE_HELPER, 'native adapter drift')
    require(build['numpy'] == np.__version__ and build['python_gdal'] == gdal.VersionInfo()
            and build['python'] == sys.version, 'Python dependency drift')
    require(STORE.resolve() == STORE and not (STORE / args.output_name).exists(), 'store/output path changed or already used')
    require(not (STORE / (args.output_name + '-views')).exists(), 'frozen-view output already exists')
    for prior in STORE.glob('viewer-acceptance-*/protocol.json'):
        record = json.loads(prior.read_text())
        require(not (record.get('disposition') == DISPOSITION
                     and record.get('asset', {}).get('id') == args.asset),
                'diagnostic representation already attempted; no automatic re-preparation')
    receipt = json.loads(args.source_coverage.read_text())
    require(receipt['status'] == 'rights_and_sources_verified_not_viewer_acceptance'
            and receipt['read_only_verification']['source_objects_sha256_and_bytes_matched'] == 38,
            'reviewed 38-source coverage required')
    views_path = REPO / 'docs/real-scene-viewing-source-views.json'
    require(digest(views_path) == VIEWS_SHA256, 'frozen views changed')
    views = json.loads(views_path.read_text())
    asset = next(a for a in views['assets'] if a['id'] == args.asset)
    require(asset['bands'] == COHORT[args.asset][1] and views['scales'] == [1, 4], 'frozen cohort changed')
    original = STORE / 'source' / asset['source_path']
    before = identity(original)
    require(before['sha256'] == asset['source_sha256'], 'original source identity mismatch')
    source, quality = module('viewer-real-scene-source'), module('viewer-acceptance-quality')
    gdal.UseExceptions()
    gdal.SetConfigOption('GDAL_PAM_ENABLED', 'NO')
    gdal.SetCacheMax(64 << 20)
    ds = gdal.Open(str(original), gdal.GA_ReadOnly)
    require((ds.RasterXSize, ds.RasterYSize) == (asset['image']['width'], asset['image']['height']), 'original grid mismatch')
    output = source.fresh_group(STORE, args.output_name,
        'Complete original-scene exact GDAL masks and frozen rate diagnostic derivative; quality-rejected, not selected validation or viewer acceptance; original arrays and streams preserved.')
    representation = output / 'representation'
    write_json(output / 'protocol.json', {'disposition': DISPOSITION, 'selected_configuration': False,
        'protocol_commit': args.protocol_commit, 'protocol_files': bindings, 'grant': grant,
        'grant_identity': identity(args.grant), 'build': build, 'source_coverage': identity(args.source_coverage),
        'source_before': before, 'asset': asset, 'preparations': 1, 'retries': 0,
        'bpp': COHORT[args.asset][0], 'source_cache': 'uncontrolled; no benchmark or production claim'})
    command = [build['tool']['path'], 'prepare']
    options = {'input': original, 'bands': ','.join(map(str, asset['bands'])),
               'mask-input': original, 'mask-bands': ','.join(map(str, asset['bands'])),
               'mask-bits': asset['image']['precision'], 'bits': asset['image']['precision'],
               'gdal-library': GDAL, 'output': representation, 'target': args.asset,
               'bpp': COHORT[args.asset][0], 'tile': 512, 'levels': 6,
               'codec-revision': CODEC, 'retain-incomplete': 'true'}
    for key, value in options.items():
        command.extend(['--' + key, str(value)])
    write_json(output / 'prepare-command.json', command)
    try:
        with (output / 'prepare.stdout').open('x') as stdout, (output / 'prepare.stderr').open('x') as stderr:
            subprocess.run(command, stdout=stdout, stderr=stderr, check=True, timeout=7200)
        masks = check_masks(ds, asset, representation, output)
        requests = regional_requests(asset, masks['boundary_points'])
        write_json(output / 'requests.json', requests)
        with (output / 'native.stdout').open('x') as stdout, (output / 'native.stderr').open('x') as stderr:
            subprocess.run([build['native_helper']['path'], str(representation), str(output / 'requests.json'), str(output)],
                           stdout=stdout, stderr=stderr, timeout=7200, check=True)
        native = json.loads((output / 'native.json').read_text())
        check_regions(ds, asset, output, requests, native)
        measured = argparse.Namespace(store=STORE, tool=Path(build['tool']['path']), views=views_path,
            asset=args.asset, representation=representation, output_name=args.output_name + '-views', source_valid_gate=True)
        quality.subprocess = quality.quality.subprocess = retained_quality_processes(output)
        quality.quality.evaluate(measured)
        frozen_path = STORE / measured.output_name / 'quality.json'
        frozen = json.loads(frozen_path.read_text())
        require(frozen['status'] == 'completed' and len(frozen['records']) == len(asset['views']), 'incomplete frozen quality')
        coverage = quality.full_coverage(measured, asset, representation, output, views)
        storage = persistent_storage(quality, representation, asset)
        references = [r['reference'] for r in coverage['references']] + [r['reference'] for r in frozen['records']] + native['records']
        direct = {key: sum(r['metrics'][key] for r in references)
                  for key in ('direct_file_bytes', 'direct_file_operations', 'descriptor_bytes', 'descriptor_operations')}
        direct.update(native_mask_validation_bytes=native['checked_mask_bytes'],
                      regional_mask_read_bytes=native['regional_mask_read_bytes'],
                      boundary='Instrumented native reference logical file reads only; excludes integrity hashing, Python/GDAL reads and OS cache effects; no network delivery.')
        require(identity(original) == before, 'original source changed')
        require(native['checked_mask_files'] == masks['tiles'] * 7
                and native['checked_mask_bytes'] == storage['actual_bytes']['masks'], 'native mask coverage incomplete')
        write_json(output / 'result.json', {'schema': 'viewer-acceptance-full-scenes/1', 'disposition': DISPOSITION,
            'asset': args.asset, 'selected_configuration': False, 'viewer_acceptance': False,
            'historical_rejection_preserved': True, 'frozen_views_passed': frozen['passed'],
            'full_coverage_passed': coverage['passed'], 'frozen_quality': identity(frozen_path),
            'full_coverage': identity(output / 'full-coverage.json'), 'masks': identity(output / 'masks.json'),
            'regional_agreement': identity(output / 'regional-agreement.json'),
            'storage': storage, 'direct_file_accounting': direct,
            'browser_agreement': 'pending separate app evidence', 'source_before': before, 'source_after': identity(original)})
    except BaseException as error:
        write_json(output / 'failure.json', {'disposition': DISPOSITION, 'error': str(error), 'retained_incomplete': True})
        raise
    finally:
        after = identity(original)
        write_json(output / 'source-after.json', after)
        require(after == before, 'original source changed')
    print(json.dumps({'output': str(output), 'disposition': DISPOSITION}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('build-record', help='Build authored native helper and bind existing clean archive tool; no imagery')
    execute = commands.add_parser('run', help='Requires committed protocol and exact coordinator grant; one preparation only')
    execute.add_argument('--asset', choices=COHORT, required=True)
    execute.add_argument('--output-name', required=True)
    execute.add_argument('--protocol-commit', required=True)
    for name in ('grant', 'source-coverage'):
        execute.add_argument('--' + name, type=Path, required=True)
    execute.add_argument('--build-identity', type=Path, default=REPO / 'docs/viewer-acceptance-full-scenes-build.json')
    args = parser.parse_args()
    if args.command == 'build-record':
        build_record()
    else:
        run(args)


if __name__ == '__main__':
    main()
