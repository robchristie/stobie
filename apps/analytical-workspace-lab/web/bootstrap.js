import { startApplication } from './browser-startup.js';

await startApplication({
  app: 'analytical-workspace-lab',
  workerUrl: new URL('./worker.js', import.meta.url).href,
  load: () => import('./pkg/analytical_workspace_lab.js'),
  canvasId: 'polyorama-canvas',
  handleName: '__POLYORAMA_HANDLE',
  start: (handle, canvas) => handle.start(canvas),
});
