import {
  useCallback,
  useMemo,
  useRef,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from 'react';
import { useCanvasInteraction } from '@/features/core/canvas';
import { useVariableStore } from '@/features/core/dataStore';
import {
  EMPTY_EDITOR_PANE_SELECTION,
  getPaneSelection,
  useEditorPaneStateStore,
} from '@/features/core/dockview';
import { setInspectionContext } from './rightSidebarActions';
import type { EditorContextMenuState } from '@/features/core/editor';
import type { Pin } from '@/shared/types/domain';
import {
  captureActiveEditorCommandTarget,
  type EditorCommandTarget,
} from './editorCommandFocus';
import { prepareEditorGroupForInteraction } from './editorGroupInteraction';
import { useEditorSessionCommandsContext } from './EditorSessionContext';
import type {
  EditorCanvasCommandsSlice,
  EditorCanvasInteractionSlice,
  EditorCanvasMode,
  EditorCanvasResourcesSlice,
  EditorCanvasScope,
  EditorCanvasSession,
  EditorCanvasWorkspaceSlice,
} from './editorSessionTypes';
import { useCanvasMutationHandlers } from './useCanvasMutationHandlers';

export interface UseEditorCanvasOptions {
  mode: EditorCanvasMode;
  scope: EditorCanvasScope;
}

/** Canvas-only editor projection scoped to one Dockview panel and resource. */
export function useEditorCanvas({ mode, scope }: UseEditorCanvasOptions): EditorCanvasSession {
  const sessionCommands = useEditorSessionCommandsContext();
  const variables = useVariableStore((state) => state.variables);
  const paneSelection = useEditorPaneStateStore((state) => (
    state.selections[scope.panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION
  ));
  const mutationHandlers = useCanvasMutationHandlers();
  const interactive = mode === 'interactive';
  const groupIdRef = useRef(scope.groupId);
  const graphPathRef = useRef<string | null>(scope.graphPath);
  groupIdRef.current = scope.groupId;
  graphPathRef.current = scope.graphPath;

  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      if (targetGroupId && targetGroupId !== scope.groupId) return;
      const current = getPaneSelection(scope.panelInstanceId).selectedNodeIds;
      const next = typeof updater === 'function' ? updater(current) : updater;
      const nodeIds = [...new Set(next)];
      useEditorPaneStateStore.getState().setSelectedNodeIds(scope.panelInstanceId, nodeIds);
      setInspectionContext(scope.graphPath, nodeIds);
    },
    [scope.graphPath, scope.groupId, scope.panelInstanceId],
  );

  const setSelectedConnectionIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      if (targetGroupId && targetGroupId !== scope.groupId) return;
      const current = getPaneSelection(scope.panelInstanceId).selectedConnectionIds;
      const next = typeof updater === 'function' ? updater(current) : updater;
      useEditorPaneStateStore.getState().setSelectedConnectionIds(scope.panelInstanceId, next);
      setInspectionContext(scope.graphPath, []);
    },
    [scope.graphPath, scope.groupId, scope.panelInstanceId],
  );

  const setContextMenu = useCallback((menu: EditorContextMenuState | null) => {
    sessionCommands.setContextMenu(menu
      ? { ...menu, panelInstanceId: scope.panelInstanceId, groupId: scope.groupId, graphPath: scope.graphPath }
      : null);
  }, [scope.graphPath, scope.groupId, scope.panelInstanceId, sessionCommands.setContextMenu]);

  const resolveCommandTarget = useCallback(
    (target?: EditorCommandTarget) => target ?? captureActiveEditorCommandTarget() ?? undefined,
    [],
  );

  const createNode = useCallback(
    (descriptor: Parameters<typeof sessionCommands.createNode>[0], position: Parameters<typeof sessionCommands.createNode>[1]) => (
      sessionCommands.createNode(
        descriptor,
        position,
        resolveCommandTarget(),
      )
    ),
    [resolveCommandTarget, sessionCommands.createNode],
  );

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: groupIdRef as RefObject<string>,
    activeTabIdRef: graphPathRef,
    panelInstanceId: scope.panelInstanceId,
    viewportRef: sessionCommands.viewportRef,
    setSelectedNodeIds,
    setContextMenu,
    handlers: mutationHandlers,
    enabled: interactive,
    uiEnabled: interactive,
  });

  const prepareForInteraction = useCallback(() => {
    prepareEditorGroupForInteraction(scope.groupId);
    return true;
  }, [scope.groupId]);

  const onCanvasPointerDown = useCallback(
    (event: ReactPointerEvent) => {
      if (!prepareForInteraction()) return;
      canvasInteraction.onCanvasPointerDown(event, scope.groupId);
    },
    [canvasInteraction.onCanvasPointerDown, prepareForInteraction, scope.groupId],
  );

  const onNodePointerDown = useCallback(
    (nodeId: string, event: ReactPointerEvent) => {
      if (!prepareForInteraction()) return;
      canvasInteraction.onNodePointerDown(nodeId, event, scope.groupId);
    },
    [canvasInteraction.onNodePointerDown, prepareForInteraction, scope.groupId],
  );

  const onPinPointerDown = useCallback(
    (pin: Pin, event: ReactPointerEvent) => {
      if (!prepareForInteraction()) return;
      canvasInteraction.onPinPointerDown(pin, event, scope.groupId);
    },
    [canvasInteraction.onPinPointerDown, prepareForInteraction, scope.groupId],
  );

  const commands = useMemo(
    (): EditorCanvasCommandsSlice => ({
      copyNodes: (nodeIds, target) => sessionCommands.copyNodes(nodeIds, resolveCommandTarget(target)),
      cutNodes: (nodeIds, target) => sessionCommands.cutNodes(nodeIds, resolveCommandTarget(target)),
      duplicateNodes: (nodeIds, offset, target) => sessionCommands.duplicateNodes(nodeIds, offset, resolveCommandTarget(target)),
      deleteNodesById: (nodeIds, target) => sessionCommands.deleteNodesById(nodeIds, resolveCommandTarget(target)),
      breakAllNodeLinks: (nodeId, target) => sessionCommands.breakAllNodeLinks(nodeId, resolveCommandTarget(target)),
      breakConnectionsById: (connectionIds, graphPath, targetGroupId, target) => sessionCommands.breakConnectionsById(
        connectionIds,
        graphPath,
        targetGroupId,
        resolveCommandTarget(target),
      ),
      selectLinkedNodes: (nodeId, target) => sessionCommands.selectLinkedNodes(nodeId, resolveCommandTarget(target)),
      disconnectPinById: (pinId, target) => sessionCommands.disconnectPinById(pinId, resolveCommandTarget(target)),
      resetPinValue: (nodeId, pinId, target) => sessionCommands.resetPinValue(nodeId, pinId, resolveCommandTarget(target)),
      setSelectedNodeIds,
      setSelectedConnectionIds,
      executeGraph: sessionCommands.executeGraph,
      cancelGraphExecution: sessionCommands.cancelGraphExecution,
      clearGraphArtifacts: sessionCommands.clearGraphArtifacts,
      createNode,
    }),
    [
      resolveCommandTarget,
      sessionCommands.copyNodes,
      sessionCommands.cutNodes,
      sessionCommands.duplicateNodes,
      sessionCommands.deleteNodesById,
      sessionCommands.breakAllNodeLinks,
      sessionCommands.breakConnectionsById,
      sessionCommands.selectLinkedNodes,
      sessionCommands.disconnectPinById,
      sessionCommands.resetPinValue,
      setSelectedNodeIds,
      setSelectedConnectionIds,
      sessionCommands.executeGraph,
      sessionCommands.cancelGraphExecution,
      sessionCommands.clearGraphArtifacts,
      createNode,
    ],
  );

  const workspace = useMemo((): EditorCanvasWorkspaceSlice => {
    return {
      groupId: scope.groupId,
      activeGraph: { graphPath: scope.graphPath, kind: scope.graphKind },
      selectedNodeIds: paneSelection.selectedNodeIds,
      selectedConnectionIds: paneSelection.selectedConnectionIds,
    };
  }, [paneSelection.selectedConnectionIds, paneSelection.selectedNodeIds, scope]);

  const resources = useMemo(
    (): EditorCanvasResourcesSlice => ({ variables }),
    [variables],
  );

  const interaction = useMemo(
    (): EditorCanvasInteractionSlice => ({
      contextMenu: canvasInteraction.contextMenu,
      setContextMenu,
      pendingConnection: canvasInteraction.pendingConnection,
      setPendingConnection: canvasInteraction.setPendingConnection,
      onCanvasPointerDown,
      onNodePointerDown,
      onPinPointerDown,
      insertRerouteAtConnection: canvasInteraction.insertRerouteAtConnection,
    }),
    [
      canvasInteraction.contextMenu,
      setContextMenu,
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
