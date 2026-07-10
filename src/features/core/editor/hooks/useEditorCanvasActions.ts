import { useCallback, type RefObject } from 'react';
import { commitViewport, setViewportLive } from '@/features/core/viewport';
import type { EditorViewport } from '@/features/core/viewport';

export function useEditorCanvasActions(activeTabIdRef: RefObject<string | null>) {
  const setCanvas = useCallback(
    (
      updater: EditorViewport | ((prev: EditorViewport) => EditorViewport),
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
