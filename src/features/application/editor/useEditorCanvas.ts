import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from "react";
import { useCanvasInteraction } from "@/features/core/canvas";
import { useNodeManagement } from "@/features/application/dataManagement";
import { useVariableStore } from "@/features/core/dataStore";
import {
  EMPTY_EDITOR_PANE_SELECTION,
  getPaneSelection,
  useEditorPaneStateStore,
} from "@/features/core/dockview";
import { setInspectionContext } from "./rightSidebarActions";
import { useEditorUIActions, type EditorContextMenuState } from "@/features/core/editor";
import { editorViewportScope, getViewport, subscribeToViewport } from "@/features/core/viewport";
import type { Pin } from "@/shared/types/domain";
import { captureActiveEditorCommandTarget, type EditorCommandTarget } from "./editorCommandFocus";
import { prepareEditorGroupForInteraction } from "./editorGroupInteraction";
import type {
  EditorCanvasCommandsSlice,
  EditorCanvasInteractionSlice,
  EditorCanvasMode,
  EditorCanvasResourcesSlice,
  EditorCanvasScope,
  EditorCanvasSession,
  EditorCanvasWorkspaceSlice,
} from "./editorCanvasTypes";
import { useCanvasMutationHandlers } from "./useCanvasMutationHandlers";
import { useEditorOperations } from "./useEditorOperations";
import { useProjectOperations } from "./useProjectOperations";

export interface UseEditorCanvasOptions {
  mode: EditorCanvasMode;
  scope: EditorCanvasScope;
}

/** Canvas-only editor projection scoped to one Dockview panel and resource. */
export function useEditorCanvas({ mode, scope }: UseEditorCanvasOptions): EditorCanvasSession {
  const editorCommands = useEditorOperations();
  const projectCommands = useProjectOperations();
  const nodeCommands = useNodeManagement();
  const { setContextMenu: setEditorContextMenu } = useEditorUIActions();
  const variables = useVariableStore((state) => state.variables);
  const paneSelection = useEditorPaneStateStore(
    (state) => state.selections[scope.panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION,
  );
  const mutationHandlers = useCanvasMutationHandlers();
  const interactive = mode === "interactive";
  const groupIdRef = useRef(scope.groupId);
  const graphPathRef = useRef<string | null>(scope.graphPath);
  const viewportRef = useRef(getViewport(editorViewportScope(scope.groupId, scope.graphPath)));
  groupIdRef.current = scope.groupId;
  graphPathRef.current = scope.graphPath;

  useEffect(() => {
    const viewportScope = editorViewportScope(scope.groupId, scope.graphPath);
    viewportRef.current = getViewport(viewportScope);
    return subscribeToViewport(viewportScope, (viewport) => {
      viewportRef.current = viewport;
    });
  }, [scope.graphPath, scope.groupId]);

  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      if (targetGroupId && targetGroupId !== scope.groupId) return;
      const current = getPaneSelection(scope.panelInstanceId).selectedNodeIds;
      const next = typeof updater === "function" ? updater(current) : updater;
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
      const next = typeof updater === "function" ? updater(current) : updater;
      useEditorPaneStateStore.getState().setSelectedConnectionIds(scope.panelInstanceId, next);
      setInspectionContext(scope.graphPath, []);
    },
    [scope.graphPath, scope.groupId, scope.panelInstanceId],
  );

  const setContextMenu = useCallback(
    (menu: EditorContextMenuState | null) => {
      setEditorContextMenu(
        menu
          ? {
              ...menu,
              panelInstanceId: scope.panelInstanceId,
              groupId: scope.groupId,
              graphPath: scope.graphPath,
            }
          : null,
      );
    },
    [scope.graphPath, scope.groupId, scope.panelInstanceId, setEditorContextMenu],
  );

  const resolveCommandTarget = useCallback(
    (target?: EditorCommandTarget) => target ?? captureActiveEditorCommandTarget() ?? undefined,
    [],
  );

  const createNode = useCallback(
    (
      descriptor: Parameters<typeof nodeCommands.createNode>[0],
      position: Parameters<typeof nodeCommands.createNode>[1],
    ) => nodeCommands.createNode(descriptor, position, resolveCommandTarget()),
    [nodeCommands.createNode, resolveCommandTarget],
  );

  const canvasInteraction = useCanvasInteraction({
    activeGroupIdRef: groupIdRef as RefObject<string>,
    activeTabIdRef: graphPathRef,
    panelInstanceId: scope.panelInstanceId,
    viewportRef,
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
      copyNodes: (nodeIds, target) =>
        editorCommands.copyNodes(nodeIds, resolveCommandTarget(target)),
      cutNodes: (nodeIds, target) => editorCommands.cutNodes(nodeIds, resolveCommandTarget(target)),
      duplicateNodes: (nodeIds, offset, target) =>
        editorCommands.duplicateNodes(nodeIds, offset, resolveCommandTarget(target)),
      deleteNodesById: (nodeIds, target) =>
        editorCommands.deleteNodesById(nodeIds, resolveCommandTarget(target)),
      breakAllNodeLinks: (nodeId, target) =>
        editorCommands.breakAllNodeLinks(nodeId, resolveCommandTarget(target)),
      breakConnectionsById: (connectionIds, graphPath, targetGroupId, target) =>
        editorCommands.breakConnectionsById(
          connectionIds,
          graphPath,
          targetGroupId,
          resolveCommandTarget(target),
        ),
      selectLinkedNodes: (nodeId, target) =>
        editorCommands.selectLinkedNodes(nodeId, resolveCommandTarget(target)),
      disconnectPinById: (pinId, target) =>
        editorCommands.disconnectPinById(pinId, resolveCommandTarget(target)),
      resetPinValue: (nodeId, pinId, target) =>
        editorCommands.resetPinValue(nodeId, pinId, resolveCommandTarget(target)),
      setSelectedNodeIds,
      setSelectedConnectionIds,
      executeGraph: projectCommands.executeGraph,
      cancelGraphExecution: projectCommands.cancelGraphExecution,
      clearGraphArtifacts: projectCommands.clearGraphArtifacts,
      createNode,
    }),
    [
      resolveCommandTarget,
      editorCommands.copyNodes,
      editorCommands.cutNodes,
      editorCommands.duplicateNodes,
      editorCommands.deleteNodesById,
      editorCommands.breakAllNodeLinks,
      editorCommands.breakConnectionsById,
      editorCommands.selectLinkedNodes,
      editorCommands.disconnectPinById,
      editorCommands.resetPinValue,
      setSelectedNodeIds,
      setSelectedConnectionIds,
      projectCommands.executeGraph,
      projectCommands.cancelGraphExecution,
      projectCommands.clearGraphArtifacts,
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

  const resources = useMemo((): EditorCanvasResourcesSlice => ({ variables }), [variables]);

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
