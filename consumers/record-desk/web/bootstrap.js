import init, { WebHandle } from './pkg/record_desk.js';

const status = document.getElementById('startup');
let handle;
try {
  if (!navigator.gpu) throw new Error('WebGPU is unavailable in this browser.');
  await init();
  handle = new WebHandle();
  await handle.start(document.getElementById('record-desk-canvas'));
  window.__RECORD_DESK = handle;
  const canvas = document.getElementById('record-desk-canvas');
  // eframe forwards keys, but browser Find/Location/Reload defaults can steal
  // focus before egui consumes application shortcuts. Reserve only our chords
  // while the canvas or eframe's sibling text input owns browser focus.
  document.addEventListener('keydown', event => {
    const appFocused = document.activeElement === canvas
      || (document.activeElement?.tagName === 'INPUT'
        && document.activeElement.parentElement === canvas.parentElement);
    if (!appFocused || event.altKey || !(event.ctrlKey || event.metaKey)) return;
    const key = event.key.toLowerCase();
    const registered = ['f', 'z', 'enter'].includes(key)
      || (!event.shiftKey && ['s', 'l'].includes(key))
      || (event.shiftKey && key === 'r');
    if (registered) event.preventDefault(); // Keep forwarding to eframe.
  }, { capture: true });
  canvas.dataset.ready = 'true';
  status.remove();
} catch (error) {
  handle?.destroy();
  status.textContent = `Record Desk could not start: ${error}`;
  console.error(error);
}
