import { useCallback, useEffect } from 'react';
import { useCanvasInteraction } from '@/features/core/canvas/useCanvasInteraction';
import { attachCanvasPointerLoop } from '@/features/core/canvas/canvasPointerLoop';
import { persistGraphViewport } from '@/features/core/viewport';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { activateEditorGroup } from './switchEditorTab';
import type { EditorSession } from './editorSessionTypes';

type EditorCanvasPointerLoopSession = Pick<
  EditorSession,
  | 'activeGroupIdRef'
  | 'activeTabIdRef'
  | 'viewportRef'
  | 'setSelectedNodeIds'
  | 'setContextMenu'
  | 'setPendingConnection'
>;

/** One window-level canvas pointer loop for all editor groups. */
export function useEditorCanvasPointerLoop(session: EditorCanvasPointerLoopSession): void {
  const {
    activeGroupIdRef,
    activeTabIdRef,
    viewportRef,
    setSelectedNodeIds,
    setContextMenu,
    setPendingConnection,
  } = session;

  const persistViewport = useCallback(
    (graphPath?: string | null) => {
      persistGraphViewport(graphPath ?? activeTabIdRef.current);
    },
    [activeTabIdRef],
  );

  const focusEditorGroupAfterGesture = useCallback((groupId: string) => {
    if (useLayoutStore.getState().activeEditorGroupId !== groupId) {
      void activateEditorGroup(groupId);
    }
  }, []);

  const { connectPins } = useCanvasInteraction({
    activeGroupIdRef: activeGroupIdRef as React.RefObject<string>,
    activeTabIdRef,
    viewportRef,
    setSelectedNodeIds,
    enabled: true,
    mountPointerLoop: false,
  });

  useEffect(() => {
    return attachCanvasPointerLoop({
      activeGroupIdRef: activeGroupIdRef as React.RefObject<string>,
      activeTabIdRef,
      viewportRef,
      setSelectedNodeIds,
      connectPins,
      persistViewport,
      setContextMenu,
      setPendingConnection,
      focusEditorGroupAfterGesture,
    });
  }, [
    activeGroupIdRef,
    activeTabIdRef,
    viewportRef,
    setSelectedNodeIds,
    connectPins,
    persistViewport,
    setContextMenu,
    setPendingConnection,
    focusEditorGroupAfterGesture,
  ]);
}
