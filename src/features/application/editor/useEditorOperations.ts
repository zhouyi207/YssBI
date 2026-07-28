import { useCallback, useRef } from 'react';
import {
  canCutNode,
  canDeleteNode,
} from '@/features/core/dataStore/graphNodeSelectors';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { updateEditorGroupSelectedNodeIds } from '@/features/core/layout';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import { useClipboardStore } from '@/features/core/editor';
import { buildClipboardSnapshot } from '@/features/core/editor/clipboardSnapshot';
import { executeCommand } from '@/features/core/history';
import {
  redoEditorHistory,
  undoEditorHistory,
} from '@/features/application/editorMutation/historyCoordinator';
import { uiStore } from '@/features/core/ui/UIStore';
import { notifyNodeCreationUnavailable } from './editorMutationAvailability';


/**
 * Editor Operations Hook
 * Handles clipboard operations, history, and node operations
 */
export function useEditorOperations() {
  const setClipboard = useClipboardStore((s) => s.setClipboard);

  const { activeTabId, selectedNodeIds } = useActiveEditorGroup();

  const activeTabIdRef = useRef(activeTabId);
  const selectedNodeIdsRef = useRef(selectedNodeIds);

  activeTabIdRef.current = activeTabId;
  selectedNodeIdsRef.current = selectedNodeIds;

  const setSelectedNodeIds = useCallback(
    (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
      updateEditorGroupSelectedNodeIds(updater, targetGroupId);
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
  const copy = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const dataStore = useGraphDataStore.getState();
    const graphNodeIds = dataStore.getGraphNodeIds(tid);
    const selectedNodeIdList = graphNodeIds.filter((nid) => sIds.has(nid));
    const snapshot = buildClipboardSnapshot(selectedNodeIdList, tid);
    if (snapshot) setClipboard(snapshot);
  }, [setClipboard]);

  const copyNodes = useCallback((nodeIds: string[]) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const snapshot = buildClipboardSnapshot(nodeIds, tid);
    if (snapshot) setClipboard(snapshot);
  }, [setClipboard]);

  const duplicateNodes = useCallback(async (_nodeIds: string[], _offset = { x: 40, y: 40 }) => {
    notifyNodeCreationUnavailable();
    return false;
  }, []);

  const deleteNodesById = useCallback(async (nodeIds: string[]) => {
    const tid = activeTabIdRef.current;
    if (!tid || nodeIds.length === 0) return;

    const idsToDelete = nodeIds.filter((id) => canDeleteNode(tid, id));
    if (idsToDelete.length === 0) return;

    const applied = await executeCommand(tid, 'DeleteNodes', { nodeIds: idsToDelete });
    if (!applied) uiStore.showToast("删除失败", "error", 2000);
    return applied;
  }, []);

  const breakAllNodeLinks = useCallback(async (nodeId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const pinIds = useGraphDataStore.getState().getGraphNodePins(tid, nodeId);
    for (const pinId of pinIds) {
      const connIds = useGraphDataStore.getState().getGraphPinConnections(tid, pinId);
      if (connIds.length === 0) continue;
      const applied = await executeCommand(tid, 'DisconnectPin', { pinId });
      if (!applied) return false;
    }
    return true;
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
    return executeCommand(tid, 'DisconnectPin', { pinId });
  }, []);

  const resetPinValue = useCallback(async (nodeId: string, pinId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const applied = await executeCommand(tid, 'SetPinValue', { nodeId, pinId, newValue: null });
    if (!applied) uiStore.showToast("恢复默认值失败", "error", 2000);
    return applied;
  }, []);

  const paste = useCallback(async (_pos?: { x: number; y: number }) => {
    notifyNodeCreationUnavailable();
    return false;
  }, []);

  const deleteSelected = useCallback(async () => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const dataStore = useGraphDataStore.getState();
    const idsToDelete = dataStore
      .getGraphNodeIds(tid)
      .filter((nodeId) => sIds.has(nodeId) && canDeleteNode(tid, nodeId));
    if (idsToDelete.length === 0) return;

    setSelectedNodeIds([]);

    const applied = await executeCommand(tid, 'DeleteNodes', { nodeIds: idsToDelete });
    if (!applied) uiStore.showToast("删除失败", "error", 2000);
    return applied;
  }, [setSelectedNodeIds]);

  const cutNodes = useCallback(async (nodeIds: string[]) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const cuttableIds = nodeIds.filter((nodeId) => canCutNode(tid, nodeId));
    if (cuttableIds.length === 0) return;

    copyNodes(cuttableIds);
    const deleted = await deleteNodesById(cuttableIds);
    if (deleted) setSelectedNodeIds([]);
  }, [copyNodes, deleteNodesById, setSelectedNodeIds]);

  const cut = useCallback(async () => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const selected = new Set(selectedNodeIdsRef.current);
    const nodeIds = useGraphDataStore
      .getState()
      .getGraphNodeIds(tid)
      .filter((nodeId) => selected.has(nodeId));
    await cutNodes(nodeIds);
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
    deleteNodesById,
    duplicateNodes,
    duplicateSelected,
    breakAllNodeLinks,
    selectLinkedNodes,
    disconnectPinById,
    resetPinValue,
    setSelectedNodeIds,
  };
}
