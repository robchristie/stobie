import { performance } from 'node:perf_hooks';
import { ApplicationClient } from './application-client.mjs';

// Compatibility adapter for the retained read-only Record Desk physical journey.
// Exact identities remain preferred; action-only fallback requires uniqueness.
export async function waitForClickTarget(readSnapshot, wait, id) {
  const started = performance.now();
  let elapsed = 0;
  let state;
  const selected = {};
  const scheduler = { now: () => Math.max(elapsed, Math.floor(performance.now() - started)),
    sleep: async ms => { await wait(ms); elapsed += ms; } };
  const client = new ApplicationClient(null, { scheduler });
  client.observe = async () => {
    state = await readSnapshot();
    delete selected.id;
    delete selected.capability;
    if (state.ui.nodes.some(node => node.id === id) || !state.ui.nodes.some(node => node.actions.includes(id))) selected.id = id;
    else selected.capability = id;
    // Legacy unit fixtures predate viewport scale; real application snapshots
    // publish it. The compatibility adapter alone supplies their 1× default.
    return { id: state.ui.frame ?? 0, snapshot: { pixels_per_point: 1, ...state.ui } };
  };
  try {
    const result = await client.target(selected);
    return { state, node: result.node, root: result.root, observed_ms: result.observed_ms };
  } catch (error) {
    if (error.code === 'timeout') throw new Error(`Click target ${id} did not become enabled with stable geometry within 15000 ms`, { cause: error });
    if (error.code === 'invalid_geometry') throw new Error(`Invalid target geometry for ${id}: ${error.message}`, { cause: error });
    throw error;
  }
}
