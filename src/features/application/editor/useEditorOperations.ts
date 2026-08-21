
import { logger } from '@/utils/appLogger';
import { useCallback, useRef } from 'react';
import {
  canCopyNode,
  canCutNode,
  canDeleteNode,
} from '@/features/core/dataStore/graphNodeSelectors';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { editorDockviewPort } from '@/features/core/dockview';
import {
  getActiveLayoutTab,
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
} from '@/features/core/layout';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import { executeCommand, executeCommandWithResult } from '@/features/core/history';
import type { GraphMutationCommandResult } from '@/features/core/history/types';
import {
  redoEditorHistory,
  undoEditorHistory,
} from '@/features/application/editorMutation/historyCoordinator';
import { executeSafeGraphMutation } from '@/features/application/editorMutation/safeGraphMutation';
import { exportEditorSubgraph } from '@/features/application/editorMutation/subgraphExportCoordinator';
import { insertedNodeIdsFromDelta } from '@/features/application/editorMutation/insertedNodeIdsFromDelta';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import { readGraphClipboard, writeGraphClipboard } from '@/services/clipboard';
import { disconnectConnectionsById } from './edgeOperations';
import { focusCanvasSelection } from './rightSidebarActions';

const DUPLICATE_SUBGRAPH_OFFSET = { x: 40, y: 40 } as const;
const EDITOR_OPERATIONS_LOG_SOURCE = 'EditorOperations';

interface EditorOperationContext {
  groupId: string;
  graphPath: string;
  identity: ProjectIdentitySnapshot;
}

interface SelectionAwareEditorOperationContext extends EditorOperationContext {
  nodeSelection: string[];
}

function captureEditorOperationContext(groupId: string): EditorOperationContext | null {
  if (editorDockviewPort.getActiveGroupId() !== groupId) return null;
  const graphPath = getActiveLayoutTab(groupId)?.activeTabId;
  if (!graphPath) return null;
  return { groupId, graphPath, identity: captureProjectIdentity() };
}

function captureSelectionAwareEditorOperationContext(
  groupId: string,
): SelectionAwareEditorOperationContext | null {
  const context = captureEditorOperationContext(groupId);
  if (!context) return null;
  return {
    ...context,
    nodeSelection: [...getEditorGroupGraphSelection(groupId).nodeIds],
  };
}

function isEditorOperationContextCurrent(context: EditorOperationContext): boolean {
  return isCurrentProjectIdentity(context.identity)
    && editorDockviewPort.getActiveGroupId() === context.groupId
    && getActiveLayoutTab(context.groupId)?.activeTabId === context.graphPath;
}

function isCapturedNodeSelectionCurrent(context: SelectionAwareEditorOperationContext): boolean {
  const current = [...getEditorGroupGraphSelection(context.groupId).nodeIds];
  return current.length === context.nodeSelection.length
    && current.every((nodeId, index) => nodeId === context.nodeSelection[index]);
}

function logEditorOperationError(operation: string, error: unknown): void {
  logger.graph.error(`${operation} failed: ${String(error)}`, EDITOR_OPERATIONS_LOG_SOURCE);
}

function isAppliedMutation(
  outcome: GraphMutationCommandResult | null,
): outcome is Extract<GraphMutationCommandResult, { status: 'applied' }> {
  return outcome !== null && outcome !== false && outcome.status === 'applied';
}

function mutationOutcomeStatus(outcome: GraphMutationCommandResult | null): string {
  return outcome !== null && outcome !== false ? outcome.status : 'command unavailable';
}

/**
 * Editor Operations Hook
 * Handles clipboard operations, history, and node operations
 */
