import { useCallback } from 'react';

import { openGraphInEditor } from './openGraphInEditor';
import { splitEditorAtEdge } from './editorGroupCommands';

/** Tab Management Hook — thin React facade over canonical editor commands. */
export function useTabManagement() {
  const openGraph = useCallback(async (
    id: string,
    name: string,
    type: 'event' | 'function',
    options?: { pinned?: boolean; targetGroupId?: string },
  ): Promise<void> => {
    await openGraphInEditor(
      id,
      name,
      type,
      options?.targetGroupId,
      { pinned: options?.pinned },
    );
  }, []);

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    void splitEditorAtEdge(sourceGroupId, 'right');
  }, []);

  return { openGraph, splitEditorRight };
}
