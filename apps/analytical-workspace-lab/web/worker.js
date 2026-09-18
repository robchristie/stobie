import { installWorkerStartup } from './browser-startup.js';
import init, { decode_request } from './worker-pkg/polyorama_tile_worker.js';

const startup = installWorkerStartup();
startup.mark('wasm_init_begin');
const ready = init().then(wasm => {
  startup.setMemory(wasm.memory);
  startup.mark('wasm_init_end');
  startup.mark('worker_ready');
}, error => { startup.fail('wasm_init', error); throw error; });
// Attach immediately: a rejected initialisation is reported even before any job.
ready.catch(() => {});
self.onmessage = async (message) => {
  try {
    await ready;
    const result = decode_request(message.data);
    if (result.Completed) startup.mark('first_decode_complete');
    if (result.Failed) startup.fail('first_decode', result.Failed.message);
    self.postMessage(result);
  } catch (error) {
    startup.fail('preparation_decode', error);
    const request = message.data;
    self.postMessage({
      Failed: {
        key: request.key,
        token: request.token,
        preparation_ms: 0,
        decode_ms: 0,
        message: `worker preparation/decode failed: ${String(error)}`,
      },
    });
  }
};
