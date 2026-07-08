import { useCallback, type RefObject } from 'react';
import { commitViewport, setViewportLive } from '@/features/core/viewport';
import { GraphPosition } from '@/shared/types/domain';

export function useEditorCanvasActions(activeTabIdRef: RefObject<string | null>) {
  const setCanvas = useCallback(
    (
      updater: GraphPosition | ((prev: GraphPosition) => GraphPosition),
      targetGraphPath?: string,
    ) => {
      const graphPath = targetGraphPath ?? activeTabIdRef.current;
      if (!graphPath) return;
      setViewportLive(graphPath, updater);
      commitViewport(graphPath);
    },
    [activeTabIdRef],
  );

  return { setCanvas };
}
