/** Observe the existing browser traversal, including its final allowed action. */
export async function focusNavigationDestination(destination, { target, pressTab, wait }) {
  for (let attempt = 0; attempt < 45; attempt++) {
    if ((await target(destination)).node.focused) return;
    await pressTab();
    await wait(60);
  }
  // Earlier pre-action reads observe the preceding Tab; the last Tab needs one too.
  if ((await target(destination)).node.focused) return;
  throw new Error(`Tab did not reach ${destination}`);
}
