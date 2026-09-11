#!/usr/bin/env python3
"""Run the existing full-scene GTiff preparation with exact campaign provenance."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

spec=importlib.util.spec_from_file_location('source',Path(__file__).with_name('viewer-real-scene-source.py'))
source=importlib.util.module_from_spec(spec);spec.loader.exec_module(source)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['store','tool','gdal-library','views','timing-grant']:parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--asset',required=True);parser.add_argument('--output-name',required=True)
    parser.add_argument('--bpp',type=int,choices=[1,2,4,8,12],required=True)
    parser.add_argument('--codec-revision',required=True)
    args=parser.parse_args()
    if not args.timing_grant.is_file():raise ValueError('coordinator timing grant is absent')
    a=next(a for a in json.loads(args.views.read_text())['assets'] if a['id']==args.asset)
    input_path=args.store/'source'/a['source_path']
    assert source.digest(input_path)==a['source_sha256']
    output=source.fresh_group(args.store,args.output_name,'Full-scene selected native TIFF bands encoded as tiled512/D6 indexed HT, irreversible97, no MCT, one layer; original source unchanged.')
    command=[str(args.tool),'prepare','--retain-incomplete','true','--gdal-library',str(args.gdal_library),'--input',str(input_path),'--output',str(output/'representation'),'--target',args.asset.lower()+'-'+str(args.bpp),'--bits',str(a['image']['precision']),'--bands',','.join(map(str,a['bands'])),'--tile','512','--levels','6','--bpp',str(args.bpp),'--codec-revision',args.codec_revision]
    identity={'command':command,'source':a,'tool_sha256':source.digest(args.tool),'gdal_sha256':source.digest(args.gdal_library),'viewer_revision':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'views_sha256':source.digest(args.views),'source_cache_state':'uncontrolled; wrapper hash before timing, tool hashes source within timing','environment':{k:os.environ.get(k) for k in ['LD_LIBRARY_PATH','GDAL_DRIVER_PATH','GDAL_DATA','PROJ_DATA','OMP_NUM_THREADS','OPENBLAS_NUM_THREADS']}}
    source.write_json(output/'invocation.json',identity)
    started=time.perf_counter()
    with (output/'stdout.json').open('w') as stdout,(output/'stderr.txt').open('w') as stderr:
        process=subprocess.run(command,stdout=stdout,stderr=stderr,timeout=1200)
    elapsed=time.perf_counter()-started
    result={'exit':process.returncode,'wall_seconds':elapsed,'completed':process.returncode==0,'source_after_sha256':source.digest(input_path),'physical_storage_traffic':'not measured; process and logical callback counts remain separate'}
    if process.returncode==0:
        manifest,metrics=json.loads((output/'stdout.json').read_text())
        assert manifest['identity']['profile']['width']==a['image']['width']
        assert manifest['identity']['profile']['height']==a['image']['height']
        assert source.digest(output/'representation/payload.j2c') == manifest['identity']['payload_sha256']
        for tile, expected in enumerate(manifest['descriptor_sha256']):
            assert source.digest(output/'representation/descriptors'/f'{tile}.bin') == expected
        result.update(manifest=manifest,metrics=metrics,descriptor_bytes=sum(p.stat().st_size for p in (output/'representation/descriptors').glob('*.bin')),manifest_sha256=source.digest(output/'representation/manifest.json'))
    source.write_json(output/'preparation-result.json',result)
    print(json.dumps({'output':str(output),**result}))


if __name__=='__main__':main()
