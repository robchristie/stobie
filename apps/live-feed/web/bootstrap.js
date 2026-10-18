import { startApplication } from './browser-startup.js';

// The application opens its own WebSocket to `?feed=<url>`, or to `feed`
// beside this page; the page only starts it.
await startApplication({
  app: 'live-feed',
  load: () => import('./pkg/live_feed.js'),
  canvasId: 'live-feed-canvas',
  handleName: '__STOBIE_LIVE_FEED_HANDLE',
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
