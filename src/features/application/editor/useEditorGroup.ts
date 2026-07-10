import { useCallback, useContext, useMemo } from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { activateEditorGroup } from '@/features/application/editor/switchEditorTab';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { GroupContext, useEditorGroupWorkspace } from '@/features/core/editor';
import { useEditorSession } from './EditorSessionContext';
import type { Pin } from '@/shared/types/domain';
import type { EditorViewport } from '@/features/core/viewport';
import {
  composeEditorGroupSession,
  type EditorGroupInteractionSlice,
  type EditorGroupSession,
  type EditorGroupWorkspaceSlice,
} from './editorSessionTypes';

export type UseEditorGroupOptions = {
  /** Mount the global canvas pointer loop. Only Canvas should pass true. */
  withCanvasInteraction?: boolean;
};

const noopPointer = () => undefined;
const noopConnectPins = async () => {};

/**
 * Group-scoped editor API backed by a single EditorSessionProvider instance.
 */
export function useEditorGroup(options?: UseEditorGroupOptions): EditorGroupSession {
  const session = useEditorSession();
  const currentGroupId = useContext(GroupContext);
  const overrideGroupId = currentGroupId || undefined;
  const withCanvasInteraction = options?.withCanvasInteraction ?? false;

  const { groupId, tabs, activeTabId, selectedNodeIds } = useEditorGroupWorkspace(overrideGroupId);

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: session.activeGroupIdRef as React.RefObject<string>,
    activeTabIdRef: session.activeTabIdRef,
    viewportRef: session.viewportRef,
    setSelectedNodeIds: session.setSelectedNodeIds,
    enabled: withCanvasInteraction,
  });

  const ensureActiveGroup = useCallback(() => {
    if (useLayoutStore.getState().activeEditorGroupId !== groupId) {
      void activateEditorGroup(groupId);
    }
  }, [groupId]);

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
    (updater: EditorViewport | ((prev: EditorViewport) => EditorViewport)) => {
      ensureActiveGroup();
      session.setCanvas(updater);
    },
    [ensureActiveGroup, session.setCanvas],
  );

  const workspace = useMemo(
    (): EditorGroupWorkspaceSlice => ({
      groupId,
      tabs,
      activeTabId,
      selectedNodeIds,
    }),
    [groupId, tabs, activeTabId, selectedNodeIds],
  );

  const interaction = useMemo(
    (): EditorGroupInteractionSlice => ({
      onCanvasPointerDown: withCanvasInteraction ? wrappedOnCanvasPointerDown : noopPointer,
      onNodePointerDown: withCanvasInteraction ? wrappedOnNodePointerDown : noopPointer,
      onPinPointerDown: withCanvasInteraction ? wrappedOnPinPointerDown : noopPointer,
      connectPins: withCanvasInteraction ? canvasInteraction.connectPins : noopConnectPins,
      setCanvas: wrappedSetCanvas,
    }),
    [
      withCanvasInteraction,
      wrappedOnCanvasPointerDown,
      wrappedOnNodePointerDown,
      wrappedOnPinPointerDown,
      canvasInteraction.connectPins,
      wrappedSetCanvas,
    ],
  );

  return useMemo(
    () => composeEditorGroupSession(session, workspace, interaction),
    [session, workspace, interaction],
  );
}

export type { EditorGroupSession } from './editorSessionTypes';
