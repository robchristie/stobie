// Check Web Push end to end in Chromium on the production package of the
// `outside-push` application (tools/web-push-app/lib.rs), served under a
// strict Content-Security-Policy: the application subscribes when its button
// is tapped, inside the tap's input event; the subscription survives a
// reload; a push shows a notification, also for an unreadable payload; and a
// tapped notification's route reaches the open application and, through the
// route the worker remembers, a window it opens. Run inside the smoke
// environment (tools/outside-app-smoke.sh).
//
// No real push service is contacted: `PushManager.subscribe` is replaced by
// a stand-in whose endpoint is on the test server, every host name but the
// loopback address fails to resolve, and pushes are delivered through
// DevTools (`ServiceWorker.deliverPushMessage`). The worker's notification
// and badge calls are recorded as they settle. Chromium cannot click a
// system notification under automation, so the worker's own `openRoute`,
// which its `notificationclick` handler calls, delivers the routes; the
// handler itself is covered by tools/tests/service-worker-push.test.mjs.
//
// node tools/web-push-smoke.mjs APP_NAME PRODUCTION_DIR EVIDENCE_DIR
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFileSync } from 'node:fs';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { ApplicationClient, BrowserPhysicalAdapter, BrowserTransport } from './application-client.mjs';
import { createProductionServer } from './browser-serve.mjs';

const [name, productionArgument, evidenceArgument] = process.argv.slice(2);
assert(name && productionArgument && evidenceArgument, 'usage: APP_NAME PRODUCTION_DIR EVIDENCE_DIR');
const [production, evidence] = [productionArgument, evidenceArgument].map(path => resolve(path));
await mkdir(evidence, { recursive: true });
const report = { app: name, steps: [] };
const packaged = JSON.parse(readFileSync(join(production, 'manifest.json'), 'utf8')).apps.find(app => app.name === name);
assert.equal(packaged?.pwa?.webPush, true, 'the package manifest records the push worker');

// The application's test key (tools/web-push-app/lib.rs), as bytes.
const VAPID_KEY = 'BEl62iUYgUivxIkv69yViEuiBIa-Ib9-SkvMeAtA3LFgDzkrxZJjSgSnfckjBJuBkr3qBUYIHBQFLXYp5Nksh8U';
const keyBytes = [...Buffer.from(VAPID_KEY, 'base64url')];

// The policy Bokkie's gateway sends, for one origin.
const policy = origin => [
  "default-src 'none'", "script-src 'self' 'wasm-unsafe-eval'", "style-src 'self' 'unsafe-inline'", "img-src 'self'",
  `connect-src 'self' ws://${new URL(origin).host}`, "manifest-src 'self'", "worker-src 'self'", "base-uri 'self'",
  "form-action 'none'", "frame-ancestors 'none'", "object-src 'none'",
].join('; ');
const packageServer = createProductionServer({ directory: production });
const serve = packageServer.listeners('request')[0];
const server = createServer((request, response) => {
  response.setHeader('Content-Security-Policy', policy(`http://${request.headers.host}`));
  serve(request, response);
});
await new Promise(fulfil => server.listen(0, '127.0.0.1', fulfil));
const origin = `http://127.0.0.1:${server.address().port}`;
const scope = `${origin}/${name}/`;
const endpoint = `${origin}/push-service/device-1`;

// The stand-in push service: it records each subscribe call, with the event
// being dispatched at that moment, and keeps the subscription across reloads.
function pushServiceStandIn(endpoint) {
  const stored = () => JSON.parse(localStorage.getItem('push-stand-in') ?? 'null');
  const subscription = value => value && Object.assign(Object.create(null), {
    endpoint: value.endpoint, expirationTime: null,
    toJSON: () => value,
    unsubscribe: async () => { localStorage.removeItem('push-stand-in'); return true; },
  });
  window.__pushStandIn = { calls: [] };
  PushManager.prototype.subscribe = function (options) {
    window.__pushStandIn.calls.push({
      event: window.event?.type ?? null,
      userActivation: navigator.userActivation?.isActive ?? null,
      userVisibleOnly: options?.userVisibleOnly,
      key: [...new Uint8Array(options?.applicationServerKey ?? [])],
    });
    const value = { endpoint, expirationTime: null, keys: { p256dh: 'stand-in-p256dh', auth: 'stand-in-auth' } };
    localStorage.setItem('push-stand-in', JSON.stringify(value));
    return Promise.resolve(subscription(value));
  };
  PushManager.prototype.getSubscription = function () { return Promise.resolve(subscription(stored())); };
}

