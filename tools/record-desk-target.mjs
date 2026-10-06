// Read-only synchronisation for the physical Record Desk browser journey.
// Popup submission and placement may span several presentation passes.
export async function waitForClickTarget(readSnapshot, wait, id) {
  let previous;
  let confirmations = 0;
  for (let elapsed = 0; elapsed <= 15000; elapsed += 50) {
    const state = await readSnapshot();
    const nodes = state.ui.nodes;
    const node = nodes.find(n => n.id === id) ?? nodes.find(n => n.actions.includes(id));
    if (node?.enabled) {
      const root = nodes.find(n => n.id === state.ui.root);
      for (const [name, rect] of [['target', node.rect], ['root', root?.rect]]) {
        if (!rect || !['min_x', 'min_y', 'max_x', 'max_y'].every(k => Number.isFinite(rect[k]))
          || rect.max_x <= rect.min_x || rect.max_y <= rect.min_y) {
          throw new Error(`Invalid ${name} geometry for ${id}`);
        }
      }
      const current = JSON.stringify([node.id, node.rect, root.rect]);
      confirmations = current === previous ? confirmations + 1 : 1;
      if (confirmations === 3) return { state, node, root, observed_ms: elapsed };
      previous = current;
    } else {
      previous = undefined;
      confirmations = 0;
    }
    if (elapsed < 15000) await wait(50);
  }
  throw new Error(`Click target ${id} did not become enabled with stable geometry within 15000 ms`);
}
