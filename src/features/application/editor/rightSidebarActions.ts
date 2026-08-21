import type { DetailFocus } from '@/features/core/editor';
import { useEditorStore } from '@/features/core/editor';
import { setVariablesGraphScopeFromResource } from '@/features/core/editor/detail/variablesGraphScope';
import { ensureDetailVisible } from './ensureDetailVisible';

export function focusDetails(focus: DetailFocus): void {
  const store = useEditorStore.getState();
  store.setDetailFocus(focus);
  store.setRightSidebarTab('details');
  if (focus.kind === 'event' || focus.kind === 'function') {
    setVariablesGraphScopeFromResource(focus.path);
  }
  ensureDetailVisible();
}

export function focusCanvasSelection(
  graphPath: string,
  selectedNodeIds: readonly string[],
): void {
  const store = useEditorStore.getState();
  if (selectedNodeIds.length === 1) {
    store.setDetailFocus({ kind: 'node', id: selectedNodeIds[0], graphPath });
  } else if (store.detailFocus?.kind === 'node') {
    store.clearDetailFocus();
  }
  store.setRightSidebarTab('inspect');
  ensureDetailVisible();
}

export function focusResultSidebar(): void {
  useEditorStore.getState().setRightSidebarTab('result');
  ensureDetailVisible();
}
