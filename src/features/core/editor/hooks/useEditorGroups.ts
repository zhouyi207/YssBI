/**
 * Stable editor group ids for shared session context.
 * Volatile tab/selection state lives in editorTabStore — use useEditorGroupWorkspace instead.
 */

import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { EditorGroupSnapshot } from '@/shared/types';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { isEditorGroupNode } from '@/features/core/layout/layoutEditorGroupNode';

export function useEditorGroups(): EditorGroupSnapshot[] {
  const groupIds = useLayoutStore(
    useShallow((s: LayoutState) =>
      Object.values(s.nodes)
        .filter(isEditorGroupNode)
        .map((node) => node.id),
    ),
  );

  return useMemo(
    () => groupIds.map((id): EditorGroupSnapshot => ({ id })),
    [groupIds],
  );
}
