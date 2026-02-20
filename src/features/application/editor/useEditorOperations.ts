import { useCallback, useRef } from 'react';
import { Node } from '@/shared/types/ui';
import { getGraphById } from '@/features/core/dataStore';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { useClipboardStore } from '@/features/core/editor';
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
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const currentGraph = getGraphById(tid);
    if (!currentGraph) return;

    const currentNodes = currentGraph.nodes || [];
    const sel = currentNodes.filter((n: { id: string; isInternal?: boolean }) => sIds.has(n.id) && !n.isInternal);
    if (sel.length > 0) setClipboard(sel.map((n) => ('clone' in n && typeof (n as any).clone === 'function' ? (n as any).clone() : { ...n }) as Node));
  }, [setClipboard]);

  const paste = useCallback(async (pos?: { x: number; y: number }) => {
    if (clipboard.length === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    const vp = canvasRef.current;
    const tX = pos ? pos.x : -vp.x / vp.scale + 100;
    const tY = pos ? pos.y : -vp.y / vp.scale + 100;
    const minX = Math.min(...clipboard.map(n => n.position.x));
    const minY = Math.min(...clipboard.map(n => n.position.y));
    const offX = tX - minX, offY = tY - minY;

    const requests = clipboard
      .filter(n => !!n.nodeType)
      .map(n => {
        const params: Record<string, string | undefined> = {};
        if (n.variableId) params.variableId = n.variableId;
        if (n.variableName) params.variableName = n.variableName;
        if (n.variableType) params.variableType = n.variableType;
        if (n.subGraphId) params.subGraphId = n.subGraphId;
        if (n.dataframeId) params.dataframeId = n.dataframeId;

        return {
          nodeType: n.nodeType,
          x: n.position.x + offX,
          y: n.position.y + offY,
          params: Object.keys(params).length > 0 ? params : undefined,
        };
      });

    try {
      await executeCommand(tid, 'Composite', { requests });
      await new Promise(r => setTimeout(r, 50));
    } catch (e) {
      console.error('[useEditorOperations] Failed to paste nodes:', e);
      uiStore.showToast("粘贴失败", "error", 2000);
    }
  }, [clipboard, setSelectedNodeIds]);

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
