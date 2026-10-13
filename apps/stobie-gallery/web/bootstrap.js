import { startApplication } from './browser-startup.js';

await startApplication({
  app: 'stobie-gallery',
  load: () => import('./pkg/stobie_gallery.js'),
  canvasId: 'stobie-gallery-canvas',
  handleName: '__STOBIE_GALLERY_HANDLE',
  start: (handle, canvas) => handle.start(canvas),
  afterStart: handle => {
    // The fixed, versioned automation dispatcher is enabled only on request.
    if (new URL(location.href).searchParams.get('automation') === '1') {
      handle.enable_automation(crypto.randomUUID());
      window.__STOBIE_AUTOMATION = Object.freeze({
        request: json => handle.automation_request(json),
      });
    }
  },
});
