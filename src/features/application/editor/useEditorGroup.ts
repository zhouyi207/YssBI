import { useCallback, useMemo } from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { prepareEditorGroupForInteraction } from '@/features/application/editor/editorGroupInteraction';
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
  withCanvasPointerLoop?: boolean;
  /** Subscribe to transient canvas UI (context menu, pending connection). */
  withCanvasUi?: boolean;
};

const noopConnectPins = async () => {};

/**
 * Group-scoped editor API: stable commands + shared resources + per-group workspace.
 * Canvas handlers are always wired so preview groups can activate on first pointerdown.
 */
export function useEditorGroup(options?: UseEditorGroupOptions): EditorGroupSession {
  const commands = useEditorSessionCommandsContext();
  const shared = useEditorSessionSharedContext();
  const workspace = useEditorGroupWorkspace();
  const withCanvasPointerLoop = options?.withCanvasPointerLoop ?? false;
  const withCanvasUi = options?.withCanvasUi ?? withCanvasPointerLoop;

  const ui = useOptionalEditorSessionUi(withCanvasUi);

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: commands.activeGroupIdRef as React.RefObject<string>,
    activeTabIdRef: commands.activeTabIdRef,
    viewportRef: commands.viewportRef,
    setSelectedNodeIds: commands.setSelectedNodeIds,
    enabled: withCanvasPointerLoop,
  });

  const { groupId } = workspace;

  const prepareForInteraction = useCallback(() => {
    prepareEditorGroupForInteraction(groupId);
  }, [groupId]);

  const wrappedOnCanvasPointerDown = useCallback(
    (e: React.PointerEvent) => {
      prepareForInteraction();
      canvasInteraction.onCanvasPointerDown(e, groupId);
    },
    [prepareForInteraction, canvasInteraction.onCanvasPointerDown, groupId],
  );

  const wrappedOnNodePointerDown = useCallback(
    (nodeId: string, e: React.PointerEvent) => {
      prepareForInteraction();
      canvasInteraction.onNodePointerDown(nodeId, e, groupId);
    },
    [prepareForInteraction, canvasInteraction.onNodePointerDown, groupId],
  );

  const wrappedOnPinPointerDown = useCallback(
    (pin: Pin, e: React.PointerEvent) => {
      prepareForInteraction();
      canvasInteraction.onPinPointerDown(pin, e, groupId);
    },
    [prepareForInteraction, canvasInteraction.onPinPointerDown, groupId],
  );

  const wrappedSetCanvas = useCallback(
    (updater: EditorViewport | ((prev: EditorViewport) => EditorViewport)) => {
      prepareForInteraction();
      commands.setCanvas(updater);
    },
    [prepareForInteraction, commands.setCanvas],
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
      onCanvasPointerDown: wrappedOnCanvasPointerDown,
      onNodePointerDown: wrappedOnNodePointerDown,
      onPinPointerDown: wrappedOnPinPointerDown,
      connectPins: withCanvasPointerLoop ? canvasInteraction.connectPins : noopConnectPins,
      setCanvas: wrappedSetCanvas,
    }),
    [
      withCanvasPointerLoop,
      wrappedOnCanvasPointerDown,
      wrappedOnNodePointerDown,
      wrappedOnPinPointerDown,
      canvasInteraction.connectPins,
      wrappedSetCanvas,
    ],
  );

  return composeEditorGroupSession(shared, ui, commands, workspaceSlice, interaction);
}
