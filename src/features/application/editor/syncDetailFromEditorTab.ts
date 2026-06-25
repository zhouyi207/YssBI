import { useEditorStore } from '@/features/core/editor';
import type { LayoutTab } from '@/shared/types/ui';
import { ensureDetailVisible } from './ensureDetailVisible';

/**
 * Keep Detail panel selection aligned with the active editor tab.
 */
export function syncDetailFromEditorTab(tab: LayoutTab | undefined): void {
  if (!tab) {
    useEditorStore.getState().setSelectedInfo(null, null);
    return;
  }

  switch (tab.type) {
    case 'worksheet':
      useEditorStore.getState().setSelectedInfo(tab.id, 'worksheet');
      ensureDetailVisible();
      break;
    case 'event':
    case 'function':
      useEditorStore.getState().setSelectedInfo(tab.id, tab.type);
      break;
    default:
      useEditorStore.getState().setSelectedInfo(null, null);
      break;
  }
}
