/**
 * Stable editor group ids for shared session context.
 * Volatile tab/selection state lives in Dockview and pane state — use useEditorGroupWorkspace instead.
 */

import { useMemo } from 'react';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import type { EditorGroupSnapshot } from '@/shared/types';

export function useEditorGroups(): EditorGroupSnapshot[] {
  const { revision } = useDockviewPortSnapshot(workbenchDockviewPort);

  return useMemo(() => {
    const editorGroupIds = new Set(
      workbenchDockviewPort
        .listPanels()
        .filter((panel) => panel.metadata.role === 'editor')
        .map((panel) => panel.groupId),
    );
    return workbenchDockviewPort
      .listGroups()
      .filter((group) => editorGroupIds.has(group.groupId))
      .map(({ groupId }): EditorGroupSnapshot => ({ id: groupId }));
  }, [revision]);
}
