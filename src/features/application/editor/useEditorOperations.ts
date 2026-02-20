import { useCallback, useRef } from 'react';
import { Node } from '@/shared/types/ui';
import { getGraphById } from '@/features/core/dataStore';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useClipboardStore } from '@/features/core/editor';
import type { ClipboardSnapshot, ClipboardEntry, ClipboardPinEntry } from '@/features/core/editor/stores/useClipboardStore';
import { useHistoryStore, executeCommand } from '@/features/core/history';
import type { GraphHistory } from '@/features/core/history';
import { uiStore } from '@/features/core/ui/UIStore';
import { useViewportStore } from '@/features/core/viewport';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';


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
  const canvasRef = useRef(useViewportStore.getState().viewports[activeGroupId || ''] || DEFAULT_VIEWPORT);

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
    if (selectedNodeIdList.length === 0) return;

    const allSelectedPinIds = new Set<string>();
    const entries: ClipboardEntry[] = [];

    for (const nodeId of selectedNodeIdList) {
      const node = dataStore.nodes[nodeId];
      if (!node || node.isInternal) continue;

      const pinIds = dataStore.nodePins[nodeId] ?? [];
      const pins: ClipboardPinEntry[] = [];

      for (const pinId of pinIds) {
        const pin = dataStore.pins[pinId];
        if (!pin) continue;
        allSelectedPinIds.add(pinId);
        pins.push({
          pinId: pin.id,
          name: pin.name,
          direction: pin.direction as 'input' | 'output',
          userValue: pin.userValue,
        });
      }

      const params: ClipboardEntry['params'] = {};
      if (node.variableId) params.variableId = node.variableId;
      if (node.variableName) params.variableName = node.variableName;
      if (node.variableType) params.variableType = node.variableType;
      if (node.subGraphId) params.subGraphId = node.subGraphId;
      if (node.dataframeId) params.dataframeId = node.dataframeId;

      entries.push({
        nodeType: node.nodeType,
        position: { x: node.position.x, y: node.position.y },
        params: Object.keys(params).length > 0 ? params : undefined,
        pins,
      });
    }

    const internalConnections: ClipboardSnapshot['internalConnections'] = [];
    const seenConnIds = new Set<string>();

    for (const pinId of allSelectedPinIds) {
      const connIds = dataStore.pinConnections[pinId] ?? [];
      for (const connId of connIds) {
        if (seenConnIds.has(connId)) continue;
        seenConnIds.add(connId);
        const conn = dataStore.connections[connId];
        if (!conn) continue;
        if (allSelectedPinIds.has(conn.from) && allSelectedPinIds.has(conn.to)) {
          internalConnections.push({ fromPin: conn.from, toPin: conn.to });
        }
      }
    }

    setClipboard({ entries, internalConnections });
  }, [setClipboard]);

  const paste = useCallback(async (pos?: { x: number; y: number }) => {
    if (!clipboard || clipboard.entries.length === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const vp = canvasRef.current;
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
      await new Promise(r => setTimeout(r, 50));
    } catch (e) {
      console.error('[useEditorOperations] Failed to paste nodes:', e);
      uiStore.showToast("粘贴失败", "error", 2000);
    }
  }, [clipboard]);

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
      console.error('[useEditorOperations] Failed to delete nodes:', e);
      uiStore.showToast("删除失败", "error", 2000);
    }
  }, [setSelectedNodeIds]);

  const cut = useCallback(() => {
    copy();
    deleteSelected();
  }, [copy, deleteSelected]);

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
    cut,
    paste,
    deleteSelected,
  };
}
