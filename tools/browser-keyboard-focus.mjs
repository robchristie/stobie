/** Observe the existing browser traversal, including its final allowed action. */
export async function focusByTab(destination, { target, pressTab, waitForTab, wait, maxTabs = 45, settleMs = 60 }) {
  for (let attempt = 0; attempt < maxTabs; attempt++) {
    const observation = await target(destination);
    if (observation.node.focused) return;
    await pressTab();
    // A fixed delay can leave the Tab queued behind a slow application frame.
    // Do not send another key until this traversal has reached egui.
    await waitForTab(observation.tab_input_epoch);
    await wait(settleMs);
  }
  // Earlier pre-action reads observe the preceding Tab; the last Tab needs one too.
  if ((await target(destination)).node.focused) return;
  throw new Error(`Tab did not reach ${destination}`);
}

export const focusNavigationDestination = focusByTab;
