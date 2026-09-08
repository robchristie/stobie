import { chromium } from 'playwright';
import {createServer, request as httpRequest} from 'node:http';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { hostedLinuxWebGpuLaunchOptions } from './browser-launch.mjs';
const url = process.env.EMUELLA_VIEWER_URL || 'http://127.0.0.1:8088';
const output = process.env.POLYORAMA_EVIDENCE_DIR;
if (!output) throw new Error('POLYORAMA_EVIDENCE_DIR must name registered campaign scratch');
await mkdir(output, { recursive: true });
const options = process.env.EMUELLA_BROWSER_GPU === "hardware" ? { headless: true, args: ["--no-sandbox", "--enable-unsafe-webgpu", "--use-angle=vulkan", "--enable-features=Vulkan,CDPScreenshotNewSurface", "--disable-vulkan-surface"] } : hostedLinuxWebGpuLaunchOptions();
options.args.push('--remote-debugging-port=9229', '--disable-dev-shm-usage');
const browser = await chromium.launch(options);
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
const errors = [], requests = [], states = [];
await page.addInitScript(() => {
  window.__viewerGpu = [];
  const original = GPUAdapter.prototype.requestDevice;
  GPUAdapter.prototype.requestDevice = async function(...args) {
    const device = await original.apply(this, args);
    window.__viewerGpu.push({info: {vendor:this.info.vendor,architecture:this.info.architecture,device:this.info.device,description:this.info.description}, limits: {maxBufferSize: device.limits.maxBufferSize}});
    device.addEventListener('uncapturederror', event => console.error('WebGPU validation', event.error.message));
    device.lost.then(info => { if (info.reason !== 'destroyed') console.error('WebGPU device lost', info.reason, info.message); });
    return device;
  };
});
let workers = 0;
let delayProxy;
page.on('worker', () => workers++);
page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
page.on('pageerror', e => errors.push(String(e)));
page.on('requestfailed', request => { if (!request.failure()?.errorText.includes('ABORTED')) errors.push(request.failure()?.errorText); });
page.on('response', response => { if (response.url().includes('/jpip?')) requests.push({ url: response.url(), status: response.status() }); });
try {
  await page.goto(url);
  await page.waitForFunction(() => window.emuellaViewer?.snapshot().completed > 2, null, { timeout: 120000 });
  const snapshot = () => page.evaluate(() => window.emuellaViewer.snapshot());
  const action = async (intent, label) => {
    const before=await snapshot(); const started=performance.now();
    await page.evaluate(intent => window.emuellaViewer.intent(intent), intent);
    await page.waitForFunction(generation => {const s=window.emuellaViewer.snapshot();return s.generation>generation && s.in_flight===0;}, before.generation, { timeout: 120000 });
    states.push({ label, action_to_settled_ms:performance.now()-started, started_at_ms:started, snapshot: await snapshot() });
  };
  states.push({ label: 'cold', snapshot: await snapshot() });
  await action({ kind: 'action', action: 'bookmark' }, 'bookmark');
  await action({ kind: 'zoom', factor: 0.25 }, 'zoom');
  await action({ kind: 'pan', dx: 0.3, dy: 0.2 }, 'pan');
  await action({ kind: 'gallery', row: 1200 }, 'clustered-scattered-gallery');
  await action({ kind: 'open_detection', index: 2401 }, 'detection-detail');
  const beforeStretch = await snapshot();
  await action({ kind: 'stretch', low: 30, high: 1700, gamma: 1.4 }, 'stretch');
  const afterStretch = await snapshot();
  if (beforeStretch.worker.received_jpp_bytes !== afterStretch.worker.received_jpp_bytes || beforeStretch.worker.decode_count !== afterStretch.worker.decode_count) throw new Error('stretch invalidated source work');
  await action({ kind: 'action', action: 'next_image' }, 'next-image');
  await action({ kind: 'gallery', row: 4000 }, 'scattered-gallery');
  await action({ kind: 'open_detection', index: 8001 }, 'rgb-detail');
  await action({ kind: 'action', action: 'bookmark' }, 'second-image-bookmark');
  await action({ kind: 'action', action: 'recall' }, 'bookmark-revisit');
  await action({ kind: 'action', action: 'compare_image' }, 'simultaneous-images');
  const comparison=await snapshot();
  if (comparison.comparison_image == null || comparison.comparison_image === comparison.image || comparison.bookmarks < 2) throw new Error('multi-image comparison/bookmarks absent');
  // Start work, then change image before its completion to exercise abort acknowledgement.
  await page.evaluate(() => { window.emuellaViewer.intent({ kind: 'gallery', row: 2200 }); });
  await page.waitForTimeout(30);
  await action({ kind: 'action', action: 'next_image' }, 'abort-image-switch');
  await action({ kind: 'action', action: 'retry' }, 'retry');
  await action({ kind: 'action', action: 'compare_image' }, 'final-comparison');
  await page.screenshot({ path: join(output, 'viewer-browser-desktop.png') });
  await page.setViewportSize({ width: 1024, height: 720 });
  await page.waitForTimeout(1000);
  await page.screenshot({ path: join(output, 'viewer-browser-narrow.png') });
  const final = await snapshot();
  if (workers !== 1 || final.completed === 0 || final.rendered_regions === 0) throw new Error('real Worker or rendered output absent');
  for (const { snapshot: s } of states) {
    if (s.decoded_peak_bytes > 16 << 20 || s.gpu_peak_bytes > 64 << 20 || s.worker.compressed_bytes > 64 << 20 || s.worker.descriptor_bytes > 16 << 20 || s.worker.peak_compressed_bytes > 64 << 20 || s.worker.peak_descriptor_bytes > 16 << 20 || s.materialised_detections > 64 || s.errors.length) throw new Error(`viewer bound or worker failure: ${JSON.stringify(s)}`);
  }
  if (errors.length) throw new Error(JSON.stringify(errors));
  await writeFile(join(output, 'viewer-browser.json'), JSON.stringify({ graphics: process.env.EMUELLA_BROWSER_GPU === 'hardware' ? 'explicit Vulkan hardware request; see adapter evidence' : 'software SwiftShader WebGPU; no target performance claim', adapters: await page.evaluate(() => window.__viewerGpu), workers, states, final, errors, requests }, null, 2));
  // A fresh constrained client and delayed real JPP request prove abort acknowledgement.
  const constrained = await browser.newContext({viewport:{width:1440,height:900}});
  let intercepted=0, interrupted=0;
  const transfers=[];
  const proxy=delayProxy=createServer((incoming,outgoing)=> {
    const transfer={url:incoming.url,started_ms:performance.now()};
    if(incoming.url.startsWith('/jpip?'))transfers.push(transfer);
    const upstream=httpRequest(new URL(incoming.url,url),{method:'GET'}, response=> {
      transfer.response_received_ms=performance.now();
      const deliver=()=>{if(!outgoing.destroyed){outgoing.writeHead(response.statusCode,response.headers);response.pipe(outgoing);}else response.destroy();};
      if(incoming.url.startsWith('/jpip?')) {intercepted++;setTimeout(deliver,400);} else deliver();
    });
    upstream.on('error',error=>{if(!outgoing.destroyed){outgoing.writeHead(502);outgoing.end(String(error));}});
    outgoing.on('close',()=>{if(!outgoing.writableEnded){if(incoming.url.startsWith('/jpip?')){interrupted++;transfer.aborted_ms=performance.now();}upstream.destroy();}});
    upstream.end();
  });
  await new Promise(resolve=>proxy.listen(0,'127.0.0.1',resolve));
  const pressureUrl='http://127.0.0.1:'+proxy.address().port;
  const pressure=await constrained.newPage();
  await pressure.goto(pressureUrl+'/#compressed_mib=1&decoded_mib=4&gpu_mib=16');
  pressure.on('pageerror',error=>writeFile(join(output,'viewer-pressure-pageerror.txt'),String(error)));
  await pressure.waitForFunction(()=>window.emuellaViewer?.snapshot().in_flight>0,null,{timeout:30000});
  await pressure.waitForTimeout(50);
  await pressure.evaluate(()=>window.emuellaViewer.intent({kind:'action',action:'next_image'}));
  await pressure.waitForFunction(()=>window.emuellaViewer.snapshot().worker.aborted>0 && window.emuellaViewer.snapshot().completed>5,null,{timeout:60000});
  const cancellationSnapshot=await pressure.evaluate(()=>window.emuellaViewer.snapshot());
  for (let i=0;i<3;i++) {
    await pressure.evaluate(()=>window.emuellaViewer.intent({kind:'action',action:'next_image'}));
    await pressure.waitForTimeout(500);
    await pressure.waitForFunction(()=>window.emuellaViewer.snapshot().in_flight===0,null,{timeout:60000});
  }
  await pressure.evaluate(()=>window.emuellaViewer.intent({kind:'open_detection',index:2401}));
  await pressure.waitForTimeout(500);
  await pressure.waitForFunction(()=>window.emuellaViewer.snapshot().in_flight===0,null,{timeout:60000});
  await pressure.evaluate(()=>window.emuellaViewer.intent({kind:'open_detection',index:8001}));
  await pressure.waitForTimeout(500);
  await pressure.waitForFunction(()=>window.emuellaViewer.snapshot().in_flight===0,null,{timeout:60000});
  for(let row=200;row<4800;row+=400) {
    const generation=await pressure.evaluate(row=>{const g=window.emuellaViewer.snapshot().generation;window.emuellaViewer.intent({kind:'gallery',row});return g;},row);
    await pressure.waitForFunction(g=>{const s=window.emuellaViewer.snapshot();return s.generation>g&&s.in_flight===0;},generation,{timeout:60000});
  }
  const pressureSnapshot=await pressure.evaluate(()=>window.emuellaViewer.snapshot());
  await writeFile(join(output,'viewer-browser-pressure.json'),JSON.stringify({intercepted,interrupted,transfers,cancellation_snapshot:cancellationSnapshot,snapshot:pressureSnapshot},null,2));
  if (!intercepted || !pressureSnapshot.worker.aborted || !pressureSnapshot.gpu_evictions || !pressureSnapshot.worker.representation_evictions || pressureSnapshot.worker.compressed_bytes>1<<20 || pressureSnapshot.worker.peak_compressed_bytes>1<<20 || pressureSnapshot.worker.peak_descriptor_bytes>16<<20 || pressureSnapshot.decoded_peak_bytes>4<<20 || pressureSnapshot.gpu_peak_bytes>16<<20 || pressureSnapshot.errors.length) throw new Error('constrained/abort workload failed; see viewer-browser-pressure.json');
  await constrained.close();
  proxy.closeAllConnections(); await new Promise(resolve=>proxy.close(resolve));

  if (process.env.EMUELLA_INSPECTION_PAUSE) await page.waitForTimeout(Number(process.env.EMUELLA_INSPECTION_PAUSE));
} catch (error) {
  await writeFile(join(output, 'viewer-browser-failure.json'), JSON.stringify({ error: String(error), workers, errors, states, snapshot: await page.evaluate(() => window.emuellaViewer?.snapshot()).catch(String) }, null, 2));
  throw error;
} finally { await browser.close(); if(delayProxy){delayProxy.closeAllConnections();delayProxy.close();} }