export function useEditorOperations() {
  const {
    activeTabId,
    groupId,
    selectedNodeIds,
    selectedConnectionIds,
  } = useActiveEditorGroup();

  const activeTabIdRef = useRef(activeTabId);
  const activeGroupIdRef = useRef(groupId);
  const selectedNodeIdsRef = useRef(selectedNodeIds);
  const selectedConnectionIdsRef = useRef(selectedConnectionIds);

  activeTabIdRef.current = activeTabId;
  activeGroupIdRef.current = groupId;
  selectedNodeIdsRef.current = selectedNodeIds;
  selectedConnectionIdsRef.current = selectedConnectionIds;

  const setSelectedConnectionIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      const update = updateEditorGroupSelectedConnectionIds(updater, targetGroupId);
      if (!update) return;
      const active = getActiveLayoutTab(update.groupId);
      if (active?.tab.type === 'event' || active?.tab.type === 'function') {
        focusCanvasSelection(active.activeTabId, []);
      }
    },
    [],
  );

  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      const update = updateEditorGroupSelectedNodeIds(updater, targetGroupId);
      if (!update) return;
      const active = getActiveLayoutTab(update.groupId);
      if (active?.tab.type === 'event' || active?.tab.type === 'function') {
        focusCanvasSelection(active.activeTabId, update.nodeIds);
      }
    },
    [],
  );

  // ===== History =====
  const undo = useCallback(async () => {
    const graphPath = activeTabIdRef.current;
    if (!graphPath) return false;
    const outcome = await undoEditorHistory(graphPath);
    return outcome.status === 'applied';
  }, []);

  const redo = useCallback(async () => {
    const graphPath = activeTabIdRef.current;
    if (!graphPath) return false;
    const outcome = await redoEditorHistory(graphPath);
    return outcome.status === 'applied';
  }, []);

  // ===== Clipboard =====
  const copyNodes = useCallback(async (nodeIds: string[]) => {
    try {
      const context = captureEditorOperationContext(activeGroupIdRef.current);
      if (!context || nodeIds.length === 0) return false;
      if (!nodeIds.every((nodeId) => canCopyNode(context.graphPath, nodeId))) return false;
      const snapshot = await exportEditorSubgraph({
        graphPath: context.graphPath,
        nodeIds: [...nodeIds],
      });
      await writeGraphClipboard(snapshot);
      return true;
    } catch (error) {
      logEditorOperationError('Copy subgraph', error);
      return false;
    }
  }, []);

  const copy = useCallback(async () => {
    return copyNodes(selectedNodeIdsRef.current);
  }, [copyNodes]);

  const duplicateNodes = useCallback(async (
    nodeIds: string[],
    offset = DUPLICATE_SUBGRAPH_OFFSET,
  ) => {
    try {
      const context = captureSelectionAwareEditorOperationContext(activeGroupIdRef.current);
      if (!context || nodeIds.length === 0) return false;
      if (!nodeIds.every((nodeId) => canCopyNode(context.graphPath, nodeId))) return false;
      const outcome = await executeCommandWithResult(context.graphPath, 'DuplicateSubgraph', {
        nodeIds: [...nodeIds],
        offset: { ...offset },
      });
      if (!isAppliedMutation(outcome)) {
        logEditorOperationError('Duplicate subgraph', mutationOutcomeStatus(outcome));
        return false;
      }
      if (!isEditorOperationContextCurrent(context)) return false;
      const insertedNodeIds = insertedNodeIdsFromDelta(outcome.result.delta);
      if (insertedNodeIds.length > 0 && isCapturedNodeSelectionCurrent(context)) {
        setSelectedNodeIds(insertedNodeIds, context.groupId);
      }
      return true;
    } catch (error) {
      logEditorOperationError('Duplicate subgraph', error);
      return false;
    }
  }, [setSelectedNodeIds]);

  const deleteNodesById = useCallback(async (nodeIds: string[]) => {
    const tid = activeTabIdRef.current;
    if (!tid || nodeIds.length === 0) return;

    const idsToDelete = nodeIds.filter((id) => canDeleteNode(tid, id));
    if (idsToDelete.length === 0) return;

    return executeSafeGraphMutation(tid, 'Delete nodes', 'DeleteNodes', { nodeIds: idsToDelete });
  }, []);

  const breakAllNodeLinks = useCallback(async (nodeId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    return executeSafeGraphMutation(tid, 'Break all links', 'DisconnectNode', { nodeId });
  }, []);

  const selectLinkedNodes = useCallback((nodeId: string) => {
    const store = useGraphDataStore.getState();
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const pinIds = store.getGraphNodePins(tid, nodeId);
    const linked = new Set<string>();

    for (const pinId of pinIds) {
      const connIds = store.getGraphPinConnections(tid, pinId);
      for (const connId of connIds) {
        const conn = store.getGraphConnection(tid, connId);
        if (!conn) continue;
        const otherPinId = conn.from === pinId ? conn.to : conn.from;
        const otherPin = store.getGraphPin(tid, otherPinId);
        if (otherPin?.nodeId && otherPin.nodeId !== nodeId) {
          linked.add(otherPin.nodeId);
        }
      }
    }

    setSelectedNodeIds([...linked]);
  }, [setSelectedNodeIds]);

  const disconnectPinById = useCallback(async (pinId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    return executeSafeGraphMutation(tid, 'Disconnect port', 'DisconnectPort', { pinId });
  }, []);

  const resetPinValue = useCallback(async (nodeId: string, pinId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    return executeCommand(tid, 'SetPinValue', { nodeId, pinId, newValue: null });
  }, []);

  const paste = useCallback(async (pos = { x: 0, y: 0 }) => {
    try {
      const context = captureSelectionAwareEditorOperationContext(activeGroupIdRef.current);
      if (!context) return false;
      const snapshot = await readGraphClipboard();
      if (!isEditorOperationContextCurrent(context)) return false;
      const outcome = await executeCommandWithResult(context.graphPath, 'InsertSubgraph', {
        snapshotJson: JSON.stringify(snapshot),
        anchor: { ...pos },
      });
      if (!isAppliedMutation(outcome)) {
        logEditorOperationError('Paste subgraph', mutationOutcomeStatus(outcome));
        return false;
      }
      if (!isEditorOperationContextCurrent(context)) return false;
      const insertedNodeIds = insertedNodeIdsFromDelta(outcome.result.delta);
      if (insertedNodeIds.length > 0 && isCapturedNodeSelectionCurrent(context)) {
        setSelectedNodeIds(insertedNodeIds, context.groupId);
      }
      return true;
    } catch (error) {
      logEditorOperationError('Paste subgraph', error);
      return false;
    }
  }, [setSelectedNodeIds]);

  const breakConnectionsById = useCallback(async (
    connectionIds: string[],
    graphPath: string,
    groupId: string,
  ) => {
    if (getActiveLayoutTab(groupId)?.activeTabId !== graphPath || connectionIds.length === 0) return false;

    const selectionSnapshot = [...getEditorGroupGraphSelection(groupId).connectionIds];
    const applied = await disconnectConnectionsById(graphPath, connectionIds);
    const currentSelection = [...getEditorGroupGraphSelection(groupId).connectionIds];
    const selectionUnchanged = currentSelection.length === selectionSnapshot.length
      && currentSelection.every((id, index) => id === selectionSnapshot[index]);
    if (applied && getActiveLayoutTab(groupId)?.activeTabId === graphPath && selectionUnchanged) {
      setSelectedConnectionIds([], groupId);
    }
    return applied;
  }, [setSelectedConnectionIds]);

  const deleteSelected = useCallback(async () => {
    const groupId = editorDockviewPort.getActiveGroupId();
    if (!groupId) return;
    const graphPath = getActiveLayoutTab(groupId)?.activeTabId;
    if (!graphPath) return;
    const capturedSelection = getEditorGroupGraphSelection(groupId);
    const connectionSnapshot = [...capturedSelection.connectionIds];

    if (connectionSnapshot.length > 0) {
      const applied = await disconnectConnectionsById(graphPath, connectionSnapshot);
      const currentSelection = [...getEditorGroupGraphSelection(groupId).connectionIds];
      const selectionUnchanged = currentSelection.length === connectionSnapshot.length
        && currentSelection.every((id, index) => id === connectionSnapshot[index]);
      if (applied
        && editorDockviewPort.getActiveGroupId() === groupId
        && getActiveLayoutTab(groupId)?.activeTabId === graphPath
        && selectionUnchanged) {
        setSelectedConnectionIds([], groupId);
      }
      return applied;
    }

    const selectedSnapshot = [...capturedSelection.nodeIds];
    const selectedIds = new Set(selectedSnapshot);
    if (selectedIds.size === 0) return;
    const dataStore = useGraphDataStore.getState();
    const idsToDelete = dataStore
      .getGraphNodeIds(graphPath)
      .filter((nodeId) => selectedIds.has(nodeId) && canDeleteNode(graphPath, nodeId));
    if (idsToDelete.length === 0) return;

    const applied = await executeSafeGraphMutation(
      graphPath,
      'Delete selected nodes',
      'DeleteNodes',
      { nodeIds: idsToDelete },
    );
    const currentSelection = [...getEditorGroupGraphSelection(groupId).nodeIds];
    const selectionUnchanged = currentSelection.length === selectedSnapshot.length
      && currentSelection.every((nodeId, index) => nodeId === selectedSnapshot[index]);
    if (applied
      && editorDockviewPort.getActiveGroupId() === groupId
      && getActiveLayoutTab(groupId)?.activeTabId === graphPath
      && selectionUnchanged) {
      setSelectedNodeIds([], groupId);
    }
    return applied;
  }, [setSelectedConnectionIds, setSelectedNodeIds]);

  const cutNodes = useCallback(async (nodeIds: string[]) => {
    try {
      const context = captureSelectionAwareEditorOperationContext(activeGroupIdRef.current);
      if (!context || nodeIds.length === 0) return false;
      if (!nodeIds.every((nodeId) => canCutNode(context.graphPath, nodeId))) return false;
      const snapshot = await exportEditorSubgraph({
        graphPath: context.graphPath,
        nodeIds: [...nodeIds],
      });
      await writeGraphClipboard(snapshot);
      if (!isEditorOperationContextCurrent(context)) return false;
      const outcome = await executeCommandWithResult(context.graphPath, 'DeleteNodes', {
        nodeIds: [...nodeIds],
      });
      if (!isAppliedMutation(outcome)) {
        logEditorOperationError('Cut subgraph deletion', mutationOutcomeStatus(outcome));
        return false;
      }
      if (isEditorOperationContextCurrent(context) && isCapturedNodeSelectionCurrent(context)) {
        setSelectedNodeIds([], context.groupId);
      }
      return true;
    } catch (error) {
      logEditorOperationError('Cut subgraph', error);
      return false;
    }
  }, [setSelectedNodeIds]);

  const cut = useCallback(async () => {
    return cutNodes(selectedNodeIdsRef.current);
  }, [cutNodes]);

  const duplicateSelected = useCallback(async () => {
    const sIds = selectedNodeIdsRef.current;
    if (sIds.length === 0) return;
    await duplicateNodes(sIds);
  }, [duplicateNodes]);

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
