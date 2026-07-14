import { useEditorTabStore } from './editorTabStore';

export function isEditorGroupLocked(groupId: string): boolean {
  return useEditorTabStore.getState().getPlacement(groupId).locked === true;
}

/** Locked groups allow in-group tab reorder only. */
export function canMoveTabsAcrossEditorGroups(sourceGroupId: string, targetGroupId: string): boolean {
  if (sourceGroupId === targetGroupId) return true;
  if (isEditorGroupLocked(sourceGroupId) || isEditorGroupLocked(targetGroupId)) return false;
  return true;
}

export function canMergeEditorGroup(sourceGroupId: string, targetGroupId: string): boolean {
  if (sourceGroupId === targetGroupId) return false;
  return canMoveTabsAcrossEditorGroups(sourceGroupId, targetGroupId);
}
