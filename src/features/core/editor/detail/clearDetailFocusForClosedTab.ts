import { useEditorStore } from '../stores/useEditorStore';
import type { DetailFocus } from './types';

export function clearDetailFocusForClosedTab(tabId: string, _tabType?: string): void {
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
    return focus.id === tabId;
  }
  return false;
}
