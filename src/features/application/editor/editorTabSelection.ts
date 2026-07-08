import { useLayoutStore } from '@/features/core/layout/layoutStore';

/** Update layout store active tab for an editor group (no graph session side effects). */
export function applyEditorTabSelection(groupId: string, tabId: string | null): void {
  const currentData = useLayoutStore.getState().nodes[groupId]?.data;
  if (!currentData) return;
  useLayoutStore.getState().updateNode(groupId, {
    data: {
      ...currentData,
      activeTabId: tabId || undefined,
      params:
        currentData.activeTabId === (tabId || undefined)
          ? currentData.params
          : { ...currentData.params, selectedNodeIds: [] },
    },
  });
}
