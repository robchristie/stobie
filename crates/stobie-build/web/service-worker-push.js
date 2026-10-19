
// Web Push, because the page declares `<meta name="stobie-web-push"
// content="enabled">`. Source: crates/stobie-build/web/service-worker-push.js.
//
// Every push shows a notification: browsers that require user-visible push,
// such as Safari, revoke the permission of a site whose push shows none. The
// payload is a declarative push message, {"web_push": 8030, "notification":
// {"title", "body", "navigate", "tag", "lang", "dir", "app_badge"}}, or its
// `notification` object alone. A payload that is not such an object, or has
// no title, is titled with the application's name; one without a route opens
// the application. A tap focuses an open window of
// this scope and posts it the route, or remembers the route and opens it, so
// the application reads it at launch even if the window opens elsewhere.
const NAME = __STOBIE_NAME__;
// Where a tapped notification's route waits for a window this worker opens.
const ROUTE_CACHE = 'stobie-notification-route';
const ROUTE_KEY = new URL('__stobie/notification-route', SCOPE).href;

// The URL a notification opens: `navigate` resolved against the scope, or
// the scope itself when it is missing or outside the scope.
function routeOf(navigate) {
  if (typeof navigate !== 'string' || !navigate.trim()) return SCOPE.href;
  try {
    const url = new URL(navigate, SCOPE);
    if (url.origin === SCOPE.origin && url.pathname.startsWith(SCOPE.pathname)) return url.href;
  } catch { /* Not a URL. */ }
  return SCOPE.href;
}

// A badge count from a number or a decimal string, or undefined.
function badgeOf(value) {
  const count = typeof value === 'string' && /^\d+$/.test(value.trim()) ? Number(value) : value;
  return Number.isSafeInteger(count) && count >= 0 ? count : undefined;
}

const text = value => (typeof value === 'string' ? value : undefined);

// The notification for a push's data, which may be absent or not JSON.
function notificationFor(data) {
  let message;
  try { message = data?.json(); } catch { /* Not JSON. */ }
  const declared = message?.web_push === 8030 ? message.notification : message;
  const fields = declared && typeof declared === 'object' && !Array.isArray(declared) ? declared : {};
  const title = text(fields.title)?.trim() || NAME;
  const options = { data: { route: routeOf(fields.navigate) } };
  for (const name of ['body', 'tag', 'lang']) {
    if (text(fields[name])) options[name] = fields[name];
  }
  if (['auto', 'ltr', 'rtl'].includes(fields.dir)) options.dir = fields.dir;
  return { title, options, badge: badgeOf(fields.app_badge ?? message?.app_badge) };
}

async function setBadge(count) {
  if (count === undefined) return;
  try {
    if (count === 0) await self.navigator.clearAppBadge?.();
    else await self.navigator.setAppBadge?.(count);
  } catch { /* Badges are optional. */ }
}

async function showPush(data) {
  const { title, options, badge } = notificationFor(data);
  await Promise.all([self.registration.showNotification(title, options), setBadge(badge)]);
}

async function rememberRoute(route) {
  try {
    const cache = await caches.open(ROUTE_CACHE);
    await cache.put(ROUTE_KEY, new Response(JSON.stringify({ route, at: Date.now() }), {
      headers: { 'Content-Type': 'application/json' },
    }));
  } catch { /* The window still opens at the route. */ }
}

// Focus an open window of this scope and post it the route; otherwise
// remember the route and open a window at it.
async function openRoute(route) {
  const windows = await self.clients.matchAll({ type: 'window', includeUncontrolled: true });
  const open = windows.find(client => client.url.startsWith(SCOPE.href));
  if (open) {
    let target = open;
    try { target = (await open.focus()) ?? open; } catch { /* Not focusable; it still hears. */ }
    target.postMessage({ stobie: 'notification', route });
    return;
  }
  await rememberRoute(route);
  await self.clients.openWindow(route);
}

self.addEventListener('push', event => {
  event.waitUntil(showPush(event.data));
});

self.addEventListener('notificationclick', event => {
  event.notification.close();
  event.waitUntil(openRoute(routeOf(event.notification.data?.route)));
});
