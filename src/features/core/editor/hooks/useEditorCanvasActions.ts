/**
 * 编辑器画布操作：setCanvas（视口按 graphId 存储）
 */
import { useCallback, type RefObject } from 'react';
import { useViewportStore } from '@/features/core/viewport';
import { GraphPosition } from '@/shared/types/domain';

export function useEditorCanvasActions(activeTabIdRef: RefObject<string | null>) {
  const setCanvas = useCallback(
    (
      updater: GraphPosition | ((prev: GraphPosition) => GraphPosition),
      targetGraphId?: string,
    ) => {
      const graphId = targetGraphId ?? activeTabIdRef.current;
      if (graphId) useViewportStore.getState().setViewport(graphId, updater);
    },
    [activeTabIdRef],
  );

  return { setCanvas };
}
