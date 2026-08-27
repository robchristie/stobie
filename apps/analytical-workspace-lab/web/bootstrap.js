import init, { WebHandle } from './pkg/analytical_workspace_lab.js';

async function start() {
  await init();
  const canvas = document.getElementById('polyorama-canvas');
  const handle = new WebHandle();
  await handle.start(canvas);
  window.__POLYORAMA_HANDLE = handle;
  document.body.classList.add('ready');
}

start().catch((error) => {
  console.error('Polyorama initialisation failed', error);
  document.getElementById('loading').textContent = `Initialisation failed: ${error}`;
});
