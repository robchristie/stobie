import init, { decode_request } from './worker-pkg/polyorama_tile_worker.js';

const ready = init();
self.onmessage = async (message) => {
  await ready;
  self.postMessage(decode_request(message.data));
};
