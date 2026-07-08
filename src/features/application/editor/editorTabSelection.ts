import { useLayoutStore } from '@/features/core/layout/layoutStore';

/** Update layout store active tab for an editor group (no graph session side effects). */
export function applyEditorTabSelection(groupId: string, tabId: string | null): void {
  useLayoutStore.getState().setEditorGroupActiveTab(groupId, tabId);
}
