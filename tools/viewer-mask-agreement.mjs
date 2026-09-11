// Authored-mask fixture only: actual browser worker versus native regional evidence.
import { chromium } from 'playwright';
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const [url, nativePath, output] = process.argv.slice(2);
if (!url || !nativePath || !output) throw new Error('usage: node tools/viewer-mask-agreement.mjs URL AUTHORED_NATIVE_REGIONS OUTPUT');
const cases = JSON.parse(await readFile(nativePath, 'utf8'));
const browser = await chromium.launch({headless:true,args:['--no-sandbox']});
try {
  const page = await browser.newPage();
  await page.goto(`${url}/manifest/authored`);
  const result = await page.evaluate(async cases => {
    const worker = new Worker('/worker.js', {type:'module'});
    const next = () => new Promise((resolve,reject) => {
      const timeout = setTimeout(() => reject(new Error('worker timeout')),30000);
      worker.onmessage = ({data}) => {clearTimeout(timeout);resolve(data);};
      worker.onerror = e => {clearTimeout(timeout);reject(new Error(e.message));};
    });
    try {
      let waiting = next(); worker.postMessage({kind:'init',server:location.origin,compressed:64<<20});
      const catalogue = await waiting;
      if (!catalogue.Catalogue) throw new Error(JSON.stringify(catalogue));
      const manifest = catalogue.Catalogue.find(m=>m.target==='authored');
      const evidence=[];
      for (const [i,c] of cases.entries()) {
        const request={key:{representation:Array(32).fill(0),region:{x:c.region.x,y:c.region.y,width:c.region.width,height:c.region.height},reduction:c.region.discard,components:c.region.components,stage:1},token:{source_generation:0,demand_epoch:1,sequence:i},max_decoded_bytes:1024*1024};
        waiting=next();worker.postMessage({kind:'job',job:{request,manifest}});
        const event=await waiting;
        if (!event.Completed) throw new Error(JSON.stringify(event));
        const {pixels,metrics}=event.Completed;
        let hash=0xcbf29ce484222325n;
        for (const sample of pixels.samples) for (const byte of [sample&255,sample>>8]) hash=BigInt.asUintN(64,(hash^BigInt(byte))*0x100000001b3n);
        evidence.push({width:pixels.width,height:pixels.height,validity:Array.from(pixels.validity),fnv1a64_u16le:hash.toString(16).padStart(16,'0'),mask_bytes:metrics.mask_bytes,received_mask_bytes:metrics.received_mask_bytes,compressed_bytes:metrics.compressed_bytes});
      }
      return evidence;
    } finally {worker.terminate();}
  },cases);
  for (const [i,c] of cases.entries()) {
    assert.equal(result[i].width,c.width);assert.equal(result[i].height,c.height);
    assert.deepEqual(result[i].validity,c.validity);assert.equal(result[i].fnv1a64_u16le,c.fnv1a64_u16le);
    assert.ok(result[i].received_mask_bytes>0);
  }
  await writeFile(output,JSON.stringify({schema:'authored-mask-native-browser-agreement-v1',passed:true,cases:result.length,evidence:result.map(({validity,...row})=>({...row,valid_pixels:validity.reduce((a,b)=>a+b,0),invalid_pixels:validity.filter(v=>!v).length}))},null,2)+'\n');
  console.log(`PASS: ${result.length} authored native/browser regions, masks and unchanged samples agree`);
} finally {await browser.close();}
