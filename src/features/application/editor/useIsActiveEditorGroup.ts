import { useContext } from 'react';
import { GroupContext } from '@/features/core/editor';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

/** True when the given editor group (or GroupContext group) is the active editor group. */
export function useIsActiveEditorGroup(groupId?: string | null): boolean {
  const contextGroupId = useContext(GroupContext);
  const resolvedGroupId = groupId ?? contextGroupId;
  return useLayoutStore((s) => resolvedGroupId != null && s.activeEditorGroupId === resolvedGroupId);
}
