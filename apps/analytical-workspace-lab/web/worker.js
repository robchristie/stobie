import init, { decode_request } from './worker-pkg/polyorama_tile_worker.js';

const ready = init();
self.onmessage = async (message) => {
  try {
    await ready;
    self.postMessage(decode_request(message.data));
  } catch (error) {
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
