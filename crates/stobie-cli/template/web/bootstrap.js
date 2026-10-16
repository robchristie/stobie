// `stobie build-web` writes browser-startup.js and pkg/ beside this file.
import { startApplication } from './browser-startup.js';

await startApplication({
  app: '{{name}}',
  load: () => import('./pkg/{{crate}}.js'),
  canvasId: 'app-canvas',
  handleName: '__STOBIE_HANDLE',
  start: (handle, canvas) => handle.start(canvas),
  afterStart: handle => {
    // With ?automation=1 in the URL, `stobie attach` can read and operate the page.
    if (new URL(location.href).searchParams.get('automation') === '1') {
      handle.enable_automation(crypto.randomUUID());
      window.__STOBIE_AUTOMATION = Object.freeze({
        request: json => handle.automation_request(json),
      });
    }
  },
});
