#!/usr/bin/env python3
"""Compare installed independent decoder output with bounded same-codec views."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess

import numpy as np
from osgeo import gdal

spec=importlib.util.spec_from_file_location('source',Path(__file__).with_name('viewer-real-scene-source.py'))
source=importlib.util.module_from_spec(spec);spec.loader.exec_module(source)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['store','decoder','quality']:parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--output-name',required=True)
    args=parser.parse_args()
    inp=json.loads((args.quality/'input.json').read_text());a=inp['asset']
    out=source.fresh_group(args.store,args.output_name,'Installed independent Kakadu decoder regional TIFFs of immutable indexed HT payloads; comparison with same-codec native U16LE samples. No external decoder source inspected.')
    env=os.environ.copy();env['LD_LIBRARY_PATH']='/opt/kakadu/lib:'+env.get('LD_LIBRARY_PATH','')
    version=subprocess.run([str(args.decoder),'-version'],env=env,text=True,capture_output=True)
    source.write_json(out/'decoder.json',{'path':str(args.decoder),'sha256':source.digest(args.decoder),'version':version.stdout+version.stderr})
    records=[];gdal.UseExceptions();gdal.SetConfigOption('GDAL_PAM_ENABLED','NO')
    for i,v in enumerate(a['views']):
        height,width=a['image']['height'],a['image']['width']
        region=f"{{{v['y']/height:.17g},{v['x']/width:.17g}}},{{{v['height']/height:.17g},{v['width']/width:.17g}}}"
        command=[str(args.decoder),'-i',str(Path(inp['representation'])/'payload.j2c'),'-o',str(out/f'view-{i}.tif'),'-region',region,'-precise','-fprec',str(a['image']['precision'])+'L','-num_threads','1']
        r=subprocess.run(command,env=env,text=True,capture_output=True,timeout=120)
        (out/f'view-{i}.log').write_text(r.stdout+r.stderr)
        record={'command':command,'exit':r.returncode,'view':v}
        if r.returncode==0:
            ds=gdal.Open(str(out/f'view-{i}.tif'))
            record['geometry']=[ds.RasterXSize,ds.RasterYSize,ds.RasterCount]
            if record['geometry']==[v['width'],v['height'],len(a['bands'])]:
                independent=np.stack([ds.GetRasterBand(b+1).ReadAsArray() for b in range(ds.RasterCount)])
                same=np.fromfile(args.quality/f'view-{i}.u16le',dtype='<u2').reshape(v['height'],v['width'],len(a['bands'])).transpose(2,0,1)
                delta=independent.astype(np.int64)-same.astype(np.int64)
                record['per_band']=[{'source_band':band,'mismatched_samples':int(np.count_nonzero(delta[c])),'max_absolute':int(abs(delta[c]).max()),'rmse':float(np.sqrt(np.mean(delta[c].astype(float)**2)))} for c,band in enumerate(a['bands'])]
                record['exact']=not np.any(delta)
                record['tiff_sha256']=source.digest(out/f'view-{i}.tif')
            else:record['status']='region rounding geometry mismatch; no aligned comparison claimed'
        records.append(record)
    source.write_json(out/'independent.json',{'asset':a['id'],'payload_sha256':inp['manifest']['identity']['payload_sha256'],'method':'installed Kakadu precise reconstruction, explicit original precision LSB alignment, region TIFF compared with same-codec U16LE. Independent agreement is separate from source/display fidelity.','records':records,'all_exact':all(r.get('exact',False) for r in records)})


if __name__=='__main__':main()
