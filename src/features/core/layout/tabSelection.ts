import { useEditorTabStore } from './editorTabStore';

export function resolveTabDragTransferIds(groupId: string, draggedTabId: string): string[] {
  const placement = useEditorTabStore.getState().getPlacement(groupId);
  const selected = placement.selectedTabIds;
  if (selected.length > 1 && selected.includes(draggedTabId)) {
    return placement.tabIds.filter((tabId) => selected.includes(tabId));
  }
  return [draggedTabId];
}
