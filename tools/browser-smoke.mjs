import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { basename, dirname, extname, join, normalize } from 'node:path';
import { tmpdir } from 'node:os';
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
const installedChromium = chromium.executablePath();
const revisionDirectory = basename(dirname(dirname(installedChromium)));
const revision = revisionDirectory.slice(revisionDirectory.lastIndexOf('-') + 1);
const headlessShell = process.env.POLYORAMA_CHROMIUM ?? join(
  dirname(dirname(dirname(installedChromium))),
  `chromium_headless_shell-${revision}`,
  'chrome-headless-shell-linux64',
  'chrome-headless-shell',
);
const browserProfile = await mkdtemp(join(tmpdir(), 'polyorama-browser-'));
const browserProcess = spawn(headlessShell, [
  '--headless',
  '--no-sandbox',
  '--remote-debugging-port=0',
  `--user-data-dir=${browserProfile}`,
  '--enable-unsafe-webgpu',
  '--enable-features=Vulkan',
  '--use-angle=vulkan',
  '--disable-vulkan-surface',
  'about:blank',
], {
  env: { ...process.env, LD_LIBRARY_PATH: `${join(process.cwd(), '.tools/sysroot/usr/lib')}:${process.env.LD_LIBRARY_PATH ?? ''}` },
  stdio: ['ignore', 'ignore', 'pipe'],
});
const cdpEndpoint = await new Promise((resolve, reject) => {
  let diagnostics = '';
  const timeout = setTimeout(() => reject(new Error(`Chromium CDP endpoint timed out: ${diagnostics}`)), 10_000);
  browserProcess.stderr.setEncoding('utf8');
  browserProcess.stderr.on('data', (chunk) => {
    diagnostics += chunk;
    const endpoint = diagnostics.match(/DevTools listening on (ws:\/\/\S+)/)?.[1];
    if (endpoint) {
      clearTimeout(timeout);
      resolve(endpoint);
    }
  });
  browserProcess.once('error', (error) => {
    clearTimeout(timeout);
    reject(error);
  });
  browserProcess.once('exit', (code, signal) => {
    clearTimeout(timeout);
    reject(new Error(`Chromium exited before CDP attachment: code=${code} signal=${signal}\n${diagnostics}`));
  });
});
const browser = await chromium.connectOverCDP(cdpEndpoint);
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
  const targetPoint = async (target, fractionX = 0.5, fractionY = 0.5, deltaX = 0, deltaY = 0) => {
    const snapshot = await semanticSnapshot();
    const geometry = snapshot.ui_geometry;
    const root = geometry.root;
    let rect;
    if (target.kind === 'control') {
      rect = geometry.controls.find((item) => item.name === target.name
        && (target.pane == null || item.pane === target.pane))?.rect;
    } else if (target.kind === 'splitter') {
      rect = geometry.splitters.find((item) => item.node === target.node)?.rect;
    } else if (target.kind === 'thumbnail_scroll' || target.kind === 'results_scroll') {
      rect = geometry[target.kind];
    } else if (target.kind === 'rightmost_pane_body') {
      rect = geometry.pane_bodies.reduce(
        (rightmost, item) => !rightmost || item.rect.min_x > rightmost.min_x ? item.rect : rightmost,
        null,
      );
    } else if (target.kind === 'first_result_row') {
      rect = geometry.result_rows[0]?.rect;
    } else {
      rect = geometry[target.kind].find((item) => item.pane === target.pane)?.rect;
    }
    if (!root || !rect) throw new Error(`missing Rust UI geometry for ${JSON.stringify(target)}`);
    const values = [root.min_x, root.min_y, root.max_x, root.max_y, rect.min_x, rect.min_y, rect.max_x, rect.max_y];
    if (!values.every(Number.isFinite) || rect.max_x <= rect.min_x || rect.max_y <= rect.min_y) {
      throw new Error(`invalid Rust UI geometry for ${JSON.stringify(target)}: ${JSON.stringify(rect)}`);
    }
    const canvas = await page.locator('#polyorama-canvas').boundingBox();
    if (!canvas || root.max_x <= root.min_x || root.max_y <= root.min_y) throw new Error('canvas/root geometry is unavailable');
    const logicalX = rect.min_x + (rect.max_x - rect.min_x) * fractionX + deltaX;
    const logicalY = rect.min_y + (rect.max_y - rect.min_y) * fractionY + deltaY;
    const point = {
      x: canvas.x + (logicalX - root.min_x) * canvas.width / (root.max_x - root.min_x),
      y: canvas.y + (logicalY - root.min_y) * canvas.height / (root.max_y - root.min_y),
    };
    if (point.x < canvas.x || point.x > canvas.x + canvas.width
      || point.y < canvas.y || point.y > canvas.y + canvas.height) {
      throw new Error(`Rust UI target fell outside canvas: ${JSON.stringify({ target, point, canvas })}`);
    }
    return point;
  };
  const clickTarget = async (target, options = {}) => {
    const point = await targetPoint(target);
    await page.mouse.click(point.x, point.y, options);
  };
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
  if (!initialSemantic.ui_geometry.root
      || initialSemantic.ui_geometry.tabs.length !== 8
      || initialSemantic.ui_geometry.image_viewports.length < 4) {
    throw new Error(`Rust semantic UI geometry is incomplete: ${JSON.stringify(initialSemantic.ui_geometry)}`);
  }
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
    ui_geometry: {
      root: initialSemantic.ui_geometry.root,
      tab_panes: initialSemantic.ui_geometry.tabs.map((item) => item.pane),
      visible_pane_bodies: initialSemantic.ui_geometry.pane_bodies.map((item) => item.pane),
      image_viewports: initialSemantic.ui_geometry.image_viewports,
      control_names: initialSemantic.ui_geometry.controls.map((item) => `${item.pane ?? 'global'}:${item.name}`),
      results_scroll: initialSemantic.ui_geometry.results_scroll,
    },
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

  await semanticAction({ kind: 'queue_zero_viewport_upload' });
  await page.waitForFunction(() => {
    const snapshot = window.__POLYORAMA_HANDLE.test_snapshot();
    return JSON.stringify(snapshot.visible_panes) === JSON.stringify([5])
      && snapshot.render.pending_upload_bytes === 0
      && snapshot.render.gpu_viewports === 0
      && snapshot.render.render_jobs === 0
      && snapshot.render.paint_callbacks === 0
      && snapshot.runtime.queued === 0
      && snapshot.runtime.in_flight === 0;
  }, null, { timeout: 10_000 });
  const zeroViewport = await semanticSnapshot();
  await page.waitForTimeout(500);
  const zeroViewportIdleFrame = (await semanticSnapshot()).frame_number;
  await page.waitForTimeout(700);
  const zeroViewportIdleAfter = (await semanticSnapshot()).frame_number;
  if (zeroViewportIdleAfter !== zeroViewportIdleFrame) {
    throw new Error(`zero-viewport maintenance did not become idle (${zeroViewportIdleFrame} -> ${zeroViewportIdleAfter})`);
  }
  semanticEvidence.zero_viewport_maintenance = {
    visible_panes: zeroViewport.visible_panes,
    pending_upload_bytes: zeroViewport.render.pending_upload_bytes,
    gpu_viewports: zeroViewport.render.gpu_viewports,
    render_jobs: zeroViewport.render.render_jobs,
    paint_callbacks: zeroViewport.render.paint_callbacks,
    resident_texture_bytes: zeroViewport.render.resident_texture_bytes,
    runtime_state: {
      desired: zeroViewport.runtime.desired,
      queued: zeroViewport.runtime.queued,
      in_flight: zeroViewport.runtime.in_flight,
      residency_rejected: zeroViewport.runtime.residency_rejected,
    },
    frame_before_idle: zeroViewportIdleFrame,
    frame_after_idle: zeroViewportIdleAfter,
  };
  await semanticAction({ kind: 'restore_default_workspace' });
  await page.waitForFunction(() => {
    const snapshot = window.__POLYORAMA_HANDLE.test_snapshot();
    return snapshot.render.render_jobs > 0 && snapshot.visible_tile_keys.length > 0;
  }, null, { timeout: 10_000 });

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
  const primaryBeforePan = panHistoryBefore.cameras.find((item) => item.pane === 1);
  const linkedBeforePan = panHistoryBefore.cameras.find((item) => item.pane === 2);
  await observe('four_viewports_panning', async () => {
    const start = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.35, 0.45);
    const end = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.35, 0.45, 90, 50);
    await page.mouse.move(start.x, start.y); await page.mouse.down(); await page.mouse.move(end.x, end.y, { steps: 12 }); await page.mouse.up();
  });
  const rasterAfterPan = await page.screenshot({ path: join(evidenceRoot, 'browser-pan-after.png') });
  if (rasterAfterPan.equals(rasterBeforePan)) throw new Error('rendered canvas did not change after pan');
  const panHistoryAfter = await semanticSnapshot();
  if (panHistoryAfter.undo_depth !== panHistoryBefore.undo_depth + 1) {
    throw new Error('one completed camera drag did not create exactly one history record');
  }
  const primaryAfterPan = panHistoryAfter.cameras.find((item) => item.pane === 1);
  const linkedAfterPan = panHistoryAfter.cameras.find((item) => item.pane === 2);
  const primaryRenderAfterPan = panHistoryAfter.render_cameras.find((item) => item.pane === 1);
  const expectedPan = {
    x: primaryBeforePan.camera.centre.x - 90 * primaryBeforePan.camera.pixels_per_screen_point,
    y: primaryBeforePan.camera.centre.y - 50 * primaryBeforePan.camera.pixels_per_screen_point,
  };
  const panTolerance = 1e-6;
  if (Math.abs(primaryAfterPan.camera.centre.x - expectedPan.x) > panTolerance
      || Math.abs(primaryAfterPan.camera.centre.y - expectedPan.y) > panTolerance) {
    throw new Error(`physical pan did not preserve the full 90x50-point drag: expected ${JSON.stringify(expectedPan)}, got ${JSON.stringify(primaryAfterPan.camera.centre)}`);
  }
  if (JSON.stringify(primaryAfterPan.camera) !== JSON.stringify(linkedAfterPan.camera)
      || JSON.stringify(primaryAfterPan.camera) !== JSON.stringify(primaryRenderAfterPan.camera)) {
    throw new Error('physical pan did not propagate one camera through the linked model and render plan');
  }
  const panUndone = await semanticAction({ kind: 'undo' });
  const primaryAfterPanUndo = panUndone.cameras.find((item) => item.pane === 1);
  const linkedAfterPanUndo = panUndone.cameras.find((item) => item.pane === 2);
  if (JSON.stringify(primaryAfterPanUndo.camera) !== JSON.stringify(primaryBeforePan.camera)
      || JSON.stringify(linkedAfterPanUndo.camera) !== JSON.stringify(linkedBeforePan.camera)
      || panUndone.undo_depth !== panHistoryBefore.undo_depth) {
    throw new Error('physical pan undo did not restore the exact linked starting cameras');
  }
  semanticEvidence.physical_pan = {
    pointer_delta: { x: 90, y: 50 },
    before: [primaryBeforePan, linkedBeforePan],
    after: [primaryAfterPan, linkedAfterPan],
    render_after: primaryRenderAfterPan,
    expected_centre: expectedPan,
    undo_restored: [primaryAfterPanUndo, linkedAfterPanUndo],
    undo_depth_before: panHistoryBefore.undo_depth,
    undo_depth_after: panHistoryAfter.undo_depth,
  };

  const noOpBefore = await semanticSnapshot();
  const noOpStart = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.35, 0.45);
  const noOpAway = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.35, 0.45, 60, 30);
  await page.mouse.move(noOpStart.x, noOpStart.y); await page.mouse.down();
  await page.mouse.move(noOpAway.x, noOpAway.y, { steps: 6 });
  await page.mouse.move(noOpStart.x, noOpStart.y, { steps: 6 }); await page.mouse.up();
  await page.waitForTimeout(200);
  const noOpAfter = await semanticSnapshot();
  if (noOpAfter.undo_depth !== noOpBefore.undo_depth
      || JSON.stringify(noOpAfter.cameras) !== JSON.stringify(noOpBefore.cameras)) {
    throw new Error('physical camera drag returning to origin created model or history state');
  }
  semanticEvidence.physical_no_op_pan = {
    pointer_path: [{ x: 0, y: 0 }, { x: 60, y: 30 }, { x: 0, y: 0 }],
    undo_depth_before: noOpBefore.undo_depth,
    undo_depth_after: noOpAfter.undo_depth,
    cameras_unchanged: true,
  };

  const zoomHistoryBefore = await semanticSnapshot();
  await observe('rapid_zoom_transitions', async () => {
    const point = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.35, 0.45);
    await page.mouse.move(point.x, point.y); for (let index = 0; index < 6; index += 1) await page.mouse.wheel(0, -120);
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
  await clickTarget({ kind: 'control', pane: 1, name: 'link_a' });
  await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS.cameras.find((item) => item.pane === 1)?.link === null);
  await clickTarget({ kind: 'control', pane: 1, name: 'link_a' });
  await page.waitForFunction(() => window.__POLYORAMA_DIAGNOSTICS.cameras.find((item) => item.pane === 1)?.link === 1);
  await clickTarget({ kind: 'control', pane: 1, name: 'fit' });
  await page.waitForFunction(() => {
    const cameras = window.__POLYORAMA_DIAGNOSTICS.cameras;
    const primary = cameras.find((item) => item.pane === 1)?.camera;
    const linked = cameras.find((item) => item.pane === 2)?.camera;
    return primary?.pixels_per_screen_point < 512 && JSON.stringify(primary) === JSON.stringify(linked);
  });
  await observe('million_row_scroll', async () => {
    const point = await targetPoint({ kind: 'results_scroll' });
    await page.mouse.move(point.x, point.y); await page.mouse.wheel(0, 1800);
  });
  await clickTarget({ kind: 'tabs', pane: 6 });
  await page.waitForFunction(() => {
    const virtualisation = window.__POLYORAMA_DIAGNOSTICS.virtualisation;
    return virtualisation.thumbnail_content_height > virtualisation.thumbnail_viewport_height
      && virtualisation.visible_thumbnails[1] > virtualisation.visible_thumbnails[0];
  }, null, { timeout: 10_000 });
  const thumbnailBeforeScroll = await semanticSnapshot();
  await observe('thumbnail_scroll', async () => {
    const point = await targetPoint({ kind: 'thumbnail_scroll' });
    await page.mouse.move(point.x, point.y); await page.waitForTimeout(300);
    for (let index = 0; index < 5; index += 1) {
      await page.mouse.wheel(0, 300); await page.waitForTimeout(50);
    }
  });
  await page.waitForFunction((initialStart) => {
    const range = window.__POLYORAMA_DIAGNOSTICS.virtualisation.visible_thumbnails;
    return range[0] > initialStart;
  }, thumbnailBeforeScroll.virtualisation.visible_thumbnails[0], { timeout: 10_000 });
  const thumbnailAfterScroll = await semanticSnapshot();
  const laterVisibleStart = thumbnailAfterScroll.virtualisation.visible_thumbnails[0];
  const laterDemand = thumbnailAfterScroll.visible_tile_keys.find(
    (key) => key.source === 2 && key.x >= laterVisibleStart,
  );
  if (thumbnailAfterScroll.virtualisation.thumbnail_scroll_offset_y
        <= thumbnailBeforeScroll.virtualisation.thumbnail_scroll_offset_y
      || thumbnailAfterScroll.physical_wheel_events <= thumbnailBeforeScroll.physical_wheel_events
      || !laterDemand) {
    throw new Error(`physical thumbnail wheel did not advance scroll state and demand: ${JSON.stringify(thumbnailAfterScroll.virtualisation)}`);
  }
  await page.waitForFunction((minimumKey) => window.__POLYORAMA_HANDLE.test_snapshot()
    .thumbnail_resident_keys.some((key) => key.source === 2 && key.x >= minimumKey), laterVisibleStart, { timeout: 10_000 });
  await page.screenshot({ path: join(evidenceRoot, 'browser-thumbnails.png') });
  const thumbnailSemantic = await semanticSnapshot();
  if (!thumbnailSemantic.thumbnail_resident_keys.length
      || thumbnailSemantic.thumbnail_resident_keys.length > 256) {
    throw new Error(`decoded thumbnail cache is empty or unbounded: ${thumbnailSemantic.thumbnail_resident_keys.length}`);
  }
  const visibleCount = thumbnailSemantic.virtualisation.visible_thumbnails[1]
    - thumbnailSemantic.virtualisation.visible_thumbnails[0];
  const materialisedBound = visibleCount + 4 * thumbnailSemantic.virtualisation.thumbnail_columns;
  if (thumbnailSemantic.virtualisation.materialised_thumbnails > materialisedBound
      || thumbnailSemantic.virtualisation.thumbnail_cache_bytes > 4 * 1024 * 1024) {
    throw new Error(`thumbnail presentation exceeded its materialisation/cache bound: ${JSON.stringify(thumbnailSemantic.virtualisation)}`);
  }
  semanticEvidence.thumbnail_cache = {
    resident_keys: thumbnailSemantic.thumbnail_resident_keys,
    resident_count: thumbnailSemantic.thumbnail_resident_keys.length,
    configured_item_bound: 256,
    before_scroll: thumbnailBeforeScroll.virtualisation,
    after_scroll: thumbnailSemantic.virtualisation,
    later_demanded_key: laterDemand,
    later_resident: thumbnailSemantic.thumbnail_resident_keys.find(
      (key) => key.source === 2 && key.x >= laterVisibleStart,
    ),
    physical_wheel_events_before: thumbnailBeforeScroll.physical_wheel_events,
    physical_wheel_events_after: thumbnailAfterScroll.physical_wheel_events,
    semantic_scroll_rect: thumbnailAfterScroll.ui_geometry.thumbnail_scroll,
  };
  await observe('polygon_editing', async () => {
    await clickTarget({ kind: 'control', pane: 1, name: 'polygon' });
    const first = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.2, 0.2);
    const second = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.65, 0.25);
    const third = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.4, 0.65);
    await page.mouse.click(first.x, first.y); await page.mouse.click(second.x, second.y); await page.mouse.click(third.x, third.y);
    await page.mouse.click(third.x, third.y, { button: 'right' });
    const beforeEdit = await semanticSnapshot();
    const annotation = beforeEdit.annotations.find(
      (candidate) => candidate.id === beforeEdit.selected_annotation,
    );
    const camera = beforeEdit.cameras.find((candidate) => candidate.pane === 1)?.camera;
    if (!annotation || !camera) throw new Error('physical polygon was not available for vertex editing');
    const originalVertex = annotation.vertices[0];
    await clickTarget({ kind: 'control', pane: 1, name: 'edit_vertex' });
    const editStart = await targetPoint({ kind: 'image_viewports', pane: 1 }, 0.2, 0.2);
    await page.mouse.move(editStart.x, editStart.y); await page.mouse.down();
    await page.mouse.move(editStart.x + 35, editStart.y + 25, { steps: 5 }); await page.mouse.up();
    const afterEdit = await semanticSnapshot();
    const editedVertex = afterEdit.annotations.find(
      (candidate) => candidate.id === annotation.id,
    )?.vertices[0];
    const expectedVertex = {
      x: originalVertex.x + 35 * camera.pixels_per_screen_point * 2,
      y: originalVertex.y - 25 * camera.pixels_per_screen_point * 2,
    };
    if (!editedVertex
        || Math.abs(editedVertex.x - expectedVertex.x) > 0.01
        || Math.abs(editedVertex.y - expectedVertex.y) > 0.01
        || afterEdit.undo_depth !== beforeEdit.undo_depth + 1) {
      throw new Error(`physical vertex edit did not commit its release position: ${JSON.stringify({ originalVertex, editedVertex, expectedVertex, undoBefore: beforeEdit.undo_depth, undoAfter: afterEdit.undo_depth })}`);
    }
    await clickTarget({ kind: 'control', name: 'undo' });
    const afterUndo = await semanticSnapshot();
    const undoneVertex = afterUndo.annotations.find(
      (candidate) => candidate.id === annotation.id,
    )?.vertices[0];
    await clickTarget({ kind: 'control', name: 'redo' });
    const afterRedo = await semanticSnapshot();
    const redoneVertex = afterRedo.annotations.find(
      (candidate) => candidate.id === annotation.id,
    )?.vertices[0];
    if (JSON.stringify(undoneVertex) !== JSON.stringify(originalVertex)
        || JSON.stringify(redoneVertex) !== JSON.stringify(editedVertex)) {
      throw new Error('physical vertex edit did not round-trip through exact undo/redo');
    }
    semanticEvidence.physical_vertex_edit = {
      screen_delta: { x: 35, y: 25 },
      original_vertex: originalVertex,
      expected_vertex: expectedVertex,
      committed_vertex: editedVertex,
      undo_depth_before: beforeEdit.undo_depth,
      undo_depth_after: afterEdit.undo_depth,
      undo_restored_original: true,
      redo_restored_edit: true,
      release_frame_preview_regression: 'covered by deterministic Rust frame-output test',
    };
  });
  await page.screenshot({ path: join(evidenceRoot, 'browser-polygon.png') });
  const splitterBefore = await semanticSnapshot();
  const splitterRectBefore = splitterBefore.ui_geometry.splitters.find((item) => item.node === 1)?.rect;
  if (!splitterRectBefore) throw new Error('primary splitter geometry is unavailable before resize');
  const splitterTrace = [];
  await observe('dock_splitter_interaction', async () => {
    const start = await targetPoint({ kind: 'splitter', node: 1 }, 0.5, 0.25);
    await page.mouse.move(start.x, start.y); await page.mouse.down();
    for (let step = 1; step <= 6; step += 1) {
      await page.mouse.move(start.x - 47 * step / 6, start.y);
      await page.waitForTimeout(30);
      const snapshot = await semanticSnapshot();
      const rect = snapshot.ui_geometry.splitters.find((item) => item.node === 1)?.rect;
      splitterTrace.push(rect ? (rect.min_x + rect.max_x) * 0.5 : null);
    }
    await page.mouse.up();
  });
  const splitterAfter = await semanticSnapshot();
  const splitterRectAfter = splitterAfter.ui_geometry.splitters.find((item) => item.node === 1)?.rect;
  const splitterCentreBefore = (splitterRectBefore.min_x + splitterRectBefore.max_x) * 0.5;
  const splitterCentreAfter = (splitterRectAfter?.min_x + splitterRectAfter?.max_x) * 0.5;
  const splitterPreviewTracked = splitterTrace.length === 6 && splitterTrace.every(
    (centre, index) => Math.abs(centre - (splitterCentreBefore - 47 * (index + 1) / 6)) <= 1,
  );
  if (!splitterRectAfter
      || splitterAfter.workspace_hash === splitterBefore.workspace_hash
      || !splitterPreviewTracked
      || Math.abs(splitterCentreAfter - (splitterCentreBefore - 47)) > 1
      || splitterAfter.undo_depth !== splitterBefore.undo_depth + 1) {
    throw new Error(`physical splitter resize did not commit its final displacement: ${JSON.stringify({ splitterRectBefore, splitterRectAfter, splitterCentreBefore, splitterCentreAfter, splitterTrace, hashBefore: splitterBefore.workspace_hash, hashAfter: splitterAfter.workspace_hash, undoBefore: splitterBefore.undo_depth, undoAfter: splitterAfter.undo_depth })}`);
  }
  await clickTarget({ kind: 'control', name: 'undo' });
  await page.waitForFunction(
    (before) => window.__POLYORAMA_HANDLE.test_snapshot().workspace_hash === before,
    splitterBefore.workspace_hash,
  );
  await clickTarget({ kind: 'control', name: 'redo' });
  await page.waitForFunction(
    (after) => window.__POLYORAMA_HANDLE.test_snapshot().workspace_hash === after,
    splitterAfter.workspace_hash,
  );
  const splitterNoOpBefore = await semanticSnapshot();
  const splitterNoOpStart = await targetPoint({ kind: 'splitter', node: 1 }, 0.5, 0.25);
  await page.mouse.move(splitterNoOpStart.x, splitterNoOpStart.y); await page.mouse.down();
  await page.mouse.move(splitterNoOpStart.x - 30, splitterNoOpStart.y, { steps: 4 });
  await page.mouse.move(splitterNoOpStart.x, splitterNoOpStart.y, { steps: 4 }); await page.mouse.up();
  await page.waitForTimeout(150);
  const splitterNoOpAfter = await semanticSnapshot();
  if (splitterNoOpAfter.workspace_hash !== splitterNoOpBefore.workspace_hash
      || splitterNoOpAfter.undo_depth !== splitterNoOpBefore.undo_depth) {
    throw new Error('physical splitter drag returning to origin created workspace or history state');
  }
  semanticEvidence.physical_splitter_resize = {
    pointer_delta_x: -47,
    splitter_centre_before: splitterCentreBefore,
    splitter_centre_after: splitterCentreAfter,
    splitter_preview_trace: splitterTrace,
    preview_tracked_pointer: splitterPreviewTracked,
    workspace_hash_before: splitterBefore.workspace_hash,
    workspace_hash_after: splitterAfter.workspace_hash,
    undo_depth_before: splitterBefore.undo_depth,
    undo_depth_after: splitterAfter.undo_depth,
    undo_restored_original: true,
    redo_restored_resize: true,
    out_and_back_no_op: true,
  };
  await observe('dock_pane_drag', async () => {
    const source = await targetPoint({ kind: 'tabs', pane: 4 });
    const target = await targetPoint({ kind: 'rightmost_pane_body' }, 0.5, 0.25);
    await page.mouse.move(source.x, source.y); await page.mouse.down(); await page.waitForTimeout(150);
    await page.mouse.move(target.x, target.y, { steps: 12 }); await page.waitForTimeout(150); await page.mouse.up();
  });
  await page.screenshot({ path: join(evidenceRoot, 'browser-rearranged-dock.png') });
  await clickTarget({ kind: 'control', name: 'save_layout' });
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
  const beforeUnavailable = await semanticSnapshot();
  const unavailable = await semanticAction({ kind: 'make_worker_unavailable' });
  if (unavailable.runtime.worker_health !== 'Unavailable'
      || unavailable.runtime.failed <= beforeUnavailable.runtime.failed
      || unavailable.runtime.queued !== 0
      || unavailable.runtime.in_flight !== 0
      || unavailable.runtime.worker_queue_depth !== 0
      || unavailable.runtime.browser_credits_in_use !== 0) {
    throw new Error(`unavailable Worker did not fail closed: ${JSON.stringify(unavailable.runtime)}`);
  }
  await page.waitForTimeout(500);
  const unavailableIdleFrame = (await semanticSnapshot()).frame_number;
  await page.waitForTimeout(700);
  const unavailableIdleAfter = (await semanticSnapshot()).frame_number;
  if (unavailableIdleAfter !== unavailableIdleFrame) {
    throw new Error(`unavailable Worker state repainted continuously (${unavailableIdleFrame} -> ${unavailableIdleAfter})`);
  }
  semanticEvidence.worker_unavailable = {
    health: unavailable.runtime.worker_health,
    failed_before: beforeUnavailable.runtime.failed,
    failed_after: unavailable.runtime.failed,
    queued: unavailable.runtime.queued,
    in_flight: unavailable.runtime.in_flight,
    external_queue: unavailable.runtime.worker_queue_depth,
    browser_credits_in_use: unavailable.runtime.browser_credits_in_use,
    frame_before_idle: unavailableIdleFrame,
    frame_after_idle: unavailableIdleAfter,
  };
  await writeFile(join(evidenceRoot, 'browser-semantic.json'), JSON.stringify(semanticEvidence, null, 2));
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
  if (browserProcess.exitCode === null) {
    browserProcess.kill('SIGTERM');
    await Promise.race([once(browserProcess, 'exit'), new Promise((resolve) => setTimeout(resolve, 2_000))]);
  }
  await rm(browserProfile, { recursive: true, force: true });
  await new Promise((resolve) => server.close(resolve));
}
