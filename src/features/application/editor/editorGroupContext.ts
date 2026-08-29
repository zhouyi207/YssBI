import { createContext, useContext } from 'react';

import {
  useActiveEditorGroup as useCoreActiveEditorGroup,
  useEditorGroupWorkspace as useCoreEditorGroupWorkspace,
} from '@/features/core/editor';

/** Application-owned group selection context for editor view composition. */
export const GroupContext = createContext<string | null>(null);

export function useEditorGroupWorkspace(overrideGroupId?: string | null) {
  const contextGroupId = useContext(GroupContext);
  return useCoreEditorGroupWorkspace(overrideGroupId ?? contextGroupId);
}

export function useActiveEditorGroup(overrideGroupId?: string | null) {
  const contextGroupId = useContext(GroupContext);
  return useCoreActiveEditorGroup(overrideGroupId ?? contextGroupId);
}
