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
