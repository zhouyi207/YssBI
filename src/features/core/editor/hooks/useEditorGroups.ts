/**
 * Stable editor group ids for shared session context.
 * Volatile tab/selection state lives in Dockview and pane state — use useEditorGroupWorkspace instead.
 */

import { useMemo } from 'react';
import type { EditorGroupSnapshot } from '@/shared/types';
import { editorDockviewPort, useDockviewPortSnapshot } from '@/features/core/dockview';

export function useEditorGroups(): EditorGroupSnapshot[] {
  const { revision } = useDockviewPortSnapshot(editorDockviewPort);

  return useMemo(
    () => editorDockviewPort.listGroups().map(({ groupId }): EditorGroupSnapshot => ({ id: groupId })),
    [revision],
  );
}
