import init, { WebHandle } from './pkg/polyorama_gallery.js';

async function start() {
  await init();
  const canvas = document.getElementById('polyorama-gallery-canvas');
  const handle = new WebHandle();
  await handle.start(canvas);
  window.__POLYORAMA_GALLERY_HANDLE = handle;
  document.body.classList.add('ready');
}

start().catch((error) => {
  console.error('Polyorama gallery initialisation failed', error);
  document.getElementById('loading').textContent = `Initialisation failed: ${error}`;
});