const { chromium } = await import('playwright');
const launch = await import('./browser-launch.mjs');
const profile = await mkdtemp(join(evidence, 'profile-web-push-'));
const { args, ...options } = launch.hostedLinuxWebGpuLaunchOptions();
const context = await chromium.launchPersistentContext(profile, {
  ...options, env: launch.uiLibraryEnvironment(), viewport: { width: 800, height: 600 },
  // Nothing but the test server resolves, so no push service can be reached.
  args: [...args, '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1'],
});
const errors = [];
const violations = [];
async function facts(client, expected, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    last = (await client.observe()).facts;
    if (Object.entries(expected).every(([key, value]) => last[key] === value)) return last;
    await new Promise(fulfil => setTimeout(fulfil, 50));
  }
  throw new Error(`facts ${JSON.stringify(expected)} not observed; last ${JSON.stringify(last)}`);
}
async function open(page, url) {
  await page.goto(launch.rendererUrl(url));
  await page.waitForFunction(() => window.__STOBIE_STARTUP?.status === 'ready', null, { timeout: 60000 });
  const client = new ApplicationClient(new BrowserTransport(page));
  await client.hello();
  return client;
}
const step = (label, detail) => report.steps.push({ label, ...detail });
try {
  await context.addInitScript(pushServiceStandIn, endpoint);
  const page = context.pages()[0] ?? await context.newPage();
  page.on('pageerror', error => errors.push(String(error)));
  page.on('console', message => { if (/Content.Security.Policy/i.test(message.text())) violations.push(message.text()); });
  await page.exposeFunction('__reportViolation', text => violations.push(text));
  await context.addInitScript(() => document.addEventListener('securitypolicyviolation', event => window.__reportViolation(`${event.violatedDirective} ${event.blockedURI}`)));

  // At launch: supported, not yet asked, no subscription.
  let client = await open(page, `${scope}?automation=1`);
  await page.waitForFunction(() => navigator.serviceWorker.controller !== null, null, { timeout: 30000 });
  const launched = await facts(client, { states: 1, support: 'supported', endpoint: '' });
  // Headless Chromium denies by default; a headful profile has not asked.
  assert(['default', 'denied'].includes(launched.permission), launched.permission);
  step('launch', { facts: launched });

  // A physical tap subscribes inside its input event. The user's answer to
  // the permission prompt is the profile's grant.
  await context.grantPermissions(['notifications'], { origin });
  const tap = await client.click({ capability: `${name}.subscribe` }, new BrowserPhysicalAdapter(page, { canvas: '#app-canvas' }));
  const subscribed = await facts(client, { endpoint, permission: 'granted', error: '' });
  const calls = await page.evaluate(() => window.__pushStandIn.calls);
  assert.equal(calls.length, 1);
  assert.match(calls[0].event, /^(pointerup|mouseup|click)$/, `subscribe ran inside the tap's event: ${calls[0].event}`);
  assert.equal(calls[0].userActivation, true);
  assert.equal(calls[0].userVisibleOnly, true);
  assert.deepEqual(calls[0].key, keyBytes, 'the application server key, decoded');
  assert.deepEqual(JSON.parse(subscribed.subscription).keys, { p256dh: 'stand-in-p256dh', auth: 'stand-in-auth' });
  step('subscribed by a tap', { tap: { point: tap.point, node: tap.node }, call: calls[0], facts: subscribed });

  // A relaunch reads the current subscription, to send it again.
  client = await open(page, `${scope}?automation=1`);
  step('relaunch', { facts: await facts(client, { states: 1, endpoint }) });

  // Pushes, through DevTools: a declarative message and an unreadable one.
  const worker = context.serviceWorkers().find(candidate => candidate.url() === `${scope}sw.js`)
    ?? await context.waitForEvent('serviceworker', { predicate: candidate => candidate.url() === `${scope}sw.js`, timeout: 20000 });
  // The worker's own calls are recorded as they settle. Reading
  // `getNotifications` instead races Chromium, which drops from it a
  // notification its message centre has not displayed yet.
  await worker.evaluate(() => {
    const shown = self.__shown = [];
    const show = self.registration.showNotification.bind(self.registration);
    self.registration.showNotification = (title, options) => show(title, options).then(
      result => { shown.push({ title, options, outcome: 'shown' }); return result; },
      error => { shown.push({ title, options, outcome: String(error) }); throw error; });
    for (const method of ['setAppBadge', 'clearAppBadge']) {
      const original = self.navigator[method]?.bind(self.navigator);
      if (original) self.navigator[method] = (...args) => { shown.push({ badge: method, args }); return original(...args); };
    }
  });
  const cdp = await context.newCDPSession(page);
  const registration = new Promise(fulfil => cdp.on('ServiceWorker.workerRegistrationUpdated', ({ registrations }) => {
    const found = registrations.find(entry => entry.scopeURL === scope && !entry.isDeleted);
    if (found) fulfil(found.registrationId);
  }));
  await cdp.send('ServiceWorker.enable');
  const registrationId = await registration;
  const route = `${scope}?bot=scout`;
  const deliver = data => cdp.send('ServiceWorker.deliverPushMessage', { origin, registrationId, data });
  async function shown(count) {
    const deadline = Date.now() + 20000;
    let calls = [];
    while (Date.now() < deadline) {
      calls = await worker.evaluate(() => self.__shown);
      if (calls.filter(call => call.outcome).length >= count) return calls;
      await new Promise(fulfil => setTimeout(fulfil, 50));
    }
    throw new Error(`${count} notifications not shown; last ${JSON.stringify(calls)}`);
  }
  await deliver(JSON.stringify({
    web_push: 8030, mutable: true,
    notification: { title: 'Scout', body: 'Five new papers', navigate: route, tag: 'bot-scout', app_badge: '2' },
  }));
  const declarative = await shown(1);
  assert.deepEqual(declarative.find(call => call.outcome), {
    title: 'Scout', options: { data: { route }, body: 'Five new papers', tag: 'bot-scout' }, outcome: 'shown',
  });
  const badges = declarative.filter(call => call.badge);
  await deliver('not json');
  const all = await shown(2);
  assert.deepEqual(all.filter(call => call.outcome)[1], { title: 'Outside Push', options: { data: { route: scope } }, outcome: 'shown' },
    'an unreadable push is still shown');
  // Chromium on Linux may have no badge; where it has one, the push set it.
  if (badges.length) assert.deepEqual(badges, [{ badge: 'setAppBadge', args: [2] }]);
  step('pushes shown', { calls: all });

  // A tap while the application is open: the worker posts it the route.
  await worker.evaluate(route => openRoute(route), route);
  step('route while open', { facts: await facts(client, { route, path: '?bot=scout', routes: 1 }) });

  // A tap with no window open: the worker remembers the route and opens a
  // window; this one lands on the scope, as WebKit's sometimes do, and the
  // application still reads the route at launch.
  await page.goto('about:blank');
  const later = `${scope}bots/atlas`;
  const opened = await worker.evaluate(route => openRoute(route).then(() => 'opened', error => `${error.name}`), later);
  // Outside a real tap Chromium refuses to open the window, so this page does.
  assert.notEqual(opened, 'opened', 'the worker opened a window outside a tap');
  client = await open(page, `${scope}?automation=1`);
  step('route at launch', { openWindow: opened, facts: await facts(client, { route: later, path: 'bots/atlas', routes: 1 }) });
  // The remembered route is read once.
  client = await open(page, `${scope}?automation=1`);
  const relaunched = await facts(client, { states: 1, routes: 0 });
  step('route read once', { facts: relaunched });

  // Turning notifications off removes the subscription; the badge is optional.
  await client.invoke({ capability: `${name}.unsubscribe` });
  await client.invoke({ capability: `${name}.badge` });
  step('unsubscribed', { facts: await facts(client, { endpoint: '' }) });
  assert.deepEqual(violations, [], 'nothing broke the Content-Security-Policy');
  assert.deepEqual(errors, [], 'no page errors');
  report.result = 'passed';
} finally {
  await context.close();
  server.close();
  await rm(profile, { recursive: true, force: true });
  await writeFile(join(evidence, 'web-push.json'), `${JSON.stringify(report, null, 2)}\n`);
}
console.log(`Web Push smoke passed; evidence ${evidence}/web-push.json`);
