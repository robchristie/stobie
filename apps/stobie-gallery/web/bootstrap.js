import { startApplication } from './browser-startup.js';

await startApplication({
  app: 'stobie-gallery',
  load: () => import('./pkg/stobie_gallery.js'),
  canvasId: 'stobie-gallery-canvas',
  handleName: '__STOBIE_GALLERY_HANDLE',
  start: (handle, canvas) => handle.start(canvas),
});
