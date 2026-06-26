import { useCallback, useRef } from 'react';
import { Node } from '@/shared/types/ui';
import { getGraphById } from '@/features/core/dataStore';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useClipboardStore, syncDetailFromNodeSelection } from '@/features/core/editor';
import { buildClipboardSnapshot } from '@/features/core/editor/clipboardSnapshot';
import type { ClipboardSnapshot } from '@/features/core/editor/stores/useClipboardStore';
import { useHistoryStore, executeCommand } from '@/features/core/history';
import type { GraphHistory } from '@/features/core/history';
import { uiStore } from '@/features/core/ui/UIStore';
import { getViewport } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';


/**
 * Editor Operations Hook
 * Handles clipboard operations, history, and node operations
 */
export function useEditorOperations() {
  const clipboard = useClipboardStore((s) => s.clipboard);
  const setClipboard = useClipboardStore((s) => s.setClipboard);

  const activeGroupId = useLayoutStore((s: LayoutState) => s.activeGroupId);
  const activeEditorNode = useLayoutStore((s: LayoutState) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;
  const selectedNodeIds = activeEditorNode?.data?.params?.selectedNodeIds || [];

  const activeTabIdRef = useRef(activeTabId);
  const selectedNodeIdsRef = useRef(selectedNodeIds);

  activeTabIdRef.current = activeTabId;
  selectedNodeIdsRef.current = selectedNodeIds;

  const setSelectedNodeIds = useCallback((updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupId;
    if (gid) {
      const state = useLayoutStore.getState() as LayoutState;
      const node = state.nodes[gid];
      if (node) {
        const current = node.data?.params?.selectedNodeIds || [];
        const next = typeof updater === 'function' ? updater(current) : updater;
        useLayoutStore.getState().updateNode(gid, {
          data: {
            ...node.data,
            params: { ...node.data?.params, selectedNodeIds: next }
          }
        });
        syncDetailFromNodeSelection(gid, next);
      }
    }
  }, [activeGroupId]);

  // ===== History =====
  const undo = useCallback(async () => {
    const tid = activeTabIdRef.current;
    return tid ? useHistoryStore.getState().undo(tid) : false;
  }, []);

  const redo = useCallback(async () => {
    const tid = activeTabIdRef.current;
    return tid ? useHistoryStore.getState().redo(tid) : false;
  }, []);

  // ===== Clipboard =====
  const copy = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const dataStore = useGraphDataStore.getState();
    const graphNodeIds = dataStore.graphNodes[tid] ?? [];
    const selectedNodeIdList = graphNodeIds.filter((nid) => sIds.has(nid));
    const snapshot = buildClipboardSnapshot(selectedNodeIdList);
    if (snapshot) setClipboard(snapshot);
  }, [setClipboard]);

  const copyNodes = useCallback((nodeIds: string[]) => {
    const snapshot = buildClipboardSnapshot(nodeIds);
    if (snapshot) setClipboard(snapshot);
  }, [setClipboard]);

  const duplicateNodes = useCallback(async (nodeIds: string[], offset = { x: 40, y: 40 }) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const snapshot = buildClipboardSnapshot(nodeIds);
    if (!snapshot) return;

    const dupSnapshot: ClipboardSnapshot = {
      ...snapshot,
      entries: snapshot.entries.map((e) => ({
        ...e,
        position: { x: e.position.x + offset.x, y: e.position.y + offset.y },
      })),
    };

    try {
      await executeCommand(tid, 'Composite', { snapshot: dupSnapshot });
    } catch (e) {
      logger.graph.error(`Failed to duplicate nodes: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
      uiStore.showToast("创建副本失败", "error", 2000);
    }
  }, []);

  const deleteNodesById = useCallback(async (nodeIds: string[]) => {
    const tid = activeTabIdRef.current;
    if (!tid || nodeIds.length === 0) return;

    const dataStore = useGraphDataStore.getState();
    const idsToDelete = nodeIds.filter((id) => {
      const node = dataStore.nodes[id];
      return node && !node.isInternal;
    });
    if (idsToDelete.length === 0) return;

    try {
      await executeCommand(tid, 'DeleteNodes', { nodeIds: idsToDelete });
    } catch (e) {
      logger.graph.error(`Failed to delete nodes: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
      uiStore.showToast("删除失败", "error", 2000);
    }
  }, []);

  const breakAllNodeLinks = useCallback(async (nodeId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const pinIds = useGraphDataStore.getState().nodePins[nodeId] ?? [];
    for (const pinId of pinIds) {
      const connIds = useGraphDataStore.getState().pinConnections[pinId] ?? [];
      if (connIds.length === 0) continue;
      try {
        await executeCommand(tid, 'DisconnectPin', { pinId });
      } catch (e) {
        logger.graph.error(`Failed to disconnect pin: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
      }
    }
  }, []);

  const selectLinkedNodes = useCallback((nodeId: string) => {
    const store = useGraphDataStore.getState();
    const pinIds = store.nodePins[nodeId] ?? [];
    const linked = new Set<string>();

    for (const pinId of pinIds) {
      const connIds = store.pinConnections[pinId] ?? [];
      for (const connId of connIds) {
        const conn = store.connections[connId];
        if (!conn) continue;
        const otherPinId = conn.from === pinId ? conn.to : conn.from;
        const otherPin = store.pins[otherPinId];
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
    try {
      await executeCommand(tid, 'DisconnectPin', { pinId });
    } catch (e) {
      logger.graph.error(`Failed to disconnect pin: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
    }
  }, []);

  const resetPinValue = useCallback(async (nodeId: string, pinId: string) => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    try {
      const { PinService } = await import('@/services');
      await PinService.clearPinUserValue(tid, nodeId, pinId);
      useGraphDataStore.getState().updatePin(pinId, { userValue: undefined });
    } catch (e) {
      logger.graph.error(`Failed to reset pin value: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
      uiStore.showToast("恢复默认值失败", "error", 2000);
    }
  }, []);

  const paste = useCallback(async (pos?: { x: number; y: number }) => {
    if (!clipboard || clipboard.entries.length === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const vp = getViewport(tid);
    const tX = pos ? pos.x : -vp.x / vp.scale + 100;
    const tY = pos ? pos.y : -vp.y / vp.scale + 100;
    const minX = Math.min(...clipboard.entries.map(e => e.position.x));
    const minY = Math.min(...clipboard.entries.map(e => e.position.y));

    const snapshot: ClipboardSnapshot = {
      ...clipboard,
      entries: clipboard.entries.map(e => ({
        ...e,
        position: { x: e.position.x + (tX - minX), y: e.position.y + (tY - minY) },
      })),
    };

    try {
      await executeCommand(tid, 'Composite', { snapshot });
    } catch (e) {
      logger.graph.error(`Failed to paste nodes: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
      uiStore.showToast("粘贴失败", "error", 2000);
    }
  }, [activeGroupId, clipboard]);

  const deleteSelected = useCallback(async () => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const currentGraph = getGraphById(tid);
    if (!currentGraph) return;

    const currentNodes = (currentGraph.nodes || []) as unknown as Node[];
    const idsToDelete = currentNodes
      .filter(n => sIds.has(n.id) && !n.isInternal)
      .map(n => n.id);
    if (idsToDelete.length === 0) return;

    setSelectedNodeIds([]);

    try {
      await executeCommand(tid, 'DeleteNodes', { nodeIds: idsToDelete });
    } catch (e) {
      logger.graph.error(`Failed to delete nodes: ${e instanceof Error ? e.message : String(e)}`, 'EditorOperations');
      uiStore.showToast("删除失败", "error", 2000);
    }
  }, [setSelectedNodeIds]);

  const cutNodes = useCallback(async (nodeIds: string[]) => {
    copyNodes(nodeIds);
    await deleteNodesById(nodeIds);
    setSelectedNodeIds([]);
  }, [copyNodes, deleteNodesById, setSelectedNodeIds]);

  const cut = useCallback(async () => {
    copy();
    await deleteSelected();
  }, [copy, deleteSelected]);

  const duplicateSelected = useCallback(async () => {
    const sIds = selectedNodeIdsRef.current;
    if (sIds.length === 0) return;
    await duplicateNodes(sIds);
  }, [duplicateNodes]);

  const canUndo = useHistoryStore((s) => {
    if (!activeTabId) return false;
    const hist: GraphHistory | undefined = s.histories[activeTabId];
    return !!(hist && hist.undoStack.length > 0);
  });
  const canRedo = useHistoryStore((s) => {
    if (!activeTabId) return false;
    const hist: GraphHistory | undefined = s.histories[activeTabId];
    return !!(hist && hist.redoStack.length > 0);
  });

  return {
    undo,
    redo,
    canUndo,
    canRedo,
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
