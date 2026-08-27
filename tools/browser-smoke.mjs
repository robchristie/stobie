import { createServer } from 'node:http';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { chromium } from 'playwright';

const root = normalize(join(process.cwd(), 'apps/analytical-workspace-lab/web'));
const evidenceRoot = join(process.cwd(), 'docs/vertical-slice-evidence');
await mkdir(evidenceRoot, { recursive: true });
const mime = new Map([['.html', 'text/html'], ['.js', 'text/javascript'], ['.css', 'text/css'], ['.wasm', 'application/wasm']]);
const server = createServer(async (request, response) => {
  try {
    const relative = request.url === '/' ? 'index.html' : request.url.slice(1).split('?')[0];
    const path = normalize(join(root, relative));
    if (!path.startsWith(root) || !(await stat(path)).isFile()) throw new Error('not found');
    response.writeHead(200, { 'Content-Type': mime.get(extname(path)) ?? 'application/octet-stream', 'Cache-Control': 'no-store' });
    response.end(await readFile(path));
  } catch {
    response.writeHead(404); response.end('not found');
  }
});
await new Promise((resolve) => server.listen(4173, '127.0.0.1', resolve));

const errors = [];
const browser = await chromium.launch({
  headless: true,
  env: { ...process.env, LD_LIBRARY_PATH: `${join(process.cwd(), '.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
  args: ['--enable-unsafe-webgpu', '--enable-features=Vulkan', '--use-angle=vulkan', '--disable-vulkan-surface'],
});
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
page.on('pageerror', (error) => errors.push(`pageerror: ${error.stack ?? error}`));
page.on('console', (message) => { if (message.type() === 'error') errors.push(`console: ${message.text()}`); });

try {
  await page.goto('http://127.0.0.1:4173', { waitUntil: 'domcontentloaded' });
  await page.locator('#polyorama-canvas[data-polyorama-ready="true"]').waitFor({ timeout: 30_000 });
  await page.waitForFunction(() => {
    const diagnostics = window.__POLYORAMA_DIAGNOSTICS;
    return Number(document.querySelector('#polyorama-canvas')?.dataset.workerCompletions ?? 0) > 0 && diagnostics?.runtime?.in_flight === 0 && diagnostics?.render?.draw_calls > 0;
  }, null, { timeout: 30_000 });
  const evidence = await page.evaluate(() => {
    const canvas = document.getElementById('polyorama-canvas');
    return {
      canvas: { width: canvas.width, height: canvas.height },
      panes: Number(canvas.dataset.paneCount),
      renderer: canvas.dataset.renderer,
      workerCompletions: Number(canvas.dataset.workerCompletions),
      diagnostics: window.__POLYORAMA_DIAGNOSTICS,
    };
  });
  if (evidence.canvas.width <= 0 || evidence.canvas.height <= 0) throw new Error('canvas has zero dimensions');
  if (evidence.panes !== 8 || evidence.renderer !== 'wgpu-scalar') throw new Error(`unexpected readiness data: ${JSON.stringify(evidence)}`);
  await page.screenshot({ path: join(evidenceRoot, 'browser-default.png') });

  const semanticSnapshot = () => page.evaluate(() => window.__POLYORAMA_HANDLE.test_snapshot());
  const semanticAction = async (action) => {
    const before = await semanticSnapshot();
    await page.evaluate((value) => window.__POLYORAMA_HANDLE.test_action(value), action);
    await page.waitForFunction(
      (frame) => window.__POLYORAMA_HANDLE.test_snapshot().frame_number > frame,
      before.frame_number,
      { timeout: 10_000 },
    );
    return semanticSnapshot();
  };
  const initialSemantic = await semanticSnapshot();
  if (!initialSemantic.visible_tile_keys.length) throw new Error('Rust semantic snapshot has no visible tile demand');
  if (initialSemantic.runtime.worker_queue_depth > initialSemantic.runtime.external_queue_capacity
      || initialSemantic.runtime.browser_credits_in_use > initialSemantic.runtime.browser_credit_capacity
      || initialSemantic.runtime.scheduler_high_water > initialSemantic.runtime.scheduler_capacity
      || initialSemantic.runtime.external_queue_high_water > initialSemantic.runtime.external_queue_capacity) {
    throw new Error(`runtime exceeded a configured bound: ${JSON.stringify(initialSemantic.runtime)}`);
  }

  const cameraSemantic = await semanticAction({
    kind: 'set_camera', pane: 1, centre_x: 32768, centre_y: 24576, pixels_per_screen_point: 8,
  });
  const primarySemantic = cameraSemantic.cameras.find((item) => item.pane === 1);
  const linkedSemantic = cameraSemantic.cameras.find((item) => item.pane === 2);
  const primaryRender = cameraSemantic.render_cameras.find((item) => item.pane === 1);
  const linkedRender = cameraSemantic.render_cameras.find((item) => item.pane === 2);
  if (JSON.stringify(primarySemantic.camera) !== JSON.stringify(linkedSemantic.camera)) {
    throw new Error('semantic linked-camera command diverged');
  }
  if (JSON.stringify(primarySemantic.camera) !== JSON.stringify(primaryRender.camera)
      || JSON.stringify(linkedSemantic.camera) !== JSON.stringify(linkedRender.camera)) {
    throw new Error('render plan did not use the authoritative same-frame camera');
  }
  if (JSON.stringify(cameraSemantic.visible_tile_keys) === JSON.stringify(initialSemantic.visible_tile_keys)) {
    throw new Error('semantic camera change did not change visible tile demand');
  }
  await semanticAction({ kind: 'undo' });

  const polygonSemantic = await semanticAction({
    kind: 'commit_polygon', vertices: [[10, 10], [90, 20], [40, 100]],
  });
  if (polygonSemantic.annotation_count !== initialSemantic.annotation_count + 1
      || polygonSemantic.selected_annotation == null
      || polygonSemantic.undo_depth !== initialSemantic.undo_depth + 1) {
    throw new Error(`semantic polygon commit was not one selected undo record: ${JSON.stringify(polygonSemantic)}`);
  }
  const polygonUndone = await semanticAction({ kind: 'undo' });
  if (polygonUndone.annotation_count !== initialSemantic.annotation_count) {
    throw new Error('semantic polygon undo did not restore the document');
  }

  const resizedSemantic = await semanticAction({ kind: 'resize_split', node: 1, fraction: 0.61 });
  if (resizedSemantic.workspace_hash === initialSemantic.workspace_hash) {
    throw new Error('semantic dock resize did not change the canonical workspace');
  }
  const resizeUndone = await semanticAction({ kind: 'undo' });
  if (resizeUndone.workspace_hash !== initialSemantic.workspace_hash) {
    throw new Error('semantic dock undo did not exactly restore the canonical workspace');
  }
  const semanticEvidence = {
    initial: {
      visible_tile_keys: initialSemantic.visible_tile_keys,
      worker_health: initialSemantic.runtime.worker_health,
      worker_failures: initialSemantic.runtime.worker_failures,
      scheduler_high_water: initialSemantic.runtime.scheduler_high_water,
      scheduler_capacity: initialSemantic.runtime.scheduler_capacity,
      external_queue_high_water: initialSemantic.runtime.external_queue_high_water,
      external_queue_capacity: initialSemantic.runtime.external_queue_capacity,
      browser_credits_in_use: initialSemantic.runtime.browser_credits_in_use,
      browser_credit_capacity: initialSemantic.runtime.browser_credit_capacity,
    },
    linked_camera_and_render_plan: {
      cameras: cameraSemantic.cameras.filter((item) => item.pane === 1 || item.pane === 2),
      render_cameras: cameraSemantic.render_cameras.filter((item) => item.pane === 1 || item.pane === 2),
      visible_tile_keys: cameraSemantic.visible_tile_keys,
      demand_changed: JSON.stringify(cameraSemantic.visible_tile_keys) !== JSON.stringify(initialSemantic.visible_tile_keys),
    },
    polygon: {
      before_count: initialSemantic.annotation_count,
      committed_count: polygonSemantic.annotation_count,
      selected_annotation: polygonSemantic.selected_annotation,
      undo_depth_before: initialSemantic.undo_depth,
      undo_depth_after_commit: polygonSemantic.undo_depth,
      count_after_undo: polygonUndone.annotation_count,
    },
    dock: {
      before_hash: initialSemantic.workspace_hash,
      resized_hash: resizedSemantic.workspace_hash,
      hash_after_undo: resizeUndone.workspace_hash,
    },
  };

  const percentile = (values, p) => values.length ? [...values].sort((a, b) => a - b)[Math.min(values.length - 1, Math.floor(values.length * p))] : null;
  const observations = {};
  async function observe(name, action) {
    const before = await page.evaluate(() => ({ frame: window.__POLYORAMA_DIAGNOSTICS.frame.frame_number, samples: window.__POLYORAMA_DIAGNOSTICS.frame.cpu_frame_history_ms.length }));
    await action();
    await page.waitForFunction((frame) => window.__POLYORAMA_DIAGNOSTICS.frame.frame_number > frame, before.frame, { timeout: 10_000 });
    await page.waitForTimeout(250);
    const samples = await page.evaluate((start) => window.__POLYORAMA_DIAGNOSTICS.frame.cpu_frame_history_ms.slice(start), before.samples);
    observations[name] = { samples, median_ms: percentile(samples, 0.5), p95_ms: percentile(samples, 0.95) };
  }

  observations.initial_loading = {
    samples: evidence.diagnostics.frame.cpu_frame_history_ms,
    median_ms: percentile(evidence.diagnostics.frame.cpu_frame_history_ms, 0.5),
    p95_ms: percentile(evidence.diagnostics.frame.cpu_frame_history_ms, 0.95),
  };
  const rasterBeforePan = await page.screenshot({ path: join(evidenceRoot, 'browser-pan-before.png') });
  const panHistoryBefore = await semanticSnapshot();
  await observe('four_viewports_panning', async () => {
    await page.mouse.move(300, 300); await page.mouse.down(); await page.mouse.move(390, 350, { steps: 12 }); await page.mouse.up();
  });
  const rasterAfterPan = await page.screenshot({ path: join(evidenceRoot, 'browser-pan-after.png') });
  if (rasterAfterPan.equals(rasterBeforePan)) throw new Error('rendered canvas did not change after pan');
  const panHistoryAfter = await semanticSnapshot();
  if (panHistoryAfter.undo_depth !== panHistoryBefore.undo_depth + 1) {
    throw new Error('one completed camera drag did not create exactly one history record');
  }

  const zoomHistoryBefore = await semanticSnapshot();
  await observe('rapid_zoom_transitions', async () => {
    await page.mouse.move(300, 300); for (let index = 0; index < 6; index += 1) await page.mouse.wheel(0, -120);
  });
  await page.waitForFunction(
    (depth) => window.__POLYORAMA_HANDLE.test_snapshot().undo_depth === depth + 1,
    zoomHistoryBefore.undo_depth,
    { timeout: 2_000 },
  );
  const zoomHistoryAfter = await semanticSnapshot();
  if (zoomHistoryAfter.undo_depth !== zoomHistoryBefore.undo_depth + 1) {
    throw new Error(`wheel zoom burst did not coalesce into one history record (${zoomHistoryBefore.undo_depth} -> ${zoomHistoryAfter.undo_depth})`);
  }
  await page.mouse.click(244, 77);
  await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS.cameras.find((item) => item.pane === 1)?.link === null);
  await page.mouse.click(244, 77);
  await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS.cameras.find((item) => item.pane === 1)?.link === 1);
  await page.mouse.click(211, 77);
  await page.waitForFunction(() => {
    const cameras = window.__POLYORAMA_DIAGNOSTICS.cameras;
    const primary = cameras.find((item) => item.pane === 1)?.camera;
    const linked = cameras.find((item) => item.pane === 2)?.camera;
    return primary?.pixels_per_screen_point < 512 && JSON.stringify(primary) === JSON.stringify(linked);
  });
  await observe('million_row_scroll', async () => {
    await page.mouse.move(1180, 270); await page.mouse.wheel(0, 1800);
  });
  await observe('thumbnail_scroll', async () => {
    await page.mouse.click(1170, 53); await page.mouse.move(1180, 270); await page.mouse.wheel(0, 1500);
  });
  await page.screenshot({ path: join(evidenceRoot, 'browser-thumbnails.png') });
  const thumbnailSemantic = await semanticSnapshot();
  if (!thumbnailSemantic.thumbnail_resident_keys.length
      || thumbnailSemantic.thumbnail_resident_keys.length > 256) {
    throw new Error(`decoded thumbnail cache is empty or unbounded: ${thumbnailSemantic.thumbnail_resident_keys.length}`);
  }
  semanticEvidence.thumbnail_cache = {
    resident_keys: thumbnailSemantic.thumbnail_resident_keys,
    resident_count: thumbnailSemantic.thumbnail_resident_keys.length,
    configured_item_bound: 256,
  };
  await observe('polygon_editing', async () => {
    await page.mouse.click(110, 77);
    await page.mouse.click(180, 180); await page.mouse.click(360, 200); await page.mouse.click(270, 340);
    await page.mouse.click(270, 340, { button: 'right' });
  });
  await page.screenshot({ path: join(evidenceRoot, 'browser-polygon.png') });
  await observe('dock_splitter_interaction', async () => {
    await page.mouse.move(1037, 400); await page.mouse.down(); await page.mouse.move(990, 400, { steps: 6 }); await page.mouse.up();
  });
  await observe('dock_pane_drag', async () => {
    await page.mouse.move(550, 642); await page.mouse.down(); await page.waitForTimeout(150);
    await page.mouse.move(800, 500, { steps: 6 });
    await page.mouse.move(1150, 200, { steps: 6 }); await page.waitForTimeout(150); await page.mouse.up();
  });
  await page.screenshot({ path: join(evidenceRoot, 'browser-rearranged-dock.png') });
  await page.mouse.click(325, 18);
  await page.waitForTimeout(250);
  const persistedKeys = await page.evaluate(() => Object.keys(localStorage));
  if (persistedKeys.length === 0) throw new Error('Save layout did not create browser persistence');

  const idleFrame = await page.evaluate(() => window.__POLYORAMA_DIAGNOSTICS.frame.frame_number);
  await page.waitForTimeout(700);
  const idleFrameAfter = await page.evaluate(() => window.__POLYORAMA_DIAGNOSTICS.frame.frame_number);
  if (idleFrameAfter !== idleFrame) throw new Error(`idle workspace repainted continuously (${idleFrame} -> ${idleFrameAfter})`);
  observations.warmed_idle = { frame_before: idleFrame, frame_after: idleFrameAfter, deliberate_continuous_repaint: false };
  semanticEvidence.warmed_idle = observations.warmed_idle;

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.locator('#polyorama-canvas[data-polyorama-ready="true"]').waitFor({ timeout: 30_000 });
  await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS?.render?.draw_calls > 0, null, { timeout: 30_000 });
  await page.screenshot({ path: join(evidenceRoot, 'browser-reloaded.png') });
  const restored = await page.evaluate(() => window.__POLYORAMA_DIAGNOSTICS);
  observations.saved_workspace_restore = {
    samples: restored.frame.cpu_frame_history_ms,
    median_ms: percentile(restored.frame.cpu_frame_history_ms, 0.5),
    p95_ms: percentile(restored.frame.cpu_frame_history_ms, 0.95),
  };
  await writeFile(join(evidenceRoot, 'browser-diagnostics.json'), JSON.stringify(restored, null, 2));
  await writeFile(join(evidenceRoot, 'browser-semantic.json'), JSON.stringify(semanticEvidence, null, 2));

  const responsiveEvidence = [];
  for (const viewport of [
    { width: 1280, height: 720, label: '1280x720' },
    { width: 900, height: 700, label: 'narrow' },
  ]) {
    await page.setViewportSize({ width: viewport.width, height: viewport.height });
    await page.reload({ waitUntil: 'domcontentloaded' });
    await page.locator('#polyorama-canvas[data-polyorama-ready="true"]').waitFor({ timeout: 30_000 });
    await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS?.render?.draw_calls > 0, null, { timeout: 30_000 });
    const canvas = await page.evaluate(() => {
      const element = document.getElementById('polyorama-canvas');
      return { width: element.width, height: element.height };
    });
    if (canvas.width <= 0 || canvas.height <= 0) throw new Error(`${viewport.label} canvas has zero dimensions`);
    responsiveEvidence.push({ viewport: `${viewport.width}x${viewport.height}`, canvas });
    await page.screenshot({ path: join(evidenceRoot, `browser-${viewport.label}.png`) });
  }
  await writeFile(join(evidenceRoot, 'browser-performance.json'), JSON.stringify({
    build: 'release',
    browser: browser.version(),
    automation: 'Playwright 1.62.1',
    host: `${process.platform}/${process.arch}`,
    backend: restored.backend,
    adapter: restored.adapter,
    primary_viewport: '1440x900',
    responsive_evidence: responsiveEvidence,
    observations,
  }, null, 2));
  if (errors.length) throw new Error(errors.join('\n'));
  console.log(JSON.stringify({ status: 'passed', ...evidence }, null, 2));
} catch (error) {
  await page.screenshot({ path: join(evidenceRoot, 'browser-failure.png') });
  const loading = await page.locator('#loading').textContent().catch(() => '<missing>');
  throw new Error(`${error.stack ?? error}\nloading: ${loading}\n${errors.join('\n')}`);
} finally {
  await browser.close();
  await new Promise((resolve) => server.close(resolve));
}
