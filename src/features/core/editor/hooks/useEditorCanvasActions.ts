/**
 * 编辑器画布操作：setCanvas
 * 使用 core/viewport
 */
import { useCallback } from 'react';
import { useViewportStore } from '@/features/core/viewport';
import { GraphPosition } from '@/shared/types/domain';

export function useEditorCanvasActions(activeGroupId: string) {
  const setCanvas = useCallback(
    (updater: GraphPosition | ((prev: GraphPosition) => GraphPosition), targetGroupId?: string) => {
      const gid = targetGroupId || activeGroupId;
      if (gid) useViewportStore.getState().setViewport(gid, updater);
    },
    [activeGroupId]
  );

  return { setCanvas };
}
