// Actual browser transport interruption and recovery; no service restart is implied.
import {chromium} from 'playwright';
import {createServer, request as httpRequest} from 'node:http';
import {writeFile} from 'node:fs/promises';
import {join} from 'node:path';
const [upstreamUrl, output] = process.argv.slice(2);
const started=performance.now(), states=[], events=[], transfers=[], errors=[], adapters=[];
const pageIdentities=new WeakMap();
let mode='truncate-once', browser, browser_version, proxy, activePage, workers=0;
const workers_boundary='total Playwright Worker creation events across sequential fresh browser contexts; not concurrent workers';
const at=()=>performance.now()-started;
const catalogue=await (await fetch(upstreamUrl+'/catalogue')).json();
const record=(kind,snapshot,detail,source=catalogue[snapshot.image]?.target??catalogue[0].target)=>events.push({at_ms:at(),kind,consumer:'browser recovery',source,generation:snapshot.generation,detail});
proxy=createServer((incoming,outgoing)=>{
  const isJpp=incoming.url.startsWith('/jpip?');
  const t={url:incoming.url,started_ms:at(),mode};
  if(isJpp)transfers.push(t);
  if(isJpp&&mode==='offline') {t.connection_destroyed_ms=at();outgoing.destroy();return;}
  const request=httpRequest(new URL(incoming.url,upstreamUrl),response=>{
    if(isJpp&&mode==='truncate-once') {
      mode='online';const chunks=[];
      response.on('data',chunk=>chunks.push(chunk));
      response.on('end',()=>{const bytes=Buffer.concat(chunks);const part=bytes.subarray(0,Math.max(1,Math.floor(bytes.length/2)));outgoing.writeHead(response.statusCode,response.headers);outgoing.write(part);t.original_body_bytes=bytes.length;t.forwarded_body_bytes=part.length;setTimeout(()=>{t.connection_destroyed_ms=at();outgoing.destroy();},30);});
    } else {
      const deliver=()=>{if(outgoing.destroyed){response.destroy();return;}outgoing.writeHead(response.statusCode,response.headers);response.pipe(outgoing);};
      if(isJpp&&mode==='delay') {t.delayed_ms=at();setTimeout(deliver,400);}else deliver();
    }
  });
  request.on('error',error=>{t.error=String(error);outgoing.destroy();});
  outgoing.on('close',()=>{if(!outgoing.writableEnded){t.client_closed_ms=at();request.destroy();}});
  request.end();
});
await new Promise(resolve=>proxy.listen(0,'127.0.0.1',resolve));
const url='http://127.0.0.1:'+proxy.address().port;
const snapshot=page=>page.evaluate(()=>window.emuellaViewer.snapshot());
const capture=async(page,label)=>{for(const adapter of await page.evaluate(()=>window.__viewerGpu??[])){if(!adapters.some(a=>JSON.stringify(a)===JSON.stringify(adapter)))adapters.push(adapter);}const s=await snapshot(page);states.push({...pageIdentities.get(page),label,at_ms:at(),snapshot:s});return s;};
const settled=page=>page.waitForFunction(()=>{const s=window.emuellaViewer.snapshot();return s.loaded&&s.desired>0&&s.ready_demands===s.desired&&s.in_flight===0;},null,{timeout:60000});
const observeGpu=page=>page.addInitScript(()=>{
  window.__viewerGpu=[];
  const original=GPUAdapter.prototype.requestDevice;
  GPUAdapter.prototype.requestDevice=async function(...args){
    const device=await original.apply(this,args);
    window.__viewerGpu.push({vendor:this.info.vendor,architecture:this.info.architecture,device:this.info.device,description:this.info.description});
    return device;
  };
});
const newPage=async context_id=>{
  // browser.newPage creates a fresh context containing this one page.
  const page=activePage=await browser.newPage({viewport:{width:1440,height:900}});
  pageIdentities.set(page,{context_id,page_id:context_id+'-page'});
  await observeGpu(page);
  page.on('worker',()=>workers++);page.on('pageerror',e=>errors.push(String(e)));
  return page;
};
try {
  browser=await chromium.launch({headless:true,args:['--no-sandbox','--enable-unsafe-webgpu','--use-angle=vulkan','--enable-features=Vulkan,CDPScreenshotNewSurface','--disable-vulkan-surface','--disable-dev-shm-usage']});
  browser_version=browser.version();
  const page=await newPage('transport');
  await page.goto(url);
  await page.waitForFunction(()=>window.emuellaViewer?.snapshot().errors.length>0,null,{timeout:60000});
  await page.waitForFunction(()=>window.emuellaViewer.snapshot().in_flight===0,null,{timeout:60000});
  const interrupted=await capture(page,'partial-transfer-failed');
  if(!transfers.some(t=>t.forwarded_body_bytes>0&&t.forwarded_body_bytes<t.original_body_bytes))throw new Error('partial real response was not interrupted');
  record('transfer_interrupted',interrupted,'proxy wrote a strict nonempty body prefix then destroyed the actual HTTP connection; worker reported failure');
  await page.evaluate(()=>window.emuellaViewer.intent({kind:'action',action:'retry'}));
  await settled(page);
  const retried=await capture(page,'partial-transfer-retried');
  if(retried.worker.requests<=interrupted.worker.requests||retried.errors.length)throw new Error('retry did not fetch additional data and recover');
  record('retry',retried,'Retry intent followed by additional real JPP request and all current demands GPU resident');
  mode='offline';
  await page.evaluate(()=>window.emuellaViewer.intent({kind:'open_detection',index:8001}));
  await page.waitForFunction(()=>window.emuellaViewer.snapshot().errors.length>0,null,{timeout:60000});
  await page.waitForFunction(()=>window.emuellaViewer.snapshot().in_flight===0,null,{timeout:60000});
  const disconnected=await capture(page,'connection-loss');
  record('connection_lost',disconnected,'proxy destroyed actual new JPP connection; worker reported failure');
  mode='online';
  await page.evaluate(()=>window.emuellaViewer.intent({kind:'action',action:'retry'}));
  await settled(page);
  const reconnected=await capture(page,'reconnected');
  if(reconnected.worker.requests<=disconnected.worker.requests||reconnected.errors.length)throw new Error('connection restoration did not recover');
  record('reconnect',reconnected,'proxy accepted new upstream TCP connections after transport restoration; Retry completed all demands; service process was not restarted');
  await page.close();
  mode='online';
  const stale=await newPage('stale-completion');
  await stale.addInitScript(()=>{
    const RealWorker=window.Worker;
    window.Worker=class extends RealWorker {
      constructor(...args){super(...args);this.addEventListener('message',event=>{
        if(event.data.Completed&&!window.__heldCompletion&&!window.__releasedCompletion){
          event.stopImmediatePropagation();
          window.__heldCompletion={worker:this,data:event.data};
        }
      });}
    };
  });
  await stale.goto(url);
  await stale.waitForFunction(()=>window.__heldCompletion,null,{timeout:60000});
  await stale.evaluate(()=>window.emuellaViewer.intent({kind:'select_image',index:1}));
  await stale.waitForFunction(()=>window.emuellaViewer.snapshot().cancelled>0,null,{timeout:60000});
  const held=await capture(stale,'real-completion-held-during-image-change');
  if(held.in_flight!==1||held.decoded_accounted_bytes===0)throw new Error('cancelled reservation vanished before actual completion acknowledgement');
  await stale.evaluate(()=>{const held=window.__heldCompletion;window.__heldCompletion=undefined;window.__releasedCompletion=true;held.worker.dispatchEvent(new MessageEvent('message',{data:held.data}));});
  await stale.waitForFunction(()=>window.emuellaViewer.snapshot().stale>0,null,{timeout:60000});
  await settled(stale);
  const rejected=await capture(stale,'real-stale-completion-rejected');
  record('stale_rejected',rejected,'held actual Worker Completed payload delivered after image generation changed; runtime rejected it and released charged reservation; pixels were not fabricated',catalogue[0].target);
  await stale.close();
  mode='delay';
  const pressure=await newPage('cache-pressure');
  await pressure.goto(url+'/#compressed_mib=1&decoded_mib=4&gpu_mib=16');
  await pressure.waitForFunction(()=>window.emuellaViewer?.snapshot().in_flight>0,null,{timeout:60000});
  const deadline=at()+60000;
  while(!transfers.some(t=>t.mode==='delay'&&t.delayed_ms!==undefined)&&at()<deadline)await pressure.waitForTimeout(10);
  if(at()>=deadline)throw new Error('no delayed actual JPP response within 60 seconds');
  await pressure.evaluate(()=>window.emuellaViewer.intent({kind:'select_image',index:1}));
  await pressure.waitForFunction(()=>window.emuellaViewer.snapshot().worker.aborted>0,null,{timeout:60000});
  mode='online';
  await settled(pressure);
  const cancelled=await capture(pressure,'cancellation-acknowledged');
  if(!transfers.some(t=>t.mode==='delay'&&t.client_closed_ms!==undefined))throw new Error('actual delayed transfer was not closed');
  record('cancel_acknowledged',cancelled,'delayed actual JPP connection closed after image switch; worker aborted counter incremented and reservation released',catalogue[0].target);
  for(let index=2;index<=8;index++){
    const generation=(await snapshot(pressure)).generation;
    await pressure.evaluate(index=>window.emuellaViewer.intent({kind:'select_image',index}),index);
    await pressure.waitForFunction(g=>window.emuellaViewer.snapshot().generation>g,generation);
    await settled(pressure);await capture(pressure,'pressure-image-'+index);
  }
  for(let row=200;row<4800;row+=400){
    await pressure.evaluate(row=>window.emuellaViewer.intent({kind:'gallery',row}),row);
    await pressure.waitForTimeout(100);await settled(pressure);
  }
  const final=await capture(pressure,'pressure-complete');
  if(!final.worker.representation_evictions||!final.gpu_evictions||final.worker.peak_compressed_bytes>1<<20||final.decoded_peak_bytes>4<<20||final.gpu_peak_bytes>16<<20||final.errors.length)throw new Error('pressure bounds or actual eviction failed');
  record('cache_eviction',final,'shared compressed representation and GPU eviction counters incremented within 1/4/16 MiB pressure budgets');
  if(errors.length)throw new Error('unexpected browser errors');
  await pressure.screenshot({path:join(output,'browser-recovery-final.png')});
  await writeFile(join(output,'browser-recovery.json'),JSON.stringify({completed:true,started_monotonic_ms:started,browser_version,workers,workers_boundary,adapters,states,events,transfers,errors},null,2));
} catch(error) {
  if(activePage)await capture(activePage,'failure-observation').catch(()=>{});
  await writeFile(join(output,'browser-recovery.json'),JSON.stringify({completed:false,error:String(error),started_monotonic_ms:started,browser_version,workers,workers_boundary,adapters,states,events,transfers,errors},null,2));
  throw error;
} finally {if(browser)await browser.close();proxy.closeAllConnections();await new Promise(resolve=>proxy.close(resolve));}
