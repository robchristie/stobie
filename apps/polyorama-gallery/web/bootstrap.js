import { startApplication } from './browser-startup.js';

await startApplication({
  app: 'polyorama-gallery',
  load: () => import('./pkg/polyorama_gallery.js'),
  canvasId: 'polyorama-gallery-canvas',
  handleName: '__POLYORAMA_GALLERY_HANDLE',
  start: (handle, canvas) => handle.start(canvas),
});
