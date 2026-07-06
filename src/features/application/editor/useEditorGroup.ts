import { useCallback, useContext, useMemo } from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { GroupContext, useEditorGroupWorkspace } from '@/features/core/editor';
import { useEditorSession } from './EditorSessionContext';
import type { Pin } from '@/shared/types/domain';

export type UseEditorGroupOptions = {
  /** Mount the global canvas pointer loop. Only Canvas should pass true. */
  withCanvasInteraction?: boolean;
};

/**
 * Group-scoped editor API backed by a single EditorSessionProvider instance.
 */
export function useEditorGroup(options?: UseEditorGroupOptions) {
  const session = useEditorSession();
  const currentGroupId = useContext(GroupContext);
  const overrideGroupId = currentGroupId || undefined;
  const withCanvasInteraction = options?.withCanvasInteraction ?? false;

  const { groupId, tabs, activeTabId, selectedNodeIds } = useEditorGroupWorkspace(overrideGroupId);
  const setActiveGroup = useLayoutStore((s) => s.setActiveGroup);

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: session.activeGroupIdRef,
    activeTabIdRef: session.activeTabIdRef,
    canvasRef: session.canvasRef,
    setSelectedNodeIds: session.setSelectedNodeIds,
    enabled: withCanvasInteraction,
  });

  const ensureActiveGroup = useCallback(() => {
    if (useLayoutStore.getState().activeGroupId !== groupId) {
      setActiveGroup(groupId);
    }
  }, [groupId, setActiveGroup]);

  const wrappedOnCanvasPointerDown = useCallback(
    (e: React.PointerEvent) => {
      ensureActiveGroup();
      canvasInteraction.onCanvasPointerDown(e, groupId);
    },
    [ensureActiveGroup, canvasInteraction.onCanvasPointerDown, groupId],
  );

  const wrappedOnNodePointerDown = useCallback(
    (nodeId: string, e: React.PointerEvent) => {
      ensureActiveGroup();
      canvasInteraction.onNodePointerDown(nodeId, e, groupId);
    },
    [ensureActiveGroup, canvasInteraction.onNodePointerDown, groupId],
  );

  const wrappedOnPinPointerDown = useCallback(
    (pin: Pin, e: React.PointerEvent) => {
      ensureActiveGroup();
      canvasInteraction.onPinPointerDown(pin, e, groupId);
    },
    [ensureActiveGroup, canvasInteraction.onPinPointerDown, groupId],
  );

  const wrappedSetCanvas = useCallback(
    (updater: Parameters<typeof session.setCanvas>[0]) => {
      ensureActiveGroup();
      session.setCanvas(updater);
    },
    [ensureActiveGroup, session.setCanvas],
  );

  return useMemo(
    () => ({
      ...session,
      groupId,
      tabs,
      activeTabId,
      selectedNodeIds,
      onCanvasPointerDown: withCanvasInteraction
        ? wrappedOnCanvasPointerDown
        : () => undefined,
      onNodePointerDown: withCanvasInteraction ? wrappedOnNodePointerDown : () => undefined,
      onPinPointerDown: withCanvasInteraction ? wrappedOnPinPointerDown : () => undefined,
      connectPins: withCanvasInteraction ? canvasInteraction.connectPins : async () => {},
      setCanvas: wrappedSetCanvas,
    }),
    [
      session,
      groupId,
      tabs,
      activeTabId,
      selectedNodeIds,
      withCanvasInteraction,
      wrappedOnCanvasPointerDown,
      wrappedOnNodePointerDown,
      wrappedOnPinPointerDown,
      canvasInteraction.connectPins,
      wrappedSetCanvas,
    ],
  );
}
