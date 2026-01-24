import React, { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Tab, SubGraphData, EditorGroup } from "../Types/canvas";
import { Pin, BaseNode } from "../Types/nodes";
import { deserializeSubGraph } from "../Utils/io";
import { ProjectService } from "../../../services/projectService";
import { useUI } from "./UIProvider";
import { NODE_REGISTRY } from "../Nodes/registry";
import { createInternalNode } from "../Utils/internalNodes";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore, useTabVariables } from "../Store/useNodeStore";
import { useProjectStore } from "../Store/useProjectStore";
import { useCanvasInteraction } from "../Hooks/useCanvasInteraction";
import { useLayoutStore } from "../../../store/layoutStore";
import { useShallow } from 'zustand/react/shallow';

/* ================= Helper Functions ================= */

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const { showToast } = useUI();

  // --- Project Metadata ---
  const currentPath = useProjectStore(useCallback(s => s.currentPath, []));
  const setCurrentPath = useProjectStore(useCallback(s => s.setCurrentPath, []));

  // --- Collection State (The "Database") uses Store ---
  const events = useProjectStore(useCallback(s => s.events, []));
  const functions = useProjectStore(useCallback(s => s.functions, []));
  const macros = useProjectStore(useCallback(s => s.macros, []));
  const globalVariables = useProjectStore(useCallback(s => s.globalVariables, []));

  // --- Multi-View Editor State (已迁移至 layoutStore) ---
  const activeGroupId = useLayoutStore(useCallback(s => s.activeGroupId, []));
  const activeEditorGroupId = useLayoutStore(useCallback(s => s.activeEditorGroupId, []));
  const activeGroupIdRef = useRef(activeGroupId || '');
  useEffect(() => {
    activeGroupIdRef.current = activeGroupId || '';
  }, [activeGroupId]);

  const activeNodeSelector = useCallback(s => activeGroupId ? s.nodes[activeGroupId] : null, [activeGroupId]);
  const activeNode = useLayoutStore(activeNodeSelector);

  // 获取真正活跃的编辑器节点（用于添加变量、节点等逻辑）
  const activeEditorNodeSelector = useCallback(s => activeEditorGroupId ? s.nodes[activeEditorGroupId] : null, [activeEditorGroupId]);
  const activeEditorNode = useLayoutStore(activeEditorNodeSelector);
  
  const activeTabId = activeEditorNode?.data?.activeTabId || null;
  const currentFocusedTabId = activeNode?.data?.activeTabId || null; // 当前视觉上获得焦点的 Tab (如果有)

  const groupNodesSelector = useCallback(s =>
    Object.values(s.nodes)
      .filter(n => n.type === 'component' && n.data?.tabs),
    []);
  const groupNodes = useLayoutStore(useShallow(groupNodesSelector));

  const groups = useMemo(() => groupNodes.map(n => ({
    id: n.id,
    tabs: n.data?.tabs || [],
    activeTabId: n.data?.activeTabId || null,
    selectedNodeIds: n.data?.params?.selectedNodeIds || []
  })), [groupNodes]);

  const setActiveTabId = useCallback((id: string | null) => {
    if (activeGroupId) {
      useLayoutStore.getState().updateNode(activeGroupId, {
        data: {
          ...useLayoutStore.getState().nodes[activeGroupId].data,
          activeTabId: id || undefined
        }
      });
    }
  }, [activeGroupId]);

  const activeTabIdRef = useRef(activeTabId); useEffect(() => { activeTabIdRef.current = activeTabId; }, [activeTabId]);

  const selectedNodeIds = useMemo(() => activeNode?.data?.params?.selectedNodeIds || [], [activeNode?.data?.params?.selectedNodeIds]);
  const selectedNodeIdsRef = useRef(selectedNodeIds); useEffect(() => { selectedNodeIdsRef.current = selectedNodeIds; }, [selectedNodeIds]);

  // Derived scoped states
  const variables = useTabVariables(activeTabId);

  const EMPTY_HISTORY = useMemo(() => ({ past: [], future: [] }), []);

  const historySelector = useCallback(s => (activeTabId && s.tabs[activeTabId]) ? s.tabs[activeTabId].history : EMPTY_HISTORY, [activeTabId, EMPTY_HISTORY]);
  const history = useNodeStore(useShallow(historySelector));

  // Current Refs
  const variablesRef = useRef(variables); useEffect(() => { variablesRef.current = variables; }, [variables]);

  // Canvas Ref - initialized from store and subscribed
  const canvasRef = useRef(useViewportStore.getState().viewports[activeGroupId || ''] || DEFAULT_VIEWPORT);
  useEffect(() => {
    // We update canvasRef whenever the store changes or groups changes
    const unsub = useViewportStore.subscribe((state) => {
      const currentGroupId = useLayoutStore.getState().activeGroupId;
      if (currentGroupId && state.viewports[currentGroupId]) {
        canvasRef.current = state.viewports[currentGroupId];
      }
    });
    // Initial sync in case we missed it
    const current = useViewportStore.getState().viewports[useLayoutStore.getState().activeGroupId || ''];
    if (current) canvasRef.current = current;

    return unsub;
  }, []);

  // --- Scoped Setters ---
  const setNodes = useCallback((updater: BaseNode[] | ((prev: BaseNode[]) => BaseNode[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    const currentNodes = useNodeStore.getState().getNodes(tId);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes) : updater;
    useNodeStore.getState().setNodes(tId, nextNodes);
  }, []);

  const setCanvas = useCallback((updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupId;
    if (gid) useViewportStore.getState().setViewport(gid, updater);
  }, [activeGroupId]);

  const setSelectedNodeIds = useCallback((updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupId;
    if (gid) {
      const node = useLayoutStore.getState().nodes[gid];
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

  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [selectedItemType, setSelectedItemType] = useState<'variable' | 'event' | 'function' | 'macro' | 'setting' | null>(null);

  const setSelectedInfo = useCallback((id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'setting' | null) => {
    setSelectedItemId(id);
    setSelectedItemType(type);
  }, []);

  const handleSetActiveTabId = useCallback((newId: string | null, forceType?: 'event' | 'function' | 'macro' | 'setting', initialData?: SubGraphData) => {
    setActiveTabId(newId);
    if (!newId) return;
    const id = newId!;
    // Check if tab is already initialized in store
    const tabState = useNodeStore.getState().tabs[id];

    if (!tabState) {
      // Access store state directly
      const st = useProjectStore.getState();
      const source = initialData || st.events[id] || st.functions[id] || st.macros[id];
      if (source) {
        const { nodes: n, variables: v } = deserializeSubGraph(source);
        useNodeStore.getState().initTab(id, n, v);
      } else {
        useNodeStore.getState().initTab(id, [], {});
      }
    }
    const st = useProjectStore.getState();
    const tabSource = st.events[id] || st.functions[id] || st.macros[id];
    const type = forceType || (tabSource as any)?.type;
    if (type) setSelectedInfo(id, type as any);
  }, [setActiveTabId, setSelectedInfo]);

  const openSubGraph = useCallback((id: string, name: string, type: "event" | "function" | "macro", initialData?: SubGraphData) => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';

    // 使用 layoutStore 添加标签
    layoutStore.addTab(targetGroupId, {
      id,
      title: name,
      component: 'GraphEditor'
    });

    handleSetActiveTabId(id, type, initialData);
  }, [handleSetActiveTabId]);

  const openSettingsTab = useCallback(() => {
    const id = "settings-tab";
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';

    layoutStore.addTab(targetGroupId, {
      id,
      title: "Settings",
      component: 'SettingsView'
    });

    handleSetActiveTabId(id, "setting");
  }, [handleSetActiveTabId]);

  const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    const nodes = useLayoutStore.getState().nodes;
    const node = Object.values(nodes).find(n => n.data?.tabs?.find(t => t.id === id));
    if (node) {
      useLayoutStore.getState().removeTab(node.id, id);
    }
  }, []);

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    useLayoutStore.getState().splitNode(sourceGroupId, 'row', 'GraphEditor');
  }, []);

  const closeGroup = useCallback((id: string) => {
    useLayoutStore.getState().removeNode(id);
  }, []);

  // --- Synchronization ---
  useEffect(() => {
    const unsub = useNodeStore.subscribe((state) => {
      const timeout = setTimeout(() => {
        useProjectStore.getState().syncWithTabs(state.tabs);
      }, 500);
      return () => clearTimeout(timeout);
    });
    return unsub;
  }, []);

  const syncActiveToCollection = useCallback(() => {
    useProjectStore.getState().syncWithTabs(useNodeStore.getState().tabs);
  }, []);

  const saveGraphAs = useCallback(async () => {
    try {
      syncActiveToCollection();
      const st = useProjectStore.getState();
      const path = await ProjectService.saveProjectAs(
        st.globalVariables,
        st.events,
        st.functions,
        st.macros
      );
      if (path) {
        setCurrentPath(path);
        showToast("项目已保存", "success", 2000);
      }
    } catch (e) { console.error(e); }
  }, [syncActiveToCollection, showToast, setCurrentPath]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) return saveGraphAs();
    syncActiveToCollection();
    try {
      const st = useProjectStore.getState();
      await ProjectService.saveProject(
        currentPath,
        st.globalVariables,
        st.events,
        st.functions,
        st.macros
      );
      showToast("项目已保存", "success", 2000);
    } catch (e) {
      console.error(e);
      showToast("保存失败", "error", 2000);
    }
  }, [currentPath, saveGraphAs, syncActiveToCollection, showToast]);

  const importGraph = useCallback(async (json?: string) => {
    try {
      const result = await ProjectService.loadProject(json);
      if (!result) return;

      const { project: p, path } = result;
      useProjectStore.getState().loadProject(p, path);

      const first = Object.values(p.events)[0] || Object.values(p.functions)[0];
      if (first) openSubGraph(first.id, first.name, first.type as any, first);
    } catch (e) { console.error(e); }
  }, [openSubGraph]);

  const executeGraph = useCallback(async () => {
    try {
      syncActiveToCollection();
      const st = useProjectStore.getState();
      const res = await ProjectService.executeProject(
        st.globalVariables,
        st.events,
        st.functions,
        st.macros
      );
      res.split('\n').filter(l => l.trim()).forEach(log => { if (log.includes("[Error]")) showToast(log, "error", 5000); });
      showToast("执行完成", "info", 2000);
    } catch (e) { showToast(`执行失败: ${e}`, "error", 5000); }
  }, [syncActiveToCollection, showToast]);

  const updateFunction = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateFunction(id, data);
  }, []);

  const updateEvent = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateEvent(id, data);
  }, []);

  const updateMacro = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateMacro(id, data);
  }, []);

  const addEvent = useCallback((name: string) => {
    const id = `event-${crypto.randomUUID()}`;
    const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Exec", type: "exec" }])];
    const sub: SubGraphData = { id, name, type: "event", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    useProjectStore.getState().addEvent(id, sub);
    openSubGraph(id, name, "event", sub);
  }, [openSubGraph]);

  const addFunction = useCallback((name: string) => {
    const id = `func-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "function_entry", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Then", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "function_return", "Return", "Internal", { x: 550, y: 150 }, [{ name: "In", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name, type: "function", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    useProjectStore.getState().addFunction(id, sub);
    openSubGraph(id, name, "function", sub);
  }, [openSubGraph]);

  const addMacro = useCallback((name: string) => {
    const id = `macro-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_inputs", "Inputs", "Internal", { x: 50, y: 150 }, [], [{ name: "In", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_outputs", "Outputs", "Internal", { x: 550, y: 150 }, [{ name: "Out", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name, type: "macro", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    useProjectStore.getState().addMacro(id, sub);
    openSubGraph(id, name, "macro", sub);
  }, [openSubGraph]);

  const deleteFunction = useCallback((id: string) => { useProjectStore.getState().deleteFunction(id); closeTab(id); }, [closeTab]);
  const deleteEvent = useCallback((id: string) => { useProjectStore.getState().deleteEvent(id); closeTab(id); }, [closeTab]);
  const deleteMacro = useCallback((id: string) => { useProjectStore.getState().deleteMacro(id); closeTab(id); }, [closeTab]);

  const addVariable = useCallback((name: string, type: string, isGlobal: boolean = false) => {
    const id = `var-${crypto.randomUUID()}`;
    const v = { name, type, value: type === "int" ? 0 : type === "bool" ? false : type === "float" ? 0.0 : "" };
    if (isGlobal) useProjectStore.getState().addGlobalVariable(id, v);
    else {
      const tid = activeTabIdRef.current;
      if (tid) useNodeStore.getState().addVariable(tid, id, v);
    }
  }, []);

  const updateVariable = useCallback((id: string, data: any) => {
    const isGlobal = !!useProjectStore.getState().globalVariables[id];
    if (isGlobal) useProjectStore.getState().updateGlobalVariable(id, data);
    else {
      const tid = activeTabIdRef.current;
      if (tid) useNodeStore.getState().updateVariable(tid, id, data);
    }
  }, []);

  const deleteVariable = useCallback((id: string) => {
    if (useProjectStore.getState().globalVariables[id]) useProjectStore.getState().deleteGlobalVariable(id);
    else {
      const tid = activeTabIdRef.current;
      if (tid) useNodeStore.getState().removeVariable(tid, id);
    }
  }, []);

  const promoteVariable = useCallback((id: string) => {
    const tid = activeTabIdRef.current; if (!tid) return;
    const v = useNodeStore.getState().tabs[tid]?.variables[id];
    if (!v) return;
    useNodeStore.getState().removeVariable(tid, id);
    useProjectStore.getState().addGlobalVariable(id, v);
  }, []);

  const demoteVariable = useCallback((id: string) => {
    const v = useProjectStore.getState().globalVariables[id]; if (!v) return;
    useProjectStore.getState().deleteGlobalVariable(id);
    const tid = activeTabIdRef.current;
    if (tid) useNodeStore.getState().addVariable(tid, id, v);
  }, []);

  const saveHistory = useCallback(() => {
    const tid = activeTabIdRef.current; if (!tid) return;
    useNodeStore.getState().saveSnapshot(tid);
  }, []);

  const undo = useCallback(() => {
    const tid = activeTabIdRef.current; if (!tid) return;
    useNodeStore.getState().undo(tid);
  }, []);

  const redo = useCallback(() => {
    const tid = activeTabIdRef.current; if (!tid) return;
    useNodeStore.getState().redo(tid);
  }, []);

  const deleteSelected = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;

    setNodes((prev: BaseNode[]) => {
      const nodesToDelete = prev.filter(n => sIds.has(n.id) && !n.isInternal);
      const idsToDelete = new Set(nodesToDelete.map(n => n.id));
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
  }, [setNodes, setSelectedNodeIds]);

  const clipboardRef = useRef<BaseNode[]>([]);
  const copy = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    const tid = activeTabIdRef.current;
    if (!tid) return;
    const currentNodes = useNodeStore.getState().getNodes(tid);
    const sel = currentNodes.filter(n => sIds.has(n.id) && !n.isInternal);
    if (sel.length > 0) clipboardRef.current = sel.map(n => n.clone());
  }, []);
  const cut = useCallback(() => { copy(); deleteSelected(); }, [copy, deleteSelected]);

  const paste = useCallback((pos?: { x: number; y: number }) => {
    if (clipboardRef.current.length === 0) return;
    saveHistory();
    const clipboard = clipboardRef.current.filter(n => NODE_REGISTRY.getDefinition(n.type));
    let tX = pos ? pos.x : -canvasRef.current.x / canvasRef.current.scale + 100;
    let tY = pos ? pos.y : -canvasRef.current.y / canvasRef.current.scale + 100;
    const minX = Math.min(...clipboard.map(n => n.position.x));
    const minY = Math.min(...clipboard.map(n => n.position.y));
    const offX = tX - minX, offY = tY - minY;

    const idMap = new Map<string, string>();
    const newSelectedIds: string[] = [];
    const newNodes = clipboard.map(n => {
      const newNode = n.clone();
      const nid = `node-${crypto.randomUUID()}`;
      newNode.id = nid;
      newNode.position = { x: n.position.x + offX, y: n.position.y + offY };
      newSelectedIds.push(nid);
      const updatePins = (ps: Pin[]) => ps.forEach(p => {
        const old = p.id;
        p.id = `${nid}_${crypto.randomUUID().slice(0, 8)}`;
        p.nodeId = nid;
        idMap.set(old, p.id);
      });
      updatePins(newNode.inputs);
      updatePins(newNode.outputs);
      return newNode;
    });

    newNodes.forEach(n => {
      [...n.inputs, ...n.outputs].forEach(p => {
        p.links = p.links.map(l => idMap.get(l)).filter(Boolean) as string[];
      });
    });

    setNodes((prev: BaseNode[]) => [...prev, ...newNodes]);
    setSelectedNodeIds(newSelectedIds);
  }, [saveHistory, setNodes, setSelectedNodeIds]);

  const {
    contextMenu, setContextMenu,
    pendingConnection, setPendingConnection,
    connectPins,
    onCanvasPointerDown,
    onNodePointerDown,
    onPinPointerDown,
    onCanvasWheel
  } = useCanvasInteraction({
    activeGroupIdRef,
    activeTabIdRef,
    canvasRef,
    groups,
    setSelectedNodeIds,
    setNodes,
    setCanvas,
    saveHistory
  });

  useEffect(() => {
    const st = useProjectStore.getState();
    const hasEvents = Object.keys(st.events).length > 0;

    if (!hasEvents && (activeNode?.data?.tabs?.length || 0) === 0) {
      const id = "default-event"; const name = "Event Graph"; const type = "event";
      const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", "On Run", "Internal", { x: 100, y: 100 }, [], [{ name: "Exec", type: "exec" }])];
      st.addEvent(id, { id, name, type, nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] });

      const targetGroupId = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId || 'default_editor';
      useLayoutStore.getState().addTab(targetGroupId, { id, title: name, component: 'GraphEditor' });

      setSelectedInfo(id, type);
      useNodeStore.getState().initTab(id, tNodes, {});
    }
  }, []);

  const handleAddTab = useCallback(() => addEvent("New Item"), [addEvent]);
  const handleSetActiveGroupId = useCallback((id: string) => useLayoutStore.getState().setActiveGroup(id), []);

  const contextValue = useMemo(() => ({
    setCanvas, nodes: [], setNodes, onCanvasWheel, onCanvasPointerDown, onNodePointerDown, onPinPointerDown, contextMenu, setContextMenu,
    saveGraphAs, saveGraph, importGraph, executeGraph, variables, globalVariables,
    selectedItemId, selectedItemType, setSelectedInfo,
    addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
    events, addEvent, updateEvent, deleteEvent,
    functions, addFunction, updateFunction, deleteFunction,
    macros, addMacro, updateMacro, deleteMacro,
    undo, redo, copy, paste, cut, deleteSelected, canUndo: history.past.length > 0, canRedo: history.future.length > 0, saveHistory,     connectPins,
    activeGroupId: activeGroupId || 'default_editor',
    activeEditorGroupId: activeEditorGroupId || 'default_editor',
    setActiveGroupId: handleSetActiveGroupId,
    splitEditorRight, closeGroup,
    activeTabId, setActiveTabId, openSubGraph, addTab: handleAddTab, closeTab, openSettingsTab, pendingConnection, setPendingConnection,
    groups,
    selectedNodeIds, setSelectedNodeIds
  }), [
    setCanvas, setNodes, onCanvasWheel, onCanvasPointerDown, onNodePointerDown, onPinPointerDown, contextMenu,
    saveGraphAs, saveGraph, importGraph, executeGraph, variables, globalVariables,
    selectedItemId, selectedItemType, setSelectedInfo,
    addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
    events, addEvent, updateEvent, deleteEvent,
    functions, addFunction, updateFunction, deleteFunction,
    macros, addMacro, updateMacro, deleteMacro,
    undo, redo, copy, paste, cut, deleteSelected, history.past.length, history.future.length, saveHistory, connectPins,
    activeGroupId, handleSetActiveGroupId, splitEditorRight, closeGroup,
    activeTabId, setActiveTabId, openSubGraph, handleAddTab, closeTab, openSettingsTab, pendingConnection,
    groups, selectedNodeIds, setSelectedNodeIds
  ]);

  return (
    <CanvasContext.Provider value={contextValue}>
      {children}
    </CanvasContext.Provider>
  );
};
