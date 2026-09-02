import { useCallback } from "react";
import { logger } from "@/features/application/observability/appLogger";
import {
  canCopyNode,
  canCutNode,
  canDeleteNode,
} from "@/features/core/dataStore/graphNodeSelectors";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
  workbenchDockviewRead,
} from "@/modules/workbench/public";
import { executeCommand, executeCommandWithResult } from "@/features/core/history";
import type { GraphDraftCommandResult } from "@/features/core/history/types";
import {
  redoEditorHistory,
  undoEditorHistory,
} from "@/features/application/graphDraft/historyCoordinator";
import { executeSafeGraphDraftEdit } from "@/features/application/graphDraft/safeGraphDraftEdit";
import { exportEditorSubgraph } from "@/features/application/graphDraft/subgraphExportCoordinator";
import { insertedNodeIdsFromPatch } from "@/features/application/graphDraft/insertedNodeIdsFromPatch";
import { readGraphClipboard, writeGraphClipboard } from "@/services/clipboard";
import { disconnectConnectionsById } from "./edgeOperations";
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
  type EditorCommandTarget,
} from "./editorCommandFocus";
import { revealInspect, setInspectionContext } from "./rightSidebarActions";

const DUPLICATE_SUBGRAPH_OFFSET = { x: 40, y: 40 } as const;
const EDITOR_OPERATIONS_LOG_SOURCE = "EditorOperations";

type GraphEditorCommandTarget = EditorCommandTarget & {
  readonly resourceKind: "event" | "function";
};

interface EditorOperationContext {
  target: GraphEditorCommandTarget;
  groupId: string;
  graphPath: string;
}

interface SelectionAwareEditorOperationContext extends EditorOperationContext {
  nodeSelection: string[];
}

function captureEditorOperationContext(
  suppliedTarget?: EditorCommandTarget,
): EditorOperationContext | null {
  const target = suppliedTarget ?? captureActiveEditorCommandTarget();
  if (!target || !isEditorCommandTargetCurrent(target)) return null;
  if (target.resourceKind !== "event" && target.resourceKind !== "function") return null;
  return {
    target: target as GraphEditorCommandTarget,
    groupId: target.groupId,
    graphPath: target.resourceRef,
  };
}

function captureSelectionAwareEditorOperationContext(
  suppliedTarget?: EditorCommandTarget,
): SelectionAwareEditorOperationContext | null {
  const context = captureEditorOperationContext(suppliedTarget);
  if (!context) return null;
  return {
    ...context,
    nodeSelection: [...getEditorGroupGraphSelection(context.groupId).nodeIds],
  };
}

function captureMatchingEditorOperationContext(
  graphPath: string,
  groupId: string,
  suppliedTarget?: EditorCommandTarget,
): EditorOperationContext | null {
  const context = captureEditorOperationContext(suppliedTarget);
  return context?.graphPath === graphPath && context.groupId === groupId ? context : null;
}

function isEditorOperationContextCurrent(context: EditorOperationContext): boolean {
  return isEditorCommandTargetCurrent(context.target);
}

function isCapturedNodeSelectionCurrent(context: SelectionAwareEditorOperationContext): boolean {
  const current = [...getEditorGroupGraphSelection(context.groupId).nodeIds];
  return (
    current.length === context.nodeSelection.length &&
    current.every((nodeId, index) => nodeId === context.nodeSelection[index])
  );
}

function logEditorOperationError(operation: string, error: unknown): void {
  logger.graph.error(`${operation} failed: ${String(error)}`, EDITOR_OPERATIONS_LOG_SOURCE);
}

function isAppliedMutation(
  outcome: GraphDraftCommandResult | null,
): outcome is Extract<GraphDraftCommandResult, { status: "applied" }> {
  return outcome !== null && outcome !== false && outcome.status === "applied";
}

function mutationOutcomeStatus(outcome: GraphDraftCommandResult | null): string {
  return outcome !== null && outcome !== false ? outcome.status : "command unavailable";
}

