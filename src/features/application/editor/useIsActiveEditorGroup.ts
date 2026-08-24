import { useContext } from 'react';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';
import { GroupContext } from '@/features/core/editor';

/** True when the given editor group (or GroupContext group) is physically active. */
export function useIsActiveEditorGroup(groupId?: string | null): boolean {
  const contextGroupId = useContext(GroupContext);
  const resolvedGroupId = groupId ?? contextGroupId;
  useDockviewPortSnapshot(workbenchDockviewPort);
  return resolvedGroupId != null
    && workbenchDockviewPort.getActiveEditorPanel()?.groupId === resolvedGroupId;
}
