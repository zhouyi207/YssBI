import {
  useCallback,
  useMemo,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { useVariableStore } from '@/features/core/dataStore';
import {
  useEditorGroupWorkspace,
} from '@/features/core/editor';
import type { Pin } from '@/shared/types/domain';
import { prepareEditorGroupForInteraction } from './editorGroupInteraction';
import { useEditorSessionCommandsContext } from './EditorSessionContext';
import type {
  EditorCanvasCommandsSlice,
  EditorCanvasInteractionSlice,
  EditorCanvasMode,
  EditorCanvasResourcesSlice,
  EditorCanvasSession,
  EditorCanvasWorkspaceSlice,
} from './editorSessionTypes';
import { useCanvasMutationHandlers } from './useCanvasMutationHandlers';

export interface UseEditorCanvasOptions {
  mode: EditorCanvasMode;
}

/** Canvas-only editor projection. Preview mode keeps activation handlers but mounts no pointer loop. */
export function useEditorCanvas({ mode }: UseEditorCanvasOptions): EditorCanvasSession {
  const sessionCommands = useEditorSessionCommandsContext();
  const groupWorkspace = useEditorGroupWorkspace();
  const variables = useVariableStore((state) => state.variables);
  const mutationHandlers = useCanvasMutationHandlers();
  const interactive = mode === 'interactive';

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: sessionCommands.activeGroupIdRef as RefObject<string>,
    activeTabIdRef: sessionCommands.activeTabIdRef,
    viewportRef: sessionCommands.viewportRef,
    setSelectedNodeIds: sessionCommands.setSelectedNodeIds,
    handlers: mutationHandlers,
    enabled: interactive && groupWorkspace.groupId !== null,
    uiEnabled: interactive && groupWorkspace.groupId !== null,
  });

  const { groupId } = groupWorkspace;
  const prepareForInteraction = useCallback(() => {
    if (!groupId) return false;
    prepareEditorGroupForInteraction(groupId);
    return true;
  }, [groupId]);

  const onCanvasPointerDown = useCallback(
    (event: ReactPointerEvent) => {
      if (!groupId || !prepareForInteraction()) return;
      canvasInteraction.onCanvasPointerDown(event, groupId);
    },
    [canvasInteraction.onCanvasPointerDown, groupId, prepareForInteraction],
  );

  const onNodePointerDown = useCallback(
    (nodeId: string, event: ReactPointerEvent) => {
      if (!groupId || !prepareForInteraction()) return;
      canvasInteraction.onNodePointerDown(nodeId, event, groupId);
    },
    [canvasInteraction.onNodePointerDown, groupId, prepareForInteraction],
  );

  const onPinPointerDown = useCallback(
    (pin: Pin, event: ReactPointerEvent) => {
      if (!groupId || !prepareForInteraction()) return;
      canvasInteraction.onPinPointerDown(pin, event, groupId);
    },
    [canvasInteraction.onPinPointerDown, groupId, prepareForInteraction],
  );

  const commands = useMemo(
    (): EditorCanvasCommandsSlice => ({
      copyNodes: sessionCommands.copyNodes,
      cutNodes: sessionCommands.cutNodes,
      duplicateNodes: sessionCommands.duplicateNodes,
      deleteNodesById: sessionCommands.deleteNodesById,
      breakAllNodeLinks: sessionCommands.breakAllNodeLinks,
      breakConnectionsById: sessionCommands.breakConnectionsById,
      selectLinkedNodes: sessionCommands.selectLinkedNodes,
      disconnectPinById: sessionCommands.disconnectPinById,
      resetPinValue: sessionCommands.resetPinValue,
      setSelectedNodeIds: sessionCommands.setSelectedNodeIds,
      setSelectedConnectionIds: sessionCommands.setSelectedConnectionIds,
      executeGraph: sessionCommands.executeGraph,
      cancelGraphExecution: sessionCommands.cancelGraphExecution,
      clearGraphArtifacts: sessionCommands.clearGraphArtifacts,
      createNode: sessionCommands.createNode,
    }),
    [
      sessionCommands.copyNodes,
      sessionCommands.cutNodes,
      sessionCommands.duplicateNodes,
      sessionCommands.deleteNodesById,
      sessionCommands.breakAllNodeLinks,
      sessionCommands.breakConnectionsById,
      sessionCommands.selectLinkedNodes,
      sessionCommands.disconnectPinById,
      sessionCommands.resetPinValue,
      sessionCommands.setSelectedNodeIds,
      sessionCommands.setSelectedConnectionIds,
      sessionCommands.executeGraph,
      sessionCommands.cancelGraphExecution,
      sessionCommands.clearGraphArtifacts,
      sessionCommands.createNode,
    ],
  );

  const workspace = useMemo((): EditorCanvasWorkspaceSlice => {
    const activeTab = groupWorkspace.tabs.find(
      (tab) => tab.id === groupWorkspace.activeTabId,
    );
    const activeGraph = activeTab?.type === 'event' || activeTab?.type === 'function'
      ? { graphPath: activeTab.id, kind: activeTab.type }
      : null;

    return {
      groupId: groupId as string,
      activeGraph,
      selectedNodeIds: groupWorkspace.selectedNodeIds,
      selectedConnectionIds: groupWorkspace.selectedConnectionIds,
    };
  }, [
    groupId,
    groupWorkspace.activeTabId,
    groupWorkspace.tabs,
    groupWorkspace.selectedNodeIds,
    groupWorkspace.selectedConnectionIds,
  ]);

  const resources = useMemo(
    (): EditorCanvasResourcesSlice => ({ variables }),
    [variables],
  );

  const interaction = useMemo(
    (): EditorCanvasInteractionSlice => ({
      contextMenu: canvasInteraction.contextMenu,
      setContextMenu: canvasInteraction.setContextMenu,
      pendingConnection: canvasInteraction.pendingConnection,
      setPendingConnection: canvasInteraction.setPendingConnection,
      onCanvasPointerDown,
      onNodePointerDown,
      onPinPointerDown,
      insertRerouteAtConnection: canvasInteraction.insertRerouteAtConnection,
    }),
    [
      canvasInteraction.contextMenu,
      canvasInteraction.setContextMenu,
      canvasInteraction.pendingConnection,
      canvasInteraction.setPendingConnection,
      canvasInteraction.insertRerouteAtConnection,
      onCanvasPointerDown,
      onNodePointerDown,
      onPinPointerDown,
    ],
  );

  return useMemo(
    () => ({ commands, workspace, resources, interaction }),
    [commands, interaction, resources, workspace],
  );
}
