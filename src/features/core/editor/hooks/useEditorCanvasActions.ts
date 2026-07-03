import { useCallback, type RefObject } from 'react';
import { commitViewport, setViewportLive } from '@/features/core/viewport';
import { GraphPosition } from '@/shared/types/domain';

export function useEditorCanvasActions(activeTabIdRef: RefObject<string | null>) {
  const setCanvas = useCallback(
    (
      updater: GraphPosition | ((prev: GraphPosition) => GraphPosition),
      targetGraphId?: string,
    ) => {
      const graphId = targetGraphId ?? activeTabIdRef.current;
      if (!graphId) return;
      setViewportLive(graphId, updater);
      commitViewport(graphId);
    },
    [activeTabIdRef],
  );

  return { setCanvas };
}
