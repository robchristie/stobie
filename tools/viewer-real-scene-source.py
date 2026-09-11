#!/usr/bin/env python3
"""Bounded source-only inspection for the frozen real-scene viewing campaign."""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from osgeo import gdal

PREPARED = '6c37b54bf75af0c17c1b67c5ad7bd2d56b5d34d45de9737ddb50d0fc1fe10e7b'
NOTICE = 'f627ad059128fa5246a21e25759c1d33e35c4bb6287d636c4b970f7df57e7eba'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def fresh_group(store, name, modification):
    if not name.startswith(('real-scene-viewing-viewer-', 'viewer-acceptance-')) or Path(name).name != name:
        raise ValueError('fresh direct campaign child required')
    notice = store / 'source/LICENSE.txt'
    manifest = store / 'prepared-final/prepared.json'
    assert digest(notice) == NOTICE
    assert digest(manifest) == PREPARED
    output = store / name
    output.mkdir(exist_ok=False)
    (output / 'LICENSE.txt').write_bytes(notice.read_bytes())
    write_json(output / 'lineage.json', {
        'attribution': 'J. Shermeyer, T. Hossler, A. Van Etten, D. Hogan, R. Lewis and D. Kim; In-Q-Tel - CosmiQ Works and AI.Reverie; RarePlanes Dataset, June 2020',
        'licence': 'CC BY-SA 4.0', 'notice_sha256': NOTICE,
        'prepared_manifest_sha256': PREPARED, 'modifications': modification,
        'source_store': str(store / 'source'),
    })
    return output


def stretch(values, ranges):
    return np.stack([np.rint(np.clip((values[c].astype(float) - lo) / max(1, hi-lo), 0, 1) * 255).astype(np.uint8)
                     for c, (lo, hi) in enumerate(ranges)], axis=-1)


def save_image(path, values, ranges):
    result = stretch(values, ranges)
    ds = gdal.GetDriverByName('MEM').Create('', result.shape[1], result.shape[0], result.shape[2], gdal.GDT_Byte)
    for c in range(result.shape[2]):
        ds.GetRasterBand(c + 1).WriteArray(result[:, :, c])
    gdal.GetDriverByName('PNG').CreateCopy(str(path), ds)


def inspect(store, output):
    gdal.UseExceptions()
    gdal.SetConfigOption('GDAL_PAM_ENABLED', 'NO')
    gdal.SetCacheMax(64 << 20)
    assets = json.loads((store / 'prepared-final/prepared.json').read_text())['assets']
    selected = [a for a in assets if (a['bundle_id'].startswith(('94_', '106_')) and a['product'] in ('PAN16', 'RGB16')) or (a['bundle_id'].startswith('105_') and a['product'] == 'RGB8')]
    records = []
    for asset in selected:
        path = store / 'source' / asset['source_path']
        assert digest(path) == asset['source_sha256']
        ds = gdal.Open(str(path), gdal.GA_ReadOnly)
        bands = asset['band_indices']
        width, height = ds.RasterXSize, ds.RasterYSize
        peak = (1 << asset['image']['precision']) - 1
        ranges = []
        for index in bands:
            band = ds.GetRasterBand(index)
            histogram = np.zeros(peak + 1, dtype=np.int64)
            for y in range(0, height, 128):
                values = band.ReadAsArray(0, y, width, min(128, height-y))
                counts = np.bincount(values.ravel(), minlength=peak+1)
                histogram += counts
            # Nodata excluded from source-derived contrast statistics, retained in captures.
            nodata = band.GetNoDataValue()
            if nodata is not None:
                histogram[int(nodata)] = 0
            cumulative = histogram.cumsum()
            ranges.append([int(np.searchsorted(cumulative, int(np.ceil(cumulative[-1]*q)))) for q in (.02, .98)])
        scale = max(1, int(np.ceil(width / 1200)))
        values = np.stack([ds.GetRasterBand(b).ReadAsArray(0, 0, width, height, max(1,width//scale), max(1,height//scale)) for b in bands])
        name = asset['id'] + '-overview.png'
        save_image(output / name, values, ranges)
        records.append({'asset': asset, 'percentile_ranges': ranges, 'overview': name, 'overview_sha256': digest(output/name), 'overview_source_per_output_pixel': [width/values.shape[2],height/values.shape[1]]})
    write_json(output / 'source-inspection.json', {'method': 'original TIFF only; full-band exact integer histogram in 128-row strips; nearest-rank 2/98 percentiles excluding nodata; nearest-neighbour overview; no compressed output read', 'assets': records})


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--store', type=Path, required=True)
    parser.add_argument('--output-name', required=True)
    args = parser.parse_args()
    inspect(args.store, fresh_group(args.store, args.output_name, 'Source-only nearest-neighbour overview and deterministic per-band linear display stretch; original TIFFs unchanged.'))
