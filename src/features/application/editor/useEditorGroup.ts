import { useCallback, useMemo } from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { activateEditorGroup } from '@/features/application/editor/switchEditorTab';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import {
  useEditorSessionCommandsContext,
  useEditorSessionSharedContext,
} from './EditorSessionContext';
import { useOptionalEditorSessionUi } from './useEditorSessionUi';
import { useEditorGroupWorkspace } from '@/features/core/editor/hooks/useEditorGroupWorkspace';
import type { Pin } from '@/shared/types/domain';
import type { EditorViewport } from '@/features/core/viewport';
import {
  composeEditorGroupSession,
  type EditorGroupInteractionSlice,
  type EditorGroupSession,
  type EditorGroupWorkspaceSlice,
} from './editorSessionTypes';

export type UseEditorGroupOptions = {
  /** Mount the global canvas pointer loop. Only the active group's Canvas should pass true. */
  withCanvasInteraction?: boolean;
  /** Subscribe to transient canvas UI (context menu, pending connection). */
  withCanvasUi?: boolean;
};

const noopPointer = () => undefined;
const noopConnectPins = async () => {};

/**
 * Group-scoped editor API: stable commands + shared resources + per-group workspace.
 * Preview canvases skip canvas UI subscriptions to avoid fan-out on context menu changes.
 */
export function useEditorGroup(options?: UseEditorGroupOptions): EditorGroupSession {
  const commands = useEditorSessionCommandsContext();
  const shared = useEditorSessionSharedContext();
  const workspace = useEditorGroupWorkspace();
  const withCanvasInteraction = options?.withCanvasInteraction ?? false;
  const withCanvasUi = options?.withCanvasUi ?? withCanvasInteraction;

  const ui = useOptionalEditorSessionUi(withCanvasUi);

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: commands.activeGroupIdRef as React.RefObject<string>,
    activeTabIdRef: commands.activeTabIdRef,
    viewportRef: commands.viewportRef,
    setSelectedNodeIds: commands.setSelectedNodeIds,
    enabled: withCanvasInteraction,
  });

  const { groupId } = workspace;

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
      commands.setCanvas(updater);
    },
    [ensureActiveGroup, commands.setCanvas],
  );

  const workspaceSlice = useMemo(
    (): EditorGroupWorkspaceSlice => ({
      groupId: workspace.groupId,
      tabs: workspace.tabs,
      activeTabId: workspace.activeTabId,
      selectedNodeIds: workspace.selectedNodeIds,
    }),
    [workspace],
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

  return composeEditorGroupSession(shared, ui, commands, workspaceSlice, interaction);
}
