import { useCallback, useRef } from 'react';
import { BaseNode } from '@/shared/types/editor';
import { SubGraphData } from '@/shared/types/editor';
import { useNodeStore } from '@/features/node-registry/stores';
import { useLayoutStore, LayoutState } from '@/features/layoutStore/layoutStore';
import { useClipboardStore } from '../stores';
import { serializeSubGraph, deserializeSubGraph, deleteNodeInBackend } from '@/shared/utils/editor';
import { ProjectService } from '@/services/project/projectService';
import { uiStore } from '@/features/ui/UIStore';
import { useViewportStore } from '@/features/canvas/stores';

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

/**
 * Editor Operations Hook
 * Handles clipboard operations, history, and node operations
 */
export function useEditorOperations() {
  const { clipboard, setClipboard } = useClipboardStore();
  
  // Get active tab and group IDs
  const activeGroupId = useLayoutStore((s: LayoutState) => s.activeGroupId);
  // const activeEditorGroupId = useLayoutStore((s: LayoutState) => s.activeEditorGroupId);
  const activeEditorNode = useLayoutStore((s: LayoutState) => 
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;
  const selectedNodeIds = activeEditorNode?.data?.params?.selectedNodeIds || [];

  // Refs for stable access in callbacks
  const activeTabIdRef = useRef(activeTabId);
  const selectedNodeIdsRef = useRef(selectedNodeIds);
  const canvasRef = useRef(useViewportStore.getState().viewports[activeGroupId || ''] || DEFAULT_VIEWPORT);

  // Update refs
  activeTabIdRef.current = activeTabId;
  selectedNodeIdsRef.current = selectedNodeIds;

  // Helper to get/set nodes
  const setNodes = useCallback((updater: BaseNode[] | ((prev: BaseNode[]) => BaseNode[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    const currentNodes = useNodeStore.getState().getNodes(tId);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes) : updater;
    useNodeStore.getState().setNodes(tId, nextNodes);
  }, []);

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

  // History operations
  const saveHistory = useCallback(() => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    useNodeStore.getState().saveSnapshot(tid);
  }, []);

  const undo = useCallback(() => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    useNodeStore.getState().undo(tid);
  }, []);

  const redo = useCallback(() => {
    const tid = activeTabIdRef.current;
    if (!tid) return;
    useNodeStore.getState().redo(tid);
  }, []);

  // Clipboard operations
  const copy = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const currentNodes = useNodeStore.getState().getNodes(tid);
    const sel = currentNodes.filter(n => sIds.has(n.id) && !n.isInternal);
    if (sel.length > 0) setClipboard(sel.map(n => n.clone()));
  }, [setClipboard]);

  const cut = useCallback(() => {
    copy();
    deleteSelected();
  }, [copy]);

  const paste = useCallback(async (pos?: { x: number; y: number }) => {
    if (clipboard.length === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    saveHistory();

    // Calculate position offset
    let tX = pos ? pos.x : -canvasRef.current.x / canvasRef.current.scale + 100;
    let tY = pos ? pos.y : -canvasRef.current.y / canvasRef.current.scale + 100;
    const minX = Math.min(...clipboard.map(n => n.position.x));
    const minY = Math.min(...clipboard.map(n => n.position.y));
    const offX = tX - minX, offY = tY - minY;

    // Prepare nodes
    const tempNodes = clipboard.map(n => {
      const clone = n.clone();
      clone.position = { x: n.position.x + offX, y: n.position.y + offY };
      return clone;
    });

    // Serialize for backend
    const serializedData = serializeSubGraph("temp", "temp", "event", tempNodes, { x: 0, y: 0, scale: 1 }, {}, [], []);

    try {
      console.log('[useEditorOperations] Pasting nodes via backend...');
      
      const newSerializedNodes = await ProjectService.createNodesWithConnections(
        tid, 
        serializedData.nodes, 
        serializedData.connections
      );

      const updatedConnections = await ProjectService.getConnections(tid);

      const tempResData: SubGraphData = {
        id: tid,
        name: "temp",
        type: "event",
        nodes: newSerializedNodes,
        connections: updatedConnections,
        canvas: { x: 0, y: 0, scale: 1 },
        variables: {},
        inputs: [],
        outputs: []
      };
      const { nodes: newBaseNodes } = deserializeSubGraph(tempResData);

      setNodes((prev) => [...prev, ...newBaseNodes]);
      setSelectedNodeIds(newBaseNodes.map(n => n.id));

      console.log('[useEditorOperations] Paste completed successfully');
    } catch (e) {
      console.error('[useEditorOperations] Failed to paste nodes:', e);
      uiStore.showToast("粘贴失败", "error", 2000);
    }
  }, [clipboard, saveHistory, setNodes, setSelectedNodeIds]);

  const deleteSelected = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;

    let idsToDelete = new Set<string>();

    setNodes((prev: BaseNode[]) => {
      const nodesToDelete = prev.filter(n => sIds.has(n.id) && !n.isInternal);
      idsToDelete = new Set(nodesToDelete.map(n => n.id));
      if (idsToDelete.size === 0) return prev;

      const pinsToDelete = new Set<string>();
      nodesToDelete.forEach(n => {
        [...n.inputs, ...n.outputs].forEach(p => pinsToDelete.add(p.id));
      });

      return prev.filter(n => !idsToDelete.has(n.id)).map(n => {
        const clone = n.clone();
        let changed = false;
        clone.inputs.forEach(p => {
          const newLinks = p.links.filter(l => !pinsToDelete.has(l));
          if (newLinks.length !== p.links.length) {
            p.links = newLinks;
            changed = true;
          }
        });
        clone.outputs.forEach(p => {
          const newLinks = p.links.filter(l => !pinsToDelete.has(l));
          if (newLinks.length !== p.links.length) {
            p.links = newLinks;
            changed = true;
          }
        });
        return changed ? clone : n;
      });
    });
    setSelectedNodeIds([]);

    // Sync deletion to backend
    const tid = activeTabIdRef.current;
    if (tid && idsToDelete.size > 0) {
      console.log(`[useEditorOperations] Deleting nodes from backend:`, Array.from(idsToDelete));
      Promise.all(Array.from(idsToDelete).map(id => deleteNodeInBackend(tid, id))).catch(e => {
        console.error('[useEditorOperations] Failed to sync node deletions:', e);
      });
    }
  }, [setNodes, setSelectedNodeIds]);

  // Get history state
  const EMPTY_HISTORY = { past: [], future: [] };
  const history = activeTabId && useNodeStore.getState().tabs[activeTabId]
    ? useNodeStore.getState().tabs[activeTabId].history
    : EMPTY_HISTORY;

  return {
    // History
    undo,
    redo,
    saveHistory,
    canUndo: history.past.length > 0,
    canRedo: history.future.length > 0,

    // Clipboard
    copy,
    cut,
    paste,
    deleteSelected,
  };
}
