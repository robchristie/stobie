import init, { WebHandle } from './pkg/emuella_viewer.js';
await init();
const params = new URLSearchParams(location.search || location.hash.slice(1));
const viewer = new WebHandle();
await viewer.start(document.querySelector('#viewer'), location.origin, Number(params.get('compressed_mib') || 64) << 20, Number(params.get('decoded_mib') || 16) << 20, Number(params.get('gpu_mib') || 64) << 20, params.has('script'));

if (window.__viewerWorkload) viewer.set_script_workload(JSON.stringify(window.__viewerWorkload));
window.emuellaViewer = viewer;