/** Handles editor-authorized clipboard, history, selection, and node operations. */
export function useEditorOperations() {
  const setSelectedConnectionIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      const update = updateEditorGroupSelectedConnectionIds(updater, targetGroupId);
      if (!update) return;
      const active = workbenchDockviewRead.getActiveEditorPanelInGroup(update.groupId);
      if (
        active?.metadata.resourceKind === "event" ||
        active?.metadata.resourceKind === "function"
      ) {
        setInspectionContext(active.metadata.resourceRef, []);
      }
    },
    [],
  );

  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      const update = updateEditorGroupSelectedNodeIds(updater, targetGroupId);
      if (!update) return;
      const active = workbenchDockviewRead.getActiveEditorPanelInGroup(update.groupId);
      if (
        active?.metadata.resourceKind === "event" ||
        active?.metadata.resourceKind === "function"
      ) {
        setInspectionContext(active.metadata.resourceRef, update.nodeIds);
      }
    },
    [],
  );

  const undo = useCallback(async (target?: EditorCommandTarget) => {
    const context = captureEditorOperationContext(target);
    if (!context || !isEditorOperationContextCurrent(context)) return false;
    const outcome = await undoEditorHistory(context.graphPath);
    return outcome.status === "applied";
  }, []);

  const redo = useCallback(async (target?: EditorCommandTarget) => {
    const context = captureEditorOperationContext(target);
    if (!context || !isEditorOperationContextCurrent(context)) return false;
    const outcome = await redoEditorHistory(context.graphPath);
    return outcome.status === "applied";
  }, []);

  const copyNodes = useCallback(async (nodeIds: string[], target?: EditorCommandTarget) => {
    try {
      const context = captureEditorOperationContext(target);
      if (!context || nodeIds.length === 0) return false;
      if (!nodeIds.every((nodeId) => canCopyNode(context.graphPath, nodeId))) return false;
      if (!isEditorOperationContextCurrent(context)) return false;
      const snapshot = await exportEditorSubgraph({
        graphPath: context.graphPath,
        nodeIds: [...nodeIds],
      });
      if (!isEditorOperationContextCurrent(context)) return false;
      await writeGraphClipboard(snapshot);
      return true;
    } catch (error) {
      logEditorOperationError("Copy subgraph", error);
      return false;
    }
  }, []);

  const copy = useCallback(
    async (target?: EditorCommandTarget) => {
      const context = captureSelectionAwareEditorOperationContext(target);
      if (!context) return false;
      return copyNodes(context.nodeSelection, target);
    },
    [copyNodes],
  );

  const duplicateNodes = useCallback(
    async (nodeIds: string[], offset = DUPLICATE_SUBGRAPH_OFFSET, target?: EditorCommandTarget) => {
      try {
        const context = captureSelectionAwareEditorOperationContext(target);
        if (!context || nodeIds.length === 0) return false;
        if (!nodeIds.every((nodeId) => canCopyNode(context.graphPath, nodeId))) return false;
        if (!isEditorOperationContextCurrent(context)) return false;
        const outcome = await executeCommandWithResult(context.graphPath, "DuplicateSubgraph", {
          nodeIds: [...nodeIds],
          offset: { ...offset },
        });
        if (!isAppliedMutation(outcome)) {
          logEditorOperationError("Duplicate subgraph", mutationOutcomeStatus(outcome));
          return false;
        }
        if (!isEditorOperationContextCurrent(context)) return false;
        const insertedNodeIds = insertedNodeIdsFromPatch(outcome.result.patch);
        if (insertedNodeIds.length > 0 && isCapturedNodeSelectionCurrent(context)) {
          setSelectedNodeIds(insertedNodeIds, context.groupId);
        }
        return true;
      } catch (error) {
        logEditorOperationError("Duplicate subgraph", error);
        return false;
      }
    },
    [setSelectedNodeIds],
  );

  const deleteNodesById = useCallback(async (nodeIds: string[], target?: EditorCommandTarget) => {
    const context = captureEditorOperationContext(target);
    if (!context || nodeIds.length === 0) return false;

    const idsToDelete = nodeIds.filter((id) => canDeleteNode(context.graphPath, id));
    if (idsToDelete.length === 0 || !isEditorOperationContextCurrent(context)) return false;

    return executeSafeGraphDraftEdit(context.graphPath, "Delete nodes", "DeleteNodes", {
      nodeIds: idsToDelete,
    });
  }, []);

  const breakAllNodeLinks = useCallback(async (nodeId: string, target?: EditorCommandTarget) => {
    const context = captureEditorOperationContext(target);
    if (!context || !isEditorOperationContextCurrent(context)) return false;
    return executeSafeGraphDraftEdit(context.graphPath, "Break all links", "DisconnectNode", {
      nodeId,
    });
  }, []);

  const selectLinkedNodes = useCallback(async (nodeId: string, target?: EditorCommandTarget) => {
    const context = captureEditorOperationContext(target);
    if (!context) return false;
    const store = useGraphProjectionStore.getState();
    const pinIds = store.getGraphNodePins(context.graphPath, nodeId);
    const linked = new Set<string>();

    for (const pinId of pinIds) {
      const connectionIds = store.getGraphPinConnections(context.graphPath, pinId);
      for (const connectionId of connectionIds) {
        const connection = store.getGraphConnection(context.graphPath, connectionId);
        if (!connection) continue;
        const otherPinId = connection.from === pinId ? connection.to : connection.from;
        const otherPin = store.getGraphPin(context.graphPath, otherPinId);
        if (otherPin?.nodeId && otherPin.nodeId !== nodeId) linked.add(otherPin.nodeId);
      }
    }

    if (!isEditorOperationContextCurrent(context)) return false;
    const update = updateEditorGroupSelectedNodeIds([...linked], context.groupId);
    if (!update || !isEditorOperationContextCurrent(context)) return false;
    await revealInspect(context.graphPath, update.nodeIds);
    return true;
  }, []);

  const disconnectPinById = useCallback(async (pinId: string, target?: EditorCommandTarget) => {
    const context = captureEditorOperationContext(target);
    if (!context || !isEditorOperationContextCurrent(context)) return false;
    return executeSafeGraphDraftEdit(context.graphPath, "Disconnect port", "DisconnectPort", {
      pinId,
    });
  }, []);

  const resetPinValue = useCallback(
    async (nodeId: string, pinId: string, target?: EditorCommandTarget) => {
      const context = captureEditorOperationContext(target);
      if (!context || !isEditorOperationContextCurrent(context)) return false;
      return executeCommand(context.graphPath, "SetPinValue", { nodeId, pinId, newValue: null });
    },
    [],
  );

  const paste = useCallback(
    async (pos = { x: 0, y: 0 }, target?: EditorCommandTarget) => {
      try {
        const context = captureSelectionAwareEditorOperationContext(target);
        if (!context) return false;
        const snapshot = await readGraphClipboard();
        if (!isEditorOperationContextCurrent(context)) return false;
        const outcome = await executeCommandWithResult(context.graphPath, "InsertSubgraph", {
          snapshotJson: JSON.stringify(snapshot),
          anchor: { ...pos },
        });
        if (!isAppliedMutation(outcome)) {
          logEditorOperationError("Paste subgraph", mutationOutcomeStatus(outcome));
          return false;
        }
        if (!isEditorOperationContextCurrent(context)) return false;
        const insertedNodeIds = insertedNodeIdsFromPatch(outcome.result.patch);
        if (insertedNodeIds.length > 0 && isCapturedNodeSelectionCurrent(context)) {
          setSelectedNodeIds(insertedNodeIds, context.groupId);
        }
        return true;
      } catch (error) {
        logEditorOperationError("Paste subgraph", error);
        return false;
      }
    },
    [setSelectedNodeIds],
  );

  const breakConnectionsById = useCallback(
    async (
      connectionIds: string[],
      graphPath: string,
      groupId: string,
      target?: EditorCommandTarget,
    ) => {
      const context = captureMatchingEditorOperationContext(graphPath, groupId, target);
      if (!context || connectionIds.length === 0) return false;

      const selectionSnapshot = [...getEditorGroupGraphSelection(groupId).connectionIds];
      if (!isEditorOperationContextCurrent(context)) return false;
      const applied = await disconnectConnectionsById(graphPath, connectionIds);
      const currentSelection = [...getEditorGroupGraphSelection(groupId).connectionIds];
      const selectionUnchanged =
        currentSelection.length === selectionSnapshot.length &&
        currentSelection.every((id, index) => id === selectionSnapshot[index]);
      if (applied && isEditorOperationContextCurrent(context) && selectionUnchanged) {
        setSelectedConnectionIds([], groupId);
      }
      return applied;
    },
    [setSelectedConnectionIds],
  );

  const deleteSelected = useCallback(
    async (target?: EditorCommandTarget) => {
      const context = captureEditorOperationContext(target);
      if (!context) return false;
      const capturedSelection = getEditorGroupGraphSelection(context.groupId);
      const connectionSnapshot = [...capturedSelection.connectionIds];

      if (connectionSnapshot.length > 0) {
        if (!isEditorOperationContextCurrent(context)) return false;
        const applied = await disconnectConnectionsById(context.graphPath, connectionSnapshot);
        const currentSelection = [...getEditorGroupGraphSelection(context.groupId).connectionIds];
        const selectionUnchanged =
          currentSelection.length === connectionSnapshot.length &&
          currentSelection.every((id, index) => id === connectionSnapshot[index]);
        if (applied && isEditorOperationContextCurrent(context) && selectionUnchanged) {
          setSelectedConnectionIds([], context.groupId);
        }
        return applied;
      }

      const selectedSnapshot = [...capturedSelection.nodeIds];
      const selectedIds = new Set(selectedSnapshot);
      if (selectedIds.size === 0) return false;
      const dataStore = useGraphProjectionStore.getState();
      const idsToDelete = dataStore
        .getGraphNodeIds(context.graphPath)
        .filter((nodeId) => selectedIds.has(nodeId) && canDeleteNode(context.graphPath, nodeId));
      if (idsToDelete.length === 0 || !isEditorOperationContextCurrent(context)) return false;

      const applied = await executeSafeGraphDraftEdit(
        context.graphPath,
        "Delete selected nodes",
        "DeleteNodes",
        { nodeIds: idsToDelete },
      );
      const currentSelection = [...getEditorGroupGraphSelection(context.groupId).nodeIds];
      const selectionUnchanged =
        currentSelection.length === selectedSnapshot.length &&
        currentSelection.every((nodeId, index) => nodeId === selectedSnapshot[index]);
      if (applied && isEditorOperationContextCurrent(context) && selectionUnchanged) {
        setSelectedNodeIds([], context.groupId);
      }
      return applied;
    },
    [setSelectedConnectionIds, setSelectedNodeIds],
  );

  const cutNodes = useCallback(
    async (nodeIds: string[], target?: EditorCommandTarget) => {
      try {
        const context = captureSelectionAwareEditorOperationContext(target);
        if (!context || nodeIds.length === 0) return false;
        if (!nodeIds.every((nodeId) => canCutNode(context.graphPath, nodeId))) return false;
        if (!isEditorOperationContextCurrent(context)) return false;
        const snapshot = await exportEditorSubgraph({
          graphPath: context.graphPath,
          nodeIds: [...nodeIds],
        });
        if (!isEditorOperationContextCurrent(context)) return false;
        await writeGraphClipboard(snapshot);
        if (!isEditorOperationContextCurrent(context)) return false;
        const outcome = await executeCommandWithResult(context.graphPath, "DeleteNodes", {
          nodeIds: [...nodeIds],
        });
        if (!isAppliedMutation(outcome)) {
          logEditorOperationError("Cut subgraph deletion", mutationOutcomeStatus(outcome));
          return false;
        }
        if (isEditorOperationContextCurrent(context) && isCapturedNodeSelectionCurrent(context)) {
          setSelectedNodeIds([], context.groupId);
        }
        return true;
      } catch (error) {
        logEditorOperationError("Cut subgraph", error);
        return false;
      }
    },
    [setSelectedNodeIds],
  );

  const cut = useCallback(
    async (target?: EditorCommandTarget) => {
      const context = captureSelectionAwareEditorOperationContext(target);
      if (!context) return false;
      return cutNodes(context.nodeSelection, target);
    },
    [cutNodes],
  );

  const duplicateSelected = useCallback(
    async (target?: EditorCommandTarget) => {
      const context = captureSelectionAwareEditorOperationContext(target);
      if (!context || context.nodeSelection.length === 0) return false;
      return duplicateNodes(context.nodeSelection, DUPLICATE_SUBGRAPH_OFFSET, target);
    },
    [duplicateNodes],
  );

  return {
    undo,
    redo,
    copy,
    copyNodes,
    cut,
    cutNodes,
    paste,
    deleteSelected,
    breakConnectionsById,
    deleteNodesById,
    duplicateNodes,
    duplicateSelected,
    breakAllNodeLinks,
    selectLinkedNodes,
    disconnectPinById,
    resetPinValue,
    setSelectedNodeIds,
    setSelectedConnectionIds,
  };
}
