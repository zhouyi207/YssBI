import { useEditorStore } from '../stores/useEditorStore';
import type { DetailFocus } from './types';

export function clearDetailFocusForClosedTab(tabId: string, tabType?: string): void {
  const focus = useEditorStore.getState().detailFocus;
  if (!focus) return;

  if (shouldClearFocus(focus, tabId, tabType)) {
    useEditorStore.getState().clearDetailFocus();
  }
}

function shouldClearFocus(focus: DetailFocus, tabId: string, tabType?: string): boolean {
  if (focus.kind === 'node' && focus.graphId === tabId) return true;
  if (focus.kind === 'event' || focus.kind === 'function' || focus.kind === 'worksheet') {
    return focus.id === tabId;
  }
  if (!tabType) return false;
  if (tabType === 'event' || tabType === 'function' || tabType === 'worksheet') {
    return focus.kind === tabType && focus.id === tabId;
  }
  return false;
}
