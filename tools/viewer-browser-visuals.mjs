// Open authored synthetic small-object, faint-detail and boundary views.
import {chromium} from 'playwright';
import {mkdir,writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {createHash} from 'node:crypto';
const [url,output]=process.argv.slice(2);
if(!url||!output)throw new Error('usage: viewer-browser-visuals.mjs URL NEW-OUTPUT');
await mkdir(output,{recursive:false});
const catalogue=await(await fetch(url+'/catalogue')).json();
const browser=await chromium.launch({headless:true,args:['--no-sandbox','--enable-unsafe-webgpu','--use-angle=vulkan','--enable-features=Vulkan,CDPScreenshotNewSurface','--disable-vulkan-surface','--disable-dev-shm-usage']});
let releaseGate;
const page=await browser.newPage({viewport:{width:1440,height:900}}),states=[];
const record={catalogue,states,pattern:'source-specific authorship and signal recipe belong to the fixture provenance; catalogue records immutable source and representation identities',interpretation:'appearance only; numerical same-representation checksums and codec quality evidence are independent'};
const settled=()=>page.waitForFunction(()=>{const s=window.emuellaViewer?.snapshot();return s&&s.desired>0&&s.in_flight===0&&s.ready_demands===s.desired;},null,{timeout:60000});
const action=async intent=>{await page.evaluate(intent=>window.emuellaViewer.intent(intent),intent);await page.waitForTimeout(100);await settled();};
const capture=async label=>{const snapshot=await page.evaluate(()=>window.emuellaViewer.snapshot());if(snapshot.errors.length)throw new Error(JSON.stringify(snapshot.errors));const bytes=await page.screenshot({path:join(output,label+'.png')});states.push({label,snapshot,image_sha256:createHash('sha256').update(bytes).digest('hex')});await writeFile(join(output,'visuals.json'),JSON.stringify(record,null,2));};
try {
 await page.goto(url);await settled();await capture('large-overview');
 const width=catalogue[0].identity.profile.width;
 // Hold a real additional-detail response so the cached coarse fallback is observable.
 const gate=new Promise(resolve=>{releaseGate=resolve;});
 await page.context().route('**/jpip?*',async route=>{try {const response=await route.fetch();await gate;await route.fulfill({response});} catch(error){record.route_error=String(error);}});
 await page.evaluate(factor=>window.emuellaViewer.intent({kind:'zoom',factor}),2048/width);
 await page.waitForFunction(()=>{const s=window.emuellaViewer.snapshot();return s.in_flight>0&&s.primary_ready>0&&s.primary_ready<s.primary_desired;},null,{timeout:60000});
 await capture('coarse-fallback-detail-pending');
 releaseGate();await settled();await page.context().unroute('**/jpip?*');
 if(record.route_error)throw new Error(record.route_error);
 await capture('reduced-centre-boundaries');
 await action({kind:'zoom',factor:0.25});
 await capture('full-detail-small-objects');
 const before=await page.evaluate(()=>window.emuellaViewer.snapshot());
 await action({kind:'stretch',low:550,high:850,gamma:0.5});
 await capture('full-detail-aggressive-stretch');
 const after=await page.evaluate(()=>window.emuellaViewer.snapshot());
 if(before.worker.received_jpp_bytes!==after.worker.received_jpp_bytes||before.worker.decode_count!==after.worker.decode_count)throw new Error('display-only stretch caused source work');
 record.completed=true;
} catch(error){record.completed=false;record.error=String(error);throw error;}
finally {if(releaseGate)releaseGate();await writeFile(join(output,'visuals.json'),JSON.stringify(record,null,2));await browser.close();}
