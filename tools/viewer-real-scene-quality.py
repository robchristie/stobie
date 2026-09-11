#!/usr/bin/env python3
"""Fixed-view numerical quality using bounded same-codec output and original TIFFs.

Pixels and optional display captures must remain in a fresh approved store child.
This command does not establish independent decoder agreement or visual acceptance.
"""
import argparse
import importlib.util
import json
import subprocess
from pathlib import Path

import numpy as np
from osgeo import gdal

spec = importlib.util.spec_from_file_location('source', Path(__file__).with_name('viewer-real-scene-source.py'))
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)


def errors(original, decoded, mask):
    delta = (decoded.astype(float)-original.astype(float))[mask]
    return None if not delta.size else {'samples': int(delta.size), 'rmse': float(np.sqrt(np.mean(delta**2))), 'mae': float(np.mean(abs(delta))), 'max_absolute': float(abs(delta).max()), 'p99_absolute': float(np.quantile(abs(delta), .99, method='higher'))}


def display_populations(original, original_display, decoded_display, statistics, scale):
    """Retain original validity, including partial reduced cells and valid-side error."""
    bands, height, width = original.shape
    result = {'all_pixels': [], 'any_valid': [], 'all_valid': [], 'partial_valid': []}
    for c, stats in enumerate(statistics):
        valid = np.ones((height, width), bool) if stats['nodata'] is None else original[c] != stats['nodata']
        if scale == 1:
            any_valid = all_valid = valid
        else:
            blocks = valid.reshape(height // scale, scale, width // scale, scale)
            any_valid = blocks.any(axis=(1, 3))
            all_valid = blocks.all(axis=(1, 3))
        populations = {'all_pixels': np.ones(original_display.shape[:2], bool),
                       'any_valid': any_valid, 'all_valid': all_valid, 'partial_valid': any_valid & ~all_valid}
        for name, mask in populations.items():
            result[name].append(errors(original_display[:, :, c], decoded_display[:, :, c], mask))
    return result


def evaluate(args):
    views = json.loads(args.views.read_text())
    a = next(a for a in views['assets'] if a['id'] == args.asset)
    manifest = json.loads((args.representation/'manifest.json').read_text())
    assert manifest['identity']['source_sha256'] == a['source_sha256']
    assert manifest['identity']['bands'] == a['bands']
    assert manifest['identity']['profile']['bits_per_sample'] == a['image']['precision']
    output = source.fresh_group(args.store, args.output_name, 'Bounded same-codec full-tile reconstruction cropped to frozen source-coordinate views; raw U16LE native samples and stretched display derivatives. Originals unchanged.')
    source.write_json(output/'input.json', {'representation':str(args.representation),'manifest':manifest,'views_sha256':source.digest(args.views),'tool_sha256':source.digest(args.tool),'asset':a})
    original_path = args.store/'source'/a['source_path']
    assert source.digest(original_path) == a['source_sha256']
    gdal.UseExceptions(); gdal.SetConfigOption('GDAL_PAM_ENABLED','NO');gdal.SetCacheMax(64<<20)
    ds = gdal.Open(str(original_path),gdal.GA_ReadOnly)
    records=[]
    for i,v in enumerate(a['views']):
        raw=output/f'view-{i}.u16le'
        command=[str(args.tool),'reference-export','--representation',str(args.representation),'--output',str(raw)]
        for k in ['x','y','width','height']:command += ['--'+k,str(v[k])]
        result=subprocess.run(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,timeout=120)
        (output/f'view-{i}.stderr').write_text(result.stderr)
        if result.returncode:
            source.write_json(output/'quality.json',{'passed':False,'status':'decode-failed','command':command,'exit':result.returncode,'completed_views':records})
            return
        reference=json.loads(result.stdout)
        source.write_json(output/f'view-{i}-reference.json',reference)
        h,w=v['height'],v['width']
        original=np.stack([ds.GetRasterBand(b).ReadAsArray(v['x'],v['y'],w,h) for b in a['bands']])
        decoded=np.fromfile(raw,dtype='<u2').reshape(h,w,len(a['bands'])).transpose(2,0,1)
        band_errors=[]
        for c,stats in enumerate(a['statistics']):
            mask = original[c] != stats['nodata'] if stats['nodata'] is not None else np.ones((h,w),bool)
            band_errors.append({'source_band':a['bands'][c],'storage_peak':2**a['image']['precision']-1,'observed_source_range':[int(original[c].min()),int(original[c].max())],'all':errors(original[c],decoded[c],np.ones((h,w),bool)),'valid':errors(original[c],decoded[c],mask),'nodata':errors(original[c],decoded[c],~mask)})
        displays=[]
        for scale in views['scales']:
            orig=original if scale==1 else original.reshape(len(a['bands']),h//scale,scale,w//scale,scale).mean(axis=(2,4))
            dec=decoded if scale==1 else decoded.reshape(len(a['bands']),h//scale,scale,w//scale,scale).mean(axis=(2,4))
            for name,ranges in a['stretches'].items():
                orig8=source.stretch(orig,ranges);dec8=source.stretch(dec,ranges)
                populations = display_populations(original, orig8, dec8, a['statistics'], scale)
                valid_gate = getattr(args, 'source_valid_gate', False)
                metrics = populations['any_valid' if valid_gate else 'all_pixels']
                clipping=[{'source_below':int((orig[c]<lo).sum()),'source_above':int((orig[c]>hi).sum()),'decoded_below':int((dec[c]<lo).sum()),'decoded_above':int((dec[c]>hi).sum())} for c,(lo,hi) in enumerate(ranges)]
                passed=all(m is None or (m['rmse']<=3 and m['p99_absolute']<=12) for m in metrics)
                displays.append({'scale':scale,'stretch':name,'bands':metrics,'historical_all_pixels':populations['all_pixels'],'source_any_valid':populations['any_valid'],'source_all_valid':populations['all_valid'],'source_partial_valid':populations['partial_valid'],'gate_population':'source_any_valid' if valid_gate else 'historical_all_pixels','clipping':clipping,'passed':passed})
                source.save_image(output/f'view-{i}-scale{scale}-{name}-decoded.png',dec,ranges)
        records.append({'view':v,'source_errors':band_errors,'display':displays,'reference':reference,'raw_sha256':source.digest(raw)})
    source.write_json(output/'quality.json',{'status':'completed','passed':all(d['passed'] for r in records for d in r['display']),'numerical_only':True,'independent_decoding':'separate evidence required','agent_visual_inspection':'separate opened-image assessment required','human_acceptance':'not performed','asset':args.asset,'bpp':manifest['identity']['profile']['bits_per_pixel'],'records':records})


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['store','tool','views','representation']:parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--source-valid-gate', action='store_true', help='Gate original-source ANY-valid cells; retain historical all-pixel metrics')
    parser.add_argument('--asset',required=True);parser.add_argument('--output-name',required=True)
    evaluate(parser.parse_args())
