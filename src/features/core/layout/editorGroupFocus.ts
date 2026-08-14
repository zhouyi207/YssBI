import {
  editorDockviewPort,
  type DockviewGroupInfo,
  type DockviewPanelInfo,
} from '@/features/core/dockview';
import { readEditorPartOptions } from './editorPartOptions';

const MAX_RECENT_EDITOR_GROUPS = 12;
let recentGroupIds: string[] = editorDockviewPort.getActiveGroupId()
  ? [editorDockviewPort.getActiveGroupId()!]
  : [];

editorDockviewPort.subscribe(() => {
  const activeGroupId = editorDockviewPort.getActiveGroupId();
  if (!activeGroupId) return;
  recentGroupIds = [
    activeGroupId,
    ...recentGroupIds.filter((groupId) => groupId !== activeGroupId),
  ].slice(0, MAX_RECENT_EDITOR_GROUPS);
});

/** VS Code MRU — next group to focus when closing the active empty group. */
export function getNextActiveEditorGroupId(excludeGroupId?: string): string | null {
  const groups = editorDockviewPort.listGroups();
  const groupIds = new Set(groups.map(({ groupId }: DockviewGroupInfo) => groupId));
  for (const groupId of recentGroupIds) {
    if (groupId !== excludeGroupId && groupIds.has(groupId)) return groupId;
  }
  return groups.find(({ groupId }: DockviewGroupInfo) => groupId !== excludeGroupId)?.groupId ?? null;
}

/** Pre-activate MRU group before removing the last tab (VS Code `doCloseActiveEditor`). */
export function prepareActiveGroupBeforeLastTabClose(groupId: string): string | null {
  if (!readEditorPartOptions().closeEmptyGroups) return null;
  const groups = editorDockviewPort.listGroups();
  if (groups.length <= 1) return null;
  const groupPanelCount = editorDockviewPort
    .listPanels()
    .filter((panel: DockviewPanelInfo) => panel.groupId === groupId)
    .length;
  if (groupPanelCount !== 1) return null;

  const nextGroupId = getNextActiveEditorGroupId(groupId);
  if (!nextGroupId) return null;

  const nextGroup = groups.find(
    ({ groupId: candidateId }: DockviewGroupInfo) => candidateId === nextGroupId,
  );
  if (!nextGroup?.activePanelInstanceId) return null;
  void editorDockviewPort.activate(nextGroup.activePanelInstanceId);
  return nextGroupId;
}
