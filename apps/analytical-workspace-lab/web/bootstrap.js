import { startApplication } from './browser-startup.js';

await startApplication({
  app: 'analytical-workspace-lab',
  workerUrl: new URL('./worker.js', import.meta.url).href,
  load: () => import('./pkg/analytical_workspace_lab.js'),
  canvasId: 'polyorama-canvas',
  handleName: '__POLYORAMA_HANDLE',
  start: async (handle, canvas) => {
    await handle.start(canvas);
    if (new URL(location.href).searchParams.get('automation') === '1') {
      handle.enable_automation(crypto.randomUUID());
      window.__POLYORAMA_AUTOMATION = Object.freeze({
        request: json => handle.automation_request(json),
      });
    }
  },
});
