import type { DetailFocus } from '@/features/core/editor';
import { useEditorStore } from '@/features/core/editor';
import { setVariablesGraphScopeFromResource } from '@/features/core/editor/detail/variablesGraphScope';
import { revealWorkbenchView } from '@/features/application/layout/workbenchLayoutActions';

export function setDetailContext(focus: DetailFocus | null): void {
  const store = useEditorStore.getState();
  if (focus) store.setDetailFocus(focus);
  else store.clearDetailFocus();

  if (focus?.kind === 'event' || focus?.kind === 'function') {
    setVariablesGraphScopeFromResource(focus.path);
  }
}

export function setInspectionContext(
  graphPath: string,
  selectedNodeIds: readonly string[],
): void {
  const store = useEditorStore.getState();
  const [nodeId] = selectedNodeIds;
  if (selectedNodeIds.length === 1 && graphPath.length > 0 && nodeId?.length > 0) {
    store.setDetailFocus({ kind: 'node', id: nodeId, graphPath });
  } else if (store.detailFocus?.kind === 'node') {
    store.clearDetailFocus();
  }
}

export async function revealDetails(focus: DetailFocus): Promise<void> {
  setDetailContext(focus);
  await revealWorkbenchView('details');
}

export async function revealInspect(
  graphPath: string,
  selectedNodeIds: readonly string[],
): Promise<void> {
  setInspectionContext(graphPath, selectedNodeIds);
  const [nodeId] = selectedNodeIds;
  if (selectedNodeIds.length !== 1 || graphPath.length === 0 || !nodeId) return;
  await revealWorkbenchView('inspect');
}
