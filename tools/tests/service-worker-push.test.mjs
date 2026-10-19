import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

// The worker `stobie build-web` writes for a page that opts into Web Push,
// filled as `pwa::service_worker` fills it.
const web = new URL('../../crates/stobie-build/web/', import.meta.url);
const source = readFileSync(new URL('service-worker.js', web), 'utf8')
  .replace('__STOBIE_APP__', '"notes"')
  .replace('__STOBIE_VERSION__', '"development"')
  .replace('__STOBIE_PRECACHE__', '[]')
  .replace('__STOBIE_FILES__', '[]')
  + readFileSync(new URL('service-worker-push.js', web), 'utf8').replace('__STOBIE_NAME__', '"Field Notes"');
const SCOPE = 'https://app.test/notes/';
// Objects made in the worker's realm, as plain values of this one.
const plain = value => JSON.parse(JSON.stringify(value));

function worker({ windows = [], focus = client => client } = {}) {
  const listeners = new Map();
  const shown = [], badges = [], opened = [], posted = [], stored = new Map();
  const client = url => ({
    url,
    focus: async function () { return focus(this); },
    postMessage(message) { posted.push(plain({ url, message })); },
  });
  const self = {
    registration: { scope: SCOPE, showNotification: async (title, options) => { shown.push(plain({ title, options })); } },
    navigator: { setAppBadge: async count => { badges.push(count); }, clearAppBadge: async () => { badges.push(0); } },
    clients: {
      matchAll: async options => { assert.deepEqual(plain(options), { type: 'window', includeUncontrolled: true }); return windows.map(client); },
      openWindow: async url => { opened.push(url); },
    },
    addEventListener: (type, listener) => listeners.set(type, listener),
  };
  const caches = {
    open: async name => ({
      put: async (key, response) => { stored.set(`${name} ${key}`, await response.json()); },
    }),
  };
  vm.runInNewContext(source, { self, caches, Response, URL, Set, Date, Promise, JSON, Number });
  async function dispatch(type, fields) {
    const waits = [];
    listeners.get(type)({ ...fields, waitUntil: promise => waits.push(promise) });
    assert.equal(waits.length, 1, `${type} waits for its work`);
    await Promise.all(waits);
  }
  const push = text => dispatch('push', { data: text === undefined ? null : { json: () => JSON.parse(text) } });
  const click = data => {
    const notification = { data, closed: false, close() { this.closed = true; } };
    return dispatch('notificationclick', { notification }).then(() => notification);
  };
  return { push, click, shown, badges, opened, posted, stored };
}

test('a declarative message shows its notification and sets the badge', async () => {
  const sw = worker();
  await sw.push(JSON.stringify({
    web_push: 8030, mutable: true,
    notification: { title: ' Scout ', body: 'Five new papers', navigate: `${SCOPE}?bot=scout`, tag: 'bot-scout', lang: 'en-AU', dir: 'ltr', app_badge: '3', silent: true },
  }));
  assert.deepEqual(sw.shown, [{
    title: 'Scout',
    options: { data: { route: `${SCOPE}?bot=scout` }, body: 'Five new papers', tag: 'bot-scout', lang: 'en-AU', dir: 'ltr' },
  }]);
  assert.deepEqual(sw.badges, [3]);
});

test('the notification object alone, a top-level badge and a relative route', async () => {
  const sw = worker();
  await sw.push(JSON.stringify({ title: 'Scout', navigate: 'bots/scout', app_badge: 0 }));
  await sw.push(JSON.stringify({ web_push: 8030, notification: { title: 'Scout', navigate: '/elsewhere/' }, app_badge: 2 }));
  assert.deepEqual(sw.shown.map(entry => entry.options.data.route), [`${SCOPE}bots/scout`, SCOPE], 'a route outside the scope opens the application');
  assert.deepEqual(sw.badges, [0, 2]);
});

test('an unreadable payload still shows a notification that opens the application', async () => {
  for (const text of [undefined, 'not json', '"text"', '[1]', 'null', '{"web_push":8030}', '{"title":7,"dir":"sideways","app_badge":"x"}']) {
    const sw = worker();
    await sw.push(text);
    assert.deepEqual(sw.shown, [{ title: 'Field Notes', options: { data: { route: SCOPE } } }], String(text));
    assert.deepEqual(sw.badges, [], String(text));
  }
  for (const badge of [-1, 1.5, '1e3', true]) {
    const sw = worker();
    await sw.push(JSON.stringify({ title: 'Scout', app_badge: badge }));
    assert.deepEqual(sw.badges, [], String(badge));
  }
});

test('a tap focuses an open window of the scope and posts it the route', async () => {
  const sw = worker({ windows: ['https://app.test/other/', `${SCOPE}?bot=atlas`] });
  const notification = await sw.click({ route: `${SCOPE}?bot=scout` });
  assert(notification.closed);
  assert.deepEqual(sw.posted, [{ url: `${SCOPE}?bot=atlas`, message: { stobie: 'notification', route: `${SCOPE}?bot=scout` } }]);
  assert.deepEqual(sw.opened, []);
  assert.equal(sw.stored.size, 0);
});

test('a window that cannot be focused still receives the route', async () => {
  const sw = worker({ windows: [SCOPE], focus: () => { throw new Error('InvalidAccessError'); } });
  await sw.click({ route: `${SCOPE}?bot=scout` });
  assert.deepEqual(sw.posted.map(entry => entry.message.route), [`${SCOPE}?bot=scout`]);
});

test('without a window, a tap remembers the route and opens it', async () => {
  const before = Date.now();
  const sw = worker({ windows: ['https://app.test/other/'] });
  await sw.click({ route: `${SCOPE}?bot=scout` });
  assert.deepEqual(sw.opened, [`${SCOPE}?bot=scout`]);
  const remembered = sw.stored.get(`stobie-notification-route ${SCOPE}__stobie/notification-route`);
  assert.equal(remembered.route, `${SCOPE}?bot=scout`);
  assert(remembered.at >= before && remembered.at <= Date.now());
  // A route from elsewhere, or none, opens the application itself.
  for (const data of [{ route: 'https://evil.test/' }, undefined, null]) {
    const other = worker();
    await other.click(data);
    assert.deepEqual(other.opened, [SCOPE]);
  }
});
