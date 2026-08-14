import { editorDockviewPort, type DockviewPanelInfo } from '@/features/core/dockview';

interface GroupLockState {
  locked?: boolean;
}

export function isEditorGroupLocked(groupId: string): boolean {
  return editorDockviewPort
    .listPanels()
    .filter((panel: DockviewPanelInfo) => panel.groupId === groupId)
    .some((panel: DockviewPanelInfo) => {
      const value = panel.tab?.data?.layoutTab;
      return value && typeof value === 'object' && (value as GroupLockState).locked === true;
    });
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
