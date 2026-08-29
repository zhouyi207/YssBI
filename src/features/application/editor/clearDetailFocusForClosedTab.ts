import { useEditorStore } from '@/features/core/editor/stores/useEditorStore';
import type { DetailFocus } from '@/shared/types/ui/detail';

export function clearDetailFocusForClosedTab(tabId: string): void {
  const focus = useEditorStore.getState().detailFocus;
  if (!focus) return;

  if (shouldClearFocus(focus, tabId)) {
    useEditorStore.getState().clearDetailFocus();
  }
}

function shouldClearFocus(focus: DetailFocus, tabId: string): boolean {
  if (focus.kind === 'node' && focus.graphPath === tabId) return true;
  if (focus.kind === 'event' || focus.kind === 'function') {
    return focus.path === tabId;
  }
  if (focus.kind === 'worksheet') {
    return focus.worksheetPath === tabId;
  }
  return false;
}
