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

/* ================= Helper Functions ================= */

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const { showToast } = useUI();

  // --- Project Metadata ---
  const currentPath = useProjectStore(s => s.currentPath);
  const setCurrentPath = useProjectStore(s => s.setCurrentPath);

  // --- Collection State (The "Database") uses Store ---
  const events = useProjectStore(s => s.events);
  const functions = useProjectStore(s => s.functions);
  const macros = useProjectStore(s => s.macros);
  const globalVariables = useProjectStore(s => s.globalVariables);

  // No specific Refs needed for these anymore if we access safe state or use store getter in callbacks, 
  // but for some closures we might want to keep using store methods directly.
  // The original implementation used refs for sync access in callbacks.
  // We can use `useProjectStore.getState()` for sync access in callbacks.
  // --- Multi-View Editor State ---

  const [groups, setGroups] = useState<EditorGroup[]>([
    { id: "main-group", tabs: [], activeTabId: null, selectedNodeIds: [] }
  ]);
  const [activeGroupId, setActiveGroupId] = useState("main-group");
  const activeGroupIdRef = useRef(activeGroupId);
  useEffect(() => {
    activeGroupIdRef.current = activeGroupId;
    const vp = useViewportStore.getState().viewports[activeGroupId];
    if (vp) canvasRef.current = vp;
  }, [activeGroupId]);

  const activeGroup = useMemo(() =>
    groups.find(g => g.id === activeGroupId) || groups[0],
    [groups, activeGroupId]
  );

  const activeTabId = activeGroup.activeTabId;
  const activeTabIdRef = useRef(activeTabId); useEffect(() => { activeTabIdRef.current = activeTabId; }, [activeTabId]);

  const selectedNodeIds = activeGroup.selectedNodeIds;
  const selectedNodeIdsRef = useRef(selectedNodeIds); useEffect(() => { selectedNodeIdsRef.current = selectedNodeIds; }, [selectedNodeIds]);

  // Derived scoped states
  const variables = useTabVariables(activeTabId);

  const EMPTY_HISTORY = { past: [], future: [] };

  const history = useNodeStore(s => (activeTabId && s.tabs[activeTabId]) ? s.tabs[activeTabId].history : EMPTY_HISTORY);

  // Current Refs
  const variablesRef = useRef(variables); useEffect(() => { variablesRef.current = variables; }, [variables]);

  // Canvas Ref - initialized from store and subscribed
  const canvasRef = useRef(useViewportStore.getState().viewports[activeGroupId] || DEFAULT_VIEWPORT);
  useEffect(() => {
    // We update canvasRef whenever the store changes or groups changes
    const unsub = useViewportStore.subscribe((state) => {
      const currentGroupId = activeGroupIdRef.current;
      if (state.viewports[currentGroupId]) {
        canvasRef.current = state.viewports[currentGroupId];
      }
    });
    // Initial sync in case we missed it
    const current = useViewportStore.getState().viewports[activeGroupIdRef.current];
    if (current) canvasRef.current = current;

    return unsub;
  }, []); // Empty dependency: one global subscription is enough as it reads Ref for groupId

  // --- Scoped Setters ---
  const setNodes = useCallback((updater: BaseNode[] | ((prev: BaseNode[]) => BaseNode[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    const currentNodes = useNodeStore.getState().getNodes(tId);
    const nextNodes = typeof updater === 'function' ? updater(currentNodes) : updater;
    useNodeStore.getState().setNodes(tId, nextNodes);
  }, []);



  const setCanvas = useCallback((updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupIdRef.current;
    useViewportStore.getState().setViewport(gid, updater);
  }, []);

  const setSelectedNodeIds = useCallback((updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => {
    setGroups(prev => prev.map(g => g.id === (targetGroupId || activeGroupIdRef.current) ? {
      ...g,
      selectedNodeIds: typeof updater === 'function' ? updater(g.selectedNodeIds) : updater
    } : g));
  }, []);

  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [selectedItemType, setSelectedItemType] = useState<'variable' | 'event' | 'function' | 'macro' | 'setting' | null>(null);

  const setSelectedInfo = useCallback((id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'setting' | null) => {
    setSelectedItemId(id);
    setSelectedItemType(type);
  }, []);

  // --- Tab & Group Management ---
  const setTabs = useCallback((updater: Tab[] | ((prev: Tab[]) => Tab[])) => {
    setGroups(prev => prev.map(g => g.id === activeGroupIdRef.current ? {
      ...g,
      tabs: typeof updater === 'function' ? (updater as any)(g.tabs) : updater
    } : g));
  }, []);

  const setActiveTabId = useCallback((id: string | null) => {
    setGroups(prev => prev.map(g => g.id === activeGroupIdRef.current ? { ...g, activeTabId: id } : g));
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
    setTabs(prev => prev.find(t => t.id === id) ? prev : [...prev, { id, title: name, type }]);
    handleSetActiveTabId(id, type, initialData);
  }, [handleSetActiveTabId, setTabs]);

  const openSettingsTab = useCallback(() => {
    const id = "settings-tab";
    setTabs(prev => prev.find(t => t.id === id) ? prev : [...prev, { id, title: "Settings", type: "setting" }]);
    handleSetActiveTabId(id, "setting");
  }, [handleSetActiveTabId, setTabs]);

  const closeTab = useCallback((id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setTabs(prev => {
      const index = prev.findIndex(t => t.id === id);
      const newTabs = prev.filter(t => t.id !== id);
      if (newTabs.length === 0) setTimeout(() => handleSetActiveTabId(null), 0);
      else if (id === activeTabId) setTimeout(() => handleSetActiveTabId(newTabs[Math.min(index, newTabs.length - 1)].id), 0);
      return newTabs;
    });
  }, [activeTabId, handleSetActiveTabId, setTabs]);

  const splitEditorRight = useCallback((sourceGroupId: string) => {
    const src = groups.find(g => g.id === sourceGroupId);
    if (!src) return;
    const nid = `group-${crypto.randomUUID()}`;
    // Initialize viewport for new group with same state as source
    const srcViewport = useViewportStore.getState().viewports[sourceGroupId] || DEFAULT_VIEWPORT;
    useViewportStore.getState().setViewport(nid, { ...srcViewport });

    const newG: EditorGroup = { id: nid, tabs: [...src.tabs], activeTabId: src.activeTabId, selectedNodeIds: [...src.selectedNodeIds] };
    setGroups(prev => [...prev, newG]);
    setActiveGroupId(nid);
  }, [groups]);

  const closeGroup = useCallback((id: string) => {
    setGroups(prev => {
      if (prev.length <= 1) return prev;
      const next = prev.filter(g => g.id !== id);
      if (activeGroupId === id) setActiveGroupId(next[next.length - 1].id);
      return next;
    });
  }, [activeGroupId]);

  // --- Synchronization ---
  // Auto-sync tab changes to project store
  useEffect(() => {
    const unsub = useNodeStore.subscribe((state) => {
      // Simple debounce or throttle could be added here if performance is an issue.
      // For now, we sync immediately but only if we can identify changes.
      // Since useNodeStore updates immutably, we can rely on reference checks if we want.
      // But ProjectService.syncTab/syncWithTabs handles diffing at the object level to some extent (by overwriting).

      // To optimize: Check which tab changed. 
      // But global subscribe doesn't give us the delta easily without comparison.
      // Let's rely on useProjectStore.syncWithTabs for now which iterates tabs.
      // Or better: Iterate tabs and check if changed? 
      // Given the requirement, let's just debounce the sync.

      const timeout = setTimeout(() => {
        useProjectStore.getState().syncWithTabs(state.tabs);
      }, 500);
      return () => clearTimeout(timeout);
    });
    return unsub;
  }, []);

  const syncActiveToCollection = useCallback(() => {
    // Flush manual sync if needed (e.g. before save)
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

  // --- Subgraph Creation & Sync ---


  const updateFunction = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateFunction(id, data);
  }, []);

  const updateEvent = useCallback((id: string, data: Partial<SubGraphData>) => {
    useProjectStore.getState().updateEvent(id, data);
    if (data.name) setTabs(prev => prev.map(t => t.id === id ? { ...t, title: data.name! } : t));
  }, [setTabs]);

  const updateMacro = useCallback((id: string, data: Partial<SubGraphData>) => {
    // updateFunction(id, data);
    // Since we moved logic to store, we should call store updateMacro or updateFunction?
    // updateMacro acts like updateFunction in this context but specifically for macros.
    // However, updateFunction calls `useProjectStore.getState().updateFunction(id, data)` which now contains the cascading logic.
    // Currently updateMacro maps to updateFunction alias in Provider, but in store they are separate.
    // Store's updateMacro ALSO has cascading logic now.
    useProjectStore.getState().updateMacro(id, data);
  }, []);

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

  const addEvent = useCallback((name: string) => {
    const id = `event-${crypto.randomUUID()}`;
    const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Exec", type: "exec" }])];
    const sub: SubGraphData = { id, name, type: "event", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    useProjectStore.getState().addEvent(id, sub);
    openSubGraph(id, name, "event", sub);
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

  // --- Variable Persistence ---
  // --- Variable Persistence ---
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

  // --- Edit Actions (Undo/Redo/Clipboard) ---
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

      // 收集所有将被删除的 pin ID，以便精确清理连接线
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

  /* --- Clipboard & Edit Actions --- */
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

    // 1. Generate new IDs and Map
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

    // 2. Refresh links: Only keep links that point to nodes also in the clipboard (internal connections)
    newNodes.forEach(n => {
      [...n.inputs, ...n.outputs].forEach(p => {
        // If the link target was mapped (exists in clipboard), update it. Otherwise drop it.
        p.links = p.links.map(l => idMap.get(l)).filter(Boolean) as string[];
      });
    });

    setNodes((prev: BaseNode[]) => [...prev, ...newNodes]);
    setSelectedNodeIds(newSelectedIds);
  }, [saveHistory, setNodes, setSelectedNodeIds]);

  // --- Interaction Hooks ---
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

  // --- Initial Project Seed ---
  useEffect(() => {
    const st = useProjectStore.getState();
    const hasEvents = Object.keys(st.events).length > 0;

    if (!hasEvents && activeGroup.tabs.length === 0) {
      const id = "default-event"; const name = "Event Graph"; const type = "event";
      const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", "On Run", "Internal", { x: 100, y: 100 }, [], [{ name: "Exec", type: "exec" }])];
      st.addEvent(id, { id, name, type, nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] });
      setGroups([{ id: "main-group", tabs: [{ id, title: name, type }], activeTabId: id, selectedNodeIds: [] }]);
      setSelectedInfo(id, type);
      // setTabNodes({ [id]: tNodes });
      useNodeStore.getState().initTab(id, tNodes, {});
      setSelectedNodeIds([], "main-group");
    }
  }, []);

  // Pass [] as nodes to Context because it's now handled by useCanvas hook which connects to useNodeStore
  // We keep setNodes for backward compatibility of the context, enabling the provider to act as controller

  return (
    <CanvasContext.Provider value={{
      setCanvas, nodes: [], setNodes, onCanvasWheel, onCanvasPointerDown, onNodePointerDown, onPinPointerDown, contextMenu, setContextMenu,
      saveGraphAs, saveGraph, importGraph, executeGraph, variables, globalVariables,
      selectedItemId, selectedItemType, setSelectedInfo,
      addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
      events, addEvent, updateEvent, deleteEvent,
      functions, addFunction, updateFunction, deleteFunction,
      macros, addMacro, updateMacro, deleteMacro,
      undo, redo, copy, paste, cut, deleteSelected, canUndo: history.past.length > 0, canRedo: history.future.length > 0, saveHistory, connectPins,
      groups, activeGroupId, setActiveGroupId, splitEditorRight, closeGroup,
      activeTabId, setActiveTabId: handleSetActiveTabId, openSubGraph, addTab: () => addEvent("New Item"), closeTab, openSettingsTab, pendingConnection, setPendingConnection,

      selectedNodeIds, setSelectedNodeIds
    }}>
      {children}
    </CanvasContext.Provider>
  );
};
