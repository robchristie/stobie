import { startApplication } from './browser-startup.js';

const params = new URLSearchParams(location.search || location.hash.slice(1));
await startApplication({
  app: 'emuella-viewer',
  workerUrl: new URL('./worker.js', import.meta.url).href,
  load: () => import('./pkg/emuella_viewer.js'),
  canvasId: 'viewer',
  handleName: 'emuellaViewer',
  start: (viewer, canvas) => viewer.start(canvas, location.origin,
    Number(params.get('compressed_mib') || 64) << 20,
    Number(params.get('decoded_mib') || 16) << 20,
    Number(params.get('gpu_mib') || 64) << 20, params.has('script')),
  afterStart: viewer => {
    if (window.__viewerWorkload) viewer.set_script_workload(JSON.stringify(window.__viewerWorkload));
  },
});
