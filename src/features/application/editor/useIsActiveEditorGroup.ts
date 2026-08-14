import { useContext } from 'react';
import { GroupContext } from '@/features/core/editor';
import { editorDockviewPort, useDockviewPortSnapshot } from '@/features/core/dockview';

/** True when the given editor group (or GroupContext group) is the active editor group. */
export function useIsActiveEditorGroup(groupId?: string | null): boolean {
  const contextGroupId = useContext(GroupContext);
  const resolvedGroupId = groupId ?? contextGroupId;
  useDockviewPortSnapshot(editorDockviewPort);
  return resolvedGroupId != null && editorDockviewPort.getActiveGroupId() === resolvedGroupId;
}
