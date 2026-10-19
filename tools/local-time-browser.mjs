// Check `stobie_ui_egui::local_time` in the browser, through the Gallery's
// WebAssembly build: the shared fixtures, formatted through the page's Intl,
// must give exactly the values the native unit tests check, and the viewer's
// zone and locale must be the page's. Each page runs with Chromium's time zone
// and locale emulation, so the result does not depend on the host's settings.
//
// Build the Gallery's web package first (`cargo xtask verify-stage
// gallery-browser` does), then: node tools/local-time-browser.mjs
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { chromium } from 'playwright';
import { hostedLinuxWebGpuLaunchOptions, uiLibraryEnvironment } from './browser-launch.mjs';

const web = resolve('apps/stobie-gallery/web');
const fixturesText = await readFile(resolve('crates/stobie-ui-egui/src/local_time/fixtures.json'), 'utf8');
const fixtures = JSON.parse(fixturesText);
const evidence = resolve(process.env.STOBIE_EVIDENCE_DIR ?? '.tools/runtime/verification-evidence', 'local-time-browser');
await mkdir(evidence, { recursive: true });

// A page that loads only the Gallery's module: no canvas, renderer or application.
const page = `<!doctype html><meta charset="utf-8"><title>Local time</title>
<script type="module">
import init, { local_time_report, local_time_viewer_report } from './pkg/stobie_gallery.js';
await init({ module_or_path: './pkg/stobie_gallery_bg.wasm' });
window.__LOCAL_TIME = { local_time_report, local_time_viewer_report };
</script>`;
const assets = new Map([
  ['pkg/stobie_gallery.js', 'text/javascript'],
  ['pkg/stobie_gallery_bg.wasm', 'application/wasm'],
]);
const server = createServer(async (request, response) => {
  const name = new URL(request.url, 'http://local').pathname.slice(1);
  try {
    if (name === 'local-time.html') {
      response.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
      response.end(page);
      return;
    }
    assert(assets.has(name), 'allowlisted Gallery asset');
    const body = await readFile(join(web, name));
    response.writeHead(200, { 'Content-Type': assets.get(name), 'Cache-Control': 'no-store' });
    response.end(body);
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise(resolveListen => server.listen(0, '127.0.0.1', resolveListen));
const url = `http://127.0.0.1:${server.address().port}/local-time.html`;

// 2026-04-04T17:00:00Z, seen from 2026-04-05T00:00:00Z: 2:30 AM in Adelaide
// after daylight saving ended that morning, and the evening before elsewhere.
const at = Date.UTC(2026, 3, 4, 17, 0);
const now = Date.UTC(2026, 3, 5, 0, 0);
const viewers = [
  {
    timezoneId: 'Australia/Adelaide', locale: 'en-AU', resolved: '12 day-month',
    formats: { time_of_day: '2:30 AM', short_date: '5 Apr', long_date: 'Sunday 5 April 2026', relative_day: 'Today', relative_day_and_time: 'Today at 2:30 AM', compact: '2:30 AM', utc_offset: 'UTC+9:30' },
  },
  {
    timezoneId: 'America/New_York', locale: 'en-US', resolved: '12 month-day',
    formats: { time_of_day: '1:00 PM', short_date: 'Apr 4', long_date: 'Saturday, April 4, 2026', relative_day: 'Today', relative_day_and_time: 'Today at 1:00 PM', compact: '1:00 PM', utc_offset: 'UTC-4:00' },
  },
  {
    timezoneId: 'Europe/London', locale: 'en-GB', resolved: '24 day-month',
    formats: { time_of_day: '18:00', short_date: '4 Apr', long_date: 'Saturday 4 April 2026', relative_day: 'Yesterday', relative_day_and_time: 'Yesterday at 18:00', compact: 'Yesterday', utc_offset: 'UTC+1:00' },
  },
  {
    // Fields stay Gregorian with Latin digits whatever the page's locale.
    timezoneId: 'Asia/Tokyo', locale: 'ar-EG', resolved: '12 day-month',
    formats: { time_of_day: '2:00 AM', short_date: '5 Apr', long_date: 'Sunday 5 April 2026', relative_day: 'Today', relative_day_and_time: 'Today at 2:00 AM', compact: '2:00 AM', utc_offset: 'UTC+9:00' },
  },
];

const browser = await chromium.launch({ ...hostedLinuxWebGpuLaunchOptions(), env: uiLibraryEnvironment() });
const report = { browser: browser.version(), url, at, now, fixtures: null, viewers: [] };
try {
  for (const viewer of viewers) {
    const context = await browser.newContext({ timezoneId: viewer.timezoneId, locale: viewer.locale });
    try {
      const tab = await context.newPage();
      const errors = [];
      tab.on('pageerror', error => errors.push(String(error)));
      await tab.goto(url);
      await tab.waitForFunction(() => window.__LOCAL_TIME, null, { timeout: 30_000 });
      const result = await tab.evaluate(({ fixturesText, at, now }) => ({
        page: { zone: Intl.DateTimeFormat().resolvedOptions().timeZone, locale: navigator.language },
        fixtures: JSON.parse(window.__LOCAL_TIME.local_time_report(fixturesText)),
        viewer: JSON.parse(window.__LOCAL_TIME.local_time_viewer_report(at, now)),
      }), { fixturesText, at, now });
      assert.deepEqual(errors, [], `${viewer.timezoneId}: no page errors`);
      // The emulation took effect, so the checks below are about the page's zone.
      assert.equal(result.page.zone, viewer.timezoneId);
      assert.equal(result.page.locale, viewer.locale);
      assert.deepEqual(result.fixtures, fixtures, `${viewer.timezoneId}: Intl formats the fixtures as natively`);
      assert.deepEqual(result.viewer, {
        zone: viewer.timezoneId, locale: viewer.resolved, formats: viewer.formats,
      }, `${viewer.timezoneId}: the viewer's zone and locale are the page's`);
      report.fixtures ??= result.fixtures;
      report.viewers.push({ timezoneId: viewer.timezoneId, locale: viewer.locale, page: result.page, viewer: result.viewer });
    } finally {
      await context.close();
    }
  }
} finally {
  await browser.close();
  server.close();
  await writeFile(join(evidence, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
}
const instants = fixtures.cases.reduce((count, item) => count + item.instants.length, 0);
console.log(`Local time agrees in ${report.browser}: ${instants} fixture instants in ${fixtures.cases.length} zones, ${Object.keys(fixtures.locales).length} language tags, ${viewers.length} page zones`);
