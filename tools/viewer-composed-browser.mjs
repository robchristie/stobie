// The native and browser executables consume the same checked-in workload.
import { chromium } from 'playwright';
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
const [url, output] = process.argv.slice(2);
if (!url || !output) throw new Error('usage: viewer-composed-browser.mjs URL EXISTING-OUTPUT');
const browser = await chromium.launch({headless:true,args:['--no-sandbox','--enable-unsafe-webgpu','--use-angle=vulkan','--enable-features=Vulkan,CDPScreenshotNewSurface','--disable-vulkan-surface','--disable-dev-shm-usage']});
const page = await browser.newPage({viewport:{width:1440,height:900}});
const samples=[], errors=[], transfers=[];
let workers=0;
page.on('worker',()=>workers++);
page.on('pageerror',error=>errors.push(String(error)));
page.on('console',message=>{if(message.type()==='error')errors.push(message.text());});
const client=await page.context().newCDPSession(page);
await client.send('Network.enable');
client.on('Network.loadingFinished',e=>transfers.push({request_id:e.requestId,encoded_data_length:e.encodedDataLength,timestamp:e.timestamp}));
await page.addInitScript(()=>{
  window.__viewerGpu=[];
  const original=GPUAdapter.prototype.requestDevice;
  GPUAdapter.prototype.requestDevice=async function(...args){
    const device=await original.apply(this,args);
    window.__viewerGpu.push({vendor:this.info.vendor,architecture:this.info.architecture,device:this.info.device,description:this.info.description});
    device.lost.then(info=>{if(info.reason!=='destroyed')console.error('WebGPU device lost',info.reason,info.message);});
    return device;
  };
});
let timer;
try {
  await page.goto(url+'/#script');
  await page.waitForFunction(()=>window.emuellaViewer,null,{timeout:60000});
  timer=setInterval(async()=>{try{samples.push(await page.evaluate(()=>window.emuellaViewer.snapshot()));}catch{}},100);
  await page.waitForFunction(()=>window.emuellaViewer.snapshot().script_complete,null,{timeout:600000});
  await page.screenshot({path:join(output,'browser-final.png')});
  const final=await page.evaluate(()=>window.emuellaViewer.snapshot());
  const stages=await page.evaluate(()=>window.emuellaViewer.stages());
  await writeFile(join(output,'app.json'),JSON.stringify(final,null,2));
  await writeFile(join(output,'app.json.stages.json'),JSON.stringify(stages,null,2));
  await writeFile(join(output,'browser.json'),JSON.stringify({adapters:await page.evaluate(()=>window.__viewerGpu),workers,errors,transfers,samples,browser_version:browser.version()},null,2));
  if(errors.length || final.errors.length || workers!==1)throw new Error('browser/worker failure; see raw evidence');
} catch(error) {
  await writeFile(join(output,'browser-failure.json'),JSON.stringify({error:String(error),errors,workers,transfers,samples},null,2));
  throw error;
} finally {clearInterval(timer);await browser.close();}
