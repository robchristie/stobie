"""Frozen compact-mask conversion and native qualification; protected outputs stay in store."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
REPO = Path(__file__).resolve().parents[1]
STORE = Path('/nvme/development/emuella/emuella-testdata/artifacts/rareplanes-expanded-v1')
COVERAGE = Path('/nvme/development/emuella/emuella-workspace/docs/evidence/representation-efficiency/source-coverage.json')

def require(condition, message='required invariant failed'):
    if not condition:
        raise ValueError(message)

def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()

def identity(path):
    path = Path(path).resolve()
    return dict(path=str(path), bytes=path.stat().st_size, sha256=digest(path))

def write(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')

def check(record):
    require(identity(record['path']) == record, f"identity changed: {record['path']}")

def indices(requests):
    """First/last window for every level and component selection, including clipped edges."""
    groups = {}
    for i, request in enumerate(requests):
        groups.setdefault((request['discard'], tuple(request['components'])), []).append(i)
    return sorted({i for group in groups.values() for i in (group[0], group[-1])})

def oracle(root, old_root, records):
    """Bounded per-tile unpacking is measurement scratch, never client residency."""
    import numpy as np
    manifest = json.loads((root / 'manifest.json').read_text())
    profile = manifest['identity']['profile']
    validity = manifest['identity']['validity']
    bands = validity['bands']
    expected = {(r['tile'], r['discard'], r['band'], r['plane']): r for r in records}
    cells = 0
    working_peak = 0
    for discard, catalogue in enumerate(validity['tile_sha256']):
        scale = 1 << discard
        cols = (profile['width'] + profile['tile_edge'] - 1) // profile['tile_edge']
        for tile, tagged in enumerate(catalogue):
            width = (min(profile['tile_edge'], profile['width'] - tile % cols * profile['tile_edge']) + scale - 1) // scale
            height = (min(profile['tile_edge'], profile['height'] - tile // cols * profile['tile_edge']) + scale - 1) // scale
            pixels = width * height
            plane = (pixels + 7) // 8
            raw = (root / f'masks/{discard}/{tile}.bin').read_bytes()
            legacy = (old_root / f'masks/{discard}/{tile}.bin').read_bytes()
            state, hashed = tagged.split(':')
            require(raw[0] == int(state) and hashlib.sha256(raw).hexdigest() == hashed)
            count = 1 if discard == 0 else 2
            require(len(raw) == (1 if state in ('0', '1') else len(legacy) + 1))
            for c, band in enumerate(bands):
                for name, offset in [('all', 0), ('any', count - 1)]:
                    start = (c * count + offset) * plane
                    old = np.unpackbits(np.frombuffer(legacy[start:start + plane], dtype=np.uint8), bitorder='little')[:pixels]
                    actual = np.full(pixels, int(state), dtype=np.uint8) if state in ('0', '1') else np.unpackbits(np.frombuffer(raw[1 + start:1 + start + plane], dtype=np.uint8), bitorder='little')[:pixels]
                    record = expected[tile, discard, band, name]
                    require(np.array_equal(old, actual), 'legacy/new bit disagreement')
                    require(hashlib.sha256(actual).hexdigest() == record['expected_sha256'] == record['actual_sha256'], 'original GDAL oracle mismatch')
                    require(int(actual.sum()) == record['valid'] and pixels == record['samples'])
                    cells += pixels
                    working_peak = max(working_peak, len(raw) + len(legacy) + old.nbytes + actual.nbytes)
    return dict(original_gdal_oracle_plane_cells=cells, false_valid=0, false_invalid=0, legacy_compact_agreement=True, oracle_array_working_peak_bytes=working_peak, source_oracle_reused='Exact expected and actual per-band ALL/ANY hashes from authenticated original GDAL receipt')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', required=True)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--probe', required=True, type=Path)
    parser.add_argument('--suffix', default='01')
    args = parser.parse_args()
    require(args.suffix.isdigit() and len(args.suffix) == 2)
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
    require(head == args.candidate and len(head) == 40)
    require(not subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=REPO))
    coverage = json.loads(COVERAGE.read_text())
    require(coverage['read_only_verification']['source_objects_sha256_and_bytes_matched'] == 38)
    summary = json.loads((REPO / 'docs/viewer-acceptance-full-scenes-results.json').read_text())
    execution = STORE / f'representation-efficiency-masks-native-{args.suffix}'
    execution.mkdir()
    build = dict(candidate=head, binary=identity(args.binary), probe=identity(args.probe), source_coverage=identity(COVERAGE), protocol=identity(__file__), historical_oracle_summary=identity(REPO / 'docs/viewer-acceptance-full-scenes-results.json'))
    write(execution / 'build.json', build)
    scenes = []
    totals = []
    for asset in summary['results']:
        check(asset['source'])
        for name in ('masks', 'requests', 'native', 'regional_agreement'):
            check(asset['evidence'][name])
        original = STORE / asset['output_name']
        old_root = original / 'representation'
        output = STORE / f"representation-efficiency-masks-{asset['name']}-{args.suffix}"
        output.mkdir()
        lineage = json.loads((original / 'lineage.json').read_text())
        lineage.update(operation_owner='compact-masks', parent_group=str(original), candidate=head, modifications='Exact version-2 validity compaction only; image payload and descriptors unchanged; quality-rejected diagnostic imagery, no viewer acceptance.')
        write(output / 'lineage.json', lineage)
        shutil.copyfile(STORE / 'source/LICENSE.txt', output / 'LICENSE.txt')
        require(digest(output / 'LICENSE.txt') == lineage['notice_sha256'])
        if not (execution / 'lineage.json').exists():
            write(execution / 'lineage.json', lineage)
            shutil.copyfile(output / 'LICENSE.txt', execution / 'LICENSE.txt')
        command = [str(args.binary), 'compact-validity', '--input', str(old_root), '--output', str(output / 'representation')]
        with (output / 'conversion.json').open('x') as out, (output / 'conversion.log').open('x') as err:
            subprocess.run(command, stdout=out, stderr=err, check=True, timeout=120)
        conversion = json.loads((output / 'conversion.json').read_text())
        masks = json.loads(Path(asset['evidence']['masks']['path']).read_text())
        result = oracle(output / 'representation', old_root, masks['records'])
        result.update(conversion=identity(output / 'conversion.json'), masks_oracle=asset['evidence']['masks'], immutable_image_source=asset['source'], candidate=head)
        write(output / 'oracle.json', result)
        requests = json.loads(Path(asset['evidence']['requests']['path']).read_text())
        scenes.append(dict(root=str(output / 'representation'), legacy_root=str(old_root), indices=indices(requests)))
        totals.append(dict(asset=asset['asset'], name=asset['name'], root=str(output / 'representation'), conversion=identity(output / 'conversion.json'), oracle=identity(output / 'oracle.json')))
        print(f"{asset['name']}: {conversion['legacy_mask_bytes']} -> {conversion['mask_bytes']} mask bytes; oracle matched", flush=True)
    write(execution / 'native-input.json', scenes)
    command = [str(args.probe), str(execution / 'native-input.json'), str(execution / 'native.json')]
    with (execution / 'native.log').open('x') as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=600)
    for asset in summary['results']:
        check(asset['source'])
    check(build['binary'])
    check(build['probe'])
    check(build['source_coverage'])
    write(execution / 'result.json', dict(schema='representation-efficiency-masks/1', candidate=head, disposition='quality-rejected-diagnostic-only', scoped_masks_only=True, assets=totals, native=identity(execution / 'native.json'), build=identity(execution / 'build.json')))
    print(f"Native evidence: {execution / 'result.json'}", flush=True)
if __name__ == '__main__':
    main()
