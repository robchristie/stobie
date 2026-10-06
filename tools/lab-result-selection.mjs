// The physical Lab journey keeps the clicked identity when the selection action
// bar changes the scroll clip and removes that row from rendered observations.
export function resultSelectionConfirmed(expectedResult, snapshot = window.__POLYORAMA_HANDLE.test_snapshot()) {
  const nodes = snapshot.ui_snapshot.nodes;
  const selection = nodes.find(node => node.id === 'pane.7.selection')?.domain_reference;
  const viewport = nodes.find(node => node.id === 'pane.1.viewport');
  return Number.isSafeInteger(expectedResult) && expectedResult >= 0
    && selection?.kind === 'result' && selection.value === expectedResult
    && viewport?.description?.split('; ')
      .includes(`selected result: result ${expectedResult}`)
    && nodes.filter(node => node.role === 'result_row' && node.selected)
      .every(node => node.domain_reference?.kind === 'result'
        && node.domain_reference.value === expectedResult);
}

const RECT_FIELDS = ['min_x', 'min_y', 'max_x', 'max_y'];
const CANVAS_FIELDS = ['x', 'y', 'width', 'height'];
const equalFields = (a, b, fields) => fields.every(field => a[field] === b[field]);

function resultTarget(snapshot, canvas, row) {
  if (!row) return null;
  const root = snapshot.ui_snapshot.nodes.find(node => node.id === snapshot.ui_snapshot.root)?.rect
    ?? snapshot.ui_geometry.root;
  const scroll = snapshot.ui_geometry.results_scroll;
  for (const [name, rect] of [['root', root], ['scroll', scroll], ['row', row.rect]]) {
    if (!rect || !RECT_FIELDS.every(field => Number.isFinite(rect[field]))
        || rect.max_x <= rect.min_x || rect.max_y <= rect.min_y) {
      throw new Error(`Invalid Results ${name} geometry`);
    }
  }
  if (!canvas || !CANVAS_FIELDS.every(field => Number.isFinite(canvas[field]))
      || canvas.width <= 0 || canvas.height <= 0) throw new Error('Invalid Results canvas transform');
  if (!Number.isSafeInteger(row.result) || row.result < 0
      || !Number.isSafeInteger(snapshot.frame_number) || snapshot.frame_number < 0
      || !Number.isSafeInteger(snapshot.physical_wheel_events) || snapshot.physical_wheel_events < 0) {
    throw new Error('Invalid Results target identity/frame/input observation');
  }
  const logicalX = (row.rect.min_x + row.rect.max_x) * 0.5;
  const logicalY = (row.rect.min_y + row.rect.max_y) * 0.5;
  if (logicalY < scroll.min_y || logicalY > scroll.max_y) return null;
  const x = canvas.x + (logicalX - root.min_x) * canvas.width / (root.max_x - root.min_x);
  const y = canvas.y + (logicalY - root.min_y) * canvas.height / (root.max_y - root.min_y);
  if (x < canvas.x || x > canvas.x + canvas.width || y < canvas.y || y > canvas.y + canvas.height) {
    throw new Error('Results target fell outside canvas');
  }
  return {
    result: row.result,
    frame: snapshot.frame_number,
    physical_wheel_events: snapshot.physical_wheel_events,
    visible_rows: [...snapshot.virtualisation.visible_rows],
    rect: { ...row.rect },
    scroll_rect: { ...scroll },
    root_rect: { ...root },
    canvas: { ...canvas },
    x,
    y,
    hit_height: row.rect.max_y - row.rect.min_y,
    clip_edge_distances: {
      top: logicalY - scroll.min_y,
      bottom: scroll.max_y - logicalY,
      left: logicalX - scroll.min_x,
      right: scroll.max_x - logicalX,
    },
  };
}

// Preserve the journey's explicit selector: the first observed row whose centre
// is inside the current scroll clip. Rows may already have clipped hit bounds.
export function firstResultRowTarget(snapshot, canvas) {
  const scroll = snapshot.ui_geometry.results_scroll;
  const row = snapshot.ui_geometry.result_rows.find(item => {
    const centre = (item.rect.min_y + item.rect.max_y) * 0.5;
    return scroll && centre >= scroll.min_y && centre <= scroll.max_y;
  });
  return resultTarget(snapshot, canvas, row);
}

export function resolveResultRowTarget(snapshot, canvas, result) {
  return resultTarget(snapshot, canvas,
    snapshot.ui_geometry.result_rows.find(row => row.result === result));
}

export function sameResultTarget(a, b) {
  return Boolean(a && b && a.result === b.result
    && equalFields(a.rect, b.rect, RECT_FIELDS)
    && equalFields(a.scroll_rect, b.scroll_rect, RECT_FIELDS)
    && equalFields(a.root_rect, b.root_rect, RECT_FIELDS)
    && equalFields(a.canvas, b.canvas, CANVAS_FIELDS));
}

// Wait for the wheel to be observed and its Results geometry to change, then
// establish 200 ms of stable targeting geometry. Tile/diagnostic frames may
// continue; the whole application need not be idle. The fixed budget fails
// continuous motion rather than dispatching at a moving or missing target.
export async function waitForStableResultTarget(readTarget, wait, beforeWheel, now = () => performance.now()) {
  const observations = [];
  const started = now();
  let previous = null;
  let stableSince = null;
  while (true) {
    const current = await readTarget();
    const observedMs = now() - started;
    observations.push({ observed_ms: observedMs, target: current });
    // Include asynchronous snapshot/geometry reads in the deadline and never
    // accept a target returned after that deadline, even if its pose is stable.
    if (observedMs >= 3500) break;
    const wheelProcessed = current && current.physical_wheel_events > beforeWheel.physical_wheel_events;
    const resultsMoved = current && (current.result !== beforeWheel.result
      || !equalFields(current.rect, beforeWheel.rect, RECT_FIELDS)
      || current.visible_rows.some((value, index) => value !== beforeWheel.visible_rows[index]));
    if (!wheelProcessed || !resultsMoved) stableSince = null;
    else if (stableSince === null || !sameResultTarget(previous, current)) stableSince = observedMs;
    const stableMs = stableSince === null ? 0 : observedMs - stableSince;
    if (stableMs >= 200) return { target: current, stable_ms: stableMs, observed_ms: observedMs, observations };
    previous = current;
    await wait(Math.min(50, 3500 - observedMs));
  }
  const error = new Error('Results target did not settle within 3500 ms after the physical wheel');
  error.observations = observations;
  throw error;
}

// Resolve the accepted domain identity once more immediately before dispatch.
// A moved/missing target fails before input; never substitute or click again.
export function assertFrozenResultTarget(frozen, current) {
  if (!sameResultTarget(frozen, current)) {
    throw new Error(`Frozen Results target ${frozen.result} moved or disappeared before click`);
  }
}
