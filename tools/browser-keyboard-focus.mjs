// Tab focus from wherever it is to one control, observing every step.
//
// The walk needs no count of the controls in front of the target: egui's Tab
// order is a cycle, so the walk fails once focus returns to an observed
// control it visited earlier (the target is not in the cycle), or when focus
// stays on one observed control for `stallLimit` steps in a row. Focus on a
// control outside the observed tree (such as an application's own chrome), or
// on none during a wrap between cycles, reports no ID; such steps neither
// stall nor count. `safetyLimit` only stops a walk that never revisits an
// observed control, such as one through controls created as it goes.

/** The Tab steps focus may stay on one observed control before the walk fails. */
export const STALL_LIMIT = 8;
/** The most Tab steps any walk takes. */
export const SAFETY_LIMIT = 1024;

/**
 * Press Tab until `destination` is focused. `target(destination)` observes the
 * application and returns `{ node, focused_id, tab_input_epoch }`: the target's
 * node, the ID of the control that has focus (or null) and the Tab receipt
 * counter.
 */
export async function focusByTab(destination, {
  target, pressTab, waitForTab, wait, settleMs = 60,
  stallLimit = STALL_LIMIT, safetyLimit = SAFETY_LIMIT,
}) {
  const visited = [];
  let previous = null;
  let stalled = 0;
  for (let step = 0; ; step++) {
    const observation = await target(destination);
    if (observation.node.focused) return;
    const focused = observation.focused_id ?? null;
    if (focused !== null && focused === previous) {
      if (++stalled >= stallLimit) {
        throw new Error(`Tab did not reach ${destination}: focus stayed on ${focused} for ${stallLimit} steps after ${visited.length} controls`);
      }
    } else {
      stalled = 0;
      if (focused !== null && visited.includes(focused)) {
        throw new Error(`Tab did not reach ${destination}: focus returned to ${focused} after ${visited.length} controls (${visited.join(', ')})`);
      }
      if (focused !== null) visited.push(focused);
    }
    previous = focused;
    if (step >= safetyLimit) throw new Error(`Tab did not reach ${destination} within ${safetyLimit} steps`);
    await pressTab();
    // A fixed delay can leave the Tab queued behind a slow application frame.
    // Do not send another key until this traversal has reached egui.
    await waitForTab(observation.tab_input_epoch);
    await wait(settleMs);
  }
}

/** The ID of the focused node in `nodes`, or null. */
export function focusedId(nodes) {
  return nodes.find(node => node.focused)?.id ?? null;
}

export const focusNavigationDestination = focusByTab;
