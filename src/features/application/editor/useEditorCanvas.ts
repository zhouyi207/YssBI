import { useCallback, useMemo } from "react";
import { useCanvasInteraction } from "./useCanvasInteraction";
import { useNodeManagement } from "@/features/application/dataManagement";
import {
  EMPTY_EDITOR_PANE_SELECTION,
  getPaneSelection,
  useEditorPaneStateStore,
  workbenchDockviewRead,
} from "@/modules/workbench/public";
import { setInspectionContext } from "./rightSidebarActions";
import { useEditorUIActions, type EditorContextMenuState } from "@/features/core/editor";
import { captureActiveEditorCommandTarget, type EditorCommandTarget } from "./editorCommandFocus";
import type {
  EditorCanvasCommandsSlice,
  EditorCanvasInteractionSlice,
  EditorCanvasMode,
  EditorCanvasScope,
  EditorCanvasSession,
  EditorCanvasWorkspaceSlice,
} from "./editorCanvasTypes";
import { useCanvasMutationHandlers } from "./useCanvasMutationHandlers";
import { useEditorOperations } from "./useEditorOperations";
import { useProjectOperations } from "./useProjectOperations";
import { useGraphDraftStore } from "@/features/core/graphDraft";

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
  const paneSelection = useEditorPaneStateStore(
    (state) => state.selections[scope.panelInstanceId] ?? EMPTY_EDITOR_PANE_SELECTION,
  );
  const mutationHandlers = useCanvasMutationHandlers();
  const compileStatus = useGraphDraftStore(
    (state) => state.sessions[scope.graphPath]?.compileStatus ?? "uncompiled",
  );
  const interactive = mode === "interactive";

  const paneStillMatches = useCallback(() => {
    const panel = workbenchDockviewRead.getPanel(scope.panelInstanceId);
    return (
      panel?.groupId === scope.groupId &&
      panel.metadata.role === "editor" &&
      panel.metadata.resourceRef === scope.graphPath
    );
  }, [scope.panelInstanceId, scope.groupId, scope.graphPath]);
  const syncInspection = useCallback(
    (nodeIds: string[]) => {
      const target = captureActiveEditorCommandTarget();
      if (
        target?.panelInstanceId === scope.panelInstanceId &&
        target.resourceRef === scope.graphPath
      ) {
        setInspectionContext(scope.graphPath, nodeIds);
      }
    },
    [scope.graphPath, scope.panelInstanceId],
  );
  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      if ((targetGroupId && targetGroupId !== scope.groupId) || !paneStillMatches()) return;
      const selection = getPaneSelection(scope.panelInstanceId);
      const current = selection.selectedNodeIds;
      const next = typeof updater === "function" ? updater(current) : updater;
      const nodeIds = [...new Set(next)];
      if (
        !selection.selectedConnectionIds.length &&
        current.length === nodeIds.length &&
        current.every((id, index) => id === nodeIds[index])
      )
        return;
      useEditorPaneStateStore.getState().setSelectedNodeIds(scope.panelInstanceId, nodeIds);
      syncInspection(nodeIds);
    },
    [scope.groupId, scope.panelInstanceId, paneStillMatches, syncInspection],
  );

  const setSelectedConnectionIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      if ((targetGroupId && targetGroupId !== scope.groupId) || !paneStillMatches()) return;
      const selection = getPaneSelection(scope.panelInstanceId);
      const current = selection.selectedConnectionIds;
      const next = typeof updater === "function" ? updater(current) : updater;
      if (
        !selection.selectedNodeIds.length &&
        current.length === next.length &&
        current.every((id, index) => id === next[index])
      )
        return;
      useEditorPaneStateStore.getState().setSelectedConnectionIds(scope.panelInstanceId, next);
      syncInspection([]);
    },
    [scope.groupId, scope.panelInstanceId, paneStillMatches, syncInspection],
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
    scope,
    enabled: interactive,
    handlers: mutationHandlers,
    setSelectedNodeIds,
  });

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
      compileGraph: projectCommands.compileGraph,
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
      projectCommands.compileGraph,
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
      compileStatus,
    };
  }, [compileStatus, paneSelection.selectedConnectionIds, paneSelection.selectedNodeIds, scope]);

  const interaction = useMemo(
    (): EditorCanvasInteractionSlice => ({
      contextMenu: canvasInteraction.contextMenu,
      setContextMenu,
      pendingConnection: canvasInteraction.pendingConnection,
      setPendingConnection: canvasInteraction.setPendingConnection,
      beginGesture: canvasInteraction.beginGesture,
      isInteractive: canvasInteraction.isInteractive,
      mutations: canvasInteraction.mutations,
      insertRerouteAtConnection: canvasInteraction.insertRerouteAtConnection,
    }),
    [
      canvasInteraction.contextMenu,
      setContextMenu,
      canvasInteraction.pendingConnection,
      canvasInteraction.setPendingConnection,
      canvasInteraction.insertRerouteAtConnection,
      canvasInteraction.beginGesture,
      canvasInteraction.isInteractive,
      canvasInteraction.mutations,
    ],
  );

  return useMemo(() => ({ commands, workspace, interaction }), [commands, interaction, workspace]);
}
