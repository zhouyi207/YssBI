/**
 * 获取所有编辑器组
 * 依赖 layout store
 */

import { useMemo } from 'react';
import { useShallow } from 'zustand/react/shallow';
import type { EditorGroupSnapshot } from '@/shared/types';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { isEditorGroupNode } from '@/features/core/layout/layoutTabQueries';
import { readEditorGroupSnapshot } from '@/features/core/layout/layoutTabModel';

export function useEditorGroups(): EditorGroupSnapshot[] {
  const groupNodes = useLayoutStore(
    useShallow((s: LayoutState) => Object.values(s.nodes).filter(isEditorGroupNode)),
  );

  return useMemo(
    () =>
      groupNodes
        .map((node) => readEditorGroupSnapshot(node))
        .filter((group): group is EditorGroupSnapshot => group != null),
    [groupNodes],
  );
}
