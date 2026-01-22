import React, { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, Gesture, Tab, SubGraphData, PinDefinition, EditorGroup } from "../Types/canvas";
import { Pin, BaseNode } from "../Types/nodes";
import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import { serializeSubGraph, deserializeSubGraph, serializeProject, deserializeProject } from "../Utils/io";
import { useUI } from "./UIProvider";
import { NODE_REGISTRY } from "../Nodes/registry";
import { createInternalNode, syncInternalNodePins } from "../Utils/internalNodes";
import { isCompatiblePins, isSingleLinkPin } from "../Utils/pinUtils";
import { clamp } from "../../../types";
import { invoke } from "@tauri-apps/api/core";
import { useViewportStore } from "../Store/useViewportStore";
import { useNodeStore } from "../Store/useNodeStore";

/* ================= Helper Functions ================= */

const updatePinLink = (node: BaseNode, pId: string, oId: string) => {
  const p = [...node.inputs, ...node.outputs].find((x) => x.id === pId);
  if (!p) return false;
  if (isSingleLinkPin(p)) {
    if (p.links.length === 1 && p.links[0] === oId) return false;
    p.links = [oId];
  } else {
    if (p.links.includes(oId)) return false;
    p.links = [...p.links, oId];
  }
  return true;
};

export const CanvasProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const { showToast } = useUI();

  // --- Project Metadata ---
  const [currentPath, setCurrentPath] = useState<string | null>(null);

  // --- Collection State (The "Database") ---
  const [events, setEvents] = useState<Record<string, SubGraphData>>({});
  const [functions, setFunctions] = useState<Record<string, SubGraphData>>({});
  const [macros, setMacros] = useState<Record<string, SubGraphData>>({});
  const [globalVariables, setGlobalVariables] = useState<Record<string, { name: string; type: string; value: any }>>({});

  const eventsRef = useRef(events); useEffect(() => { eventsRef.current = events; }, [events]);
  const functionsRef = useRef(functions); useEffect(() => { functionsRef.current = functions; }, [functions]);
  const macrosRef = useRef(macros); useEffect(() => { macrosRef.current = macros; }, [macros]);
  const globalVariablesRef = useRef(globalVariables); useEffect(() => { globalVariablesRef.current = globalVariables; }, [globalVariables]);

  // --- Multi-View Editor State ---
  const [tabNodes, setTabNodes] = useState<Record<string, BaseNode[]>>({});
  const [tabVariables, setTabVariables] = useState<Record<string, Record<string, { name: string; type: string; value: any }>>>({});
  const [tabHistory, setTabHistory] = useState<Record<string, { past: any[], future: any[] }>>({});

  const [groups, setGroups] = useState<EditorGroup[]>([
    { id: "main-group", tabs: [], activeTabId: null, canvas: { x: 0, y: 0, scale: 1 }, selectedNodeIds: [] }
  ]);
  const [activeGroupId, setActiveGroupId] = useState("main-group");
  const activeGroupIdRef = useRef(activeGroupId);
  useEffect(() => { activeGroupIdRef.current = activeGroupId; }, [activeGroupId]);

  const activeGroup = useMemo(() =>
    groups.find(g => g.id === activeGroupId) || groups[0],
    [groups, activeGroupId]
  );

  const activeTabId = activeGroup.activeTabId;
  const activeTabIdRef = useRef(activeTabId); useEffect(() => { activeTabIdRef.current = activeTabId; }, [activeTabId]);

  const selectedNodeIds = activeGroup.selectedNodeIds;
  const selectedNodeIdsRef = useRef(selectedNodeIds); useEffect(() => { selectedNodeIdsRef.current = selectedNodeIds; }, [selectedNodeIds]);

  // Derived scoped states
  const nodes = useMemo(() => (activeTabId ? tabNodes[activeTabId] || [] : []), [tabNodes, activeTabId]);
  const canvas = activeGroup.canvas;
  const variables = useMemo(() => (activeTabId ? tabVariables[activeTabId] || {} : {}), [tabVariables, activeTabId]);
  const history = useMemo(() => (activeTabId ? tabHistory[activeTabId] || { past: [], future: [] } : { past: [], future: [] }), [tabHistory, activeTabId]);

  // Current Refs
  const nodesRef = useRef(nodes); useEffect(() => { nodesRef.current = nodes; }, [nodes]);
  const variablesRef = useRef(variables); useEffect(() => { variablesRef.current = variables; }, [variables]);
  const canvasRef = useRef(canvas); 
  useEffect(() => { 
    // We update canvasRef whenever the store changes or groups changes
    const unsub = useViewportStore.subscribe((state) => {
      const currentGroupId = activeGroupIdRef.current;
      if (state.viewports[currentGroupId]) {
        canvasRef.current = state.viewports[currentGroupId];
      }
    });
    return unsub;
  }, []);
  useEffect(() => { canvasRef.current = canvas; }, [canvas]);

  // --- Scoped Setters ---
  const setNodes = useCallback((updater: BaseNode[] | ((prev: BaseNode[]) => BaseNode[])) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    setTabNodes(prev => {
      const currentNodes = prev[tId] || [];
      const nextNodes = typeof updater === 'function' ? (updater as any)(currentNodes) : updater;
      // 同步到高性能 Store
      useNodeStore.getState().setNodes(nextNodes);
      return { ...prev, [tId]: nextNodes };
    });
  }, []);

  // 当活动 Tab 切换或节点数据变化时，同步节点到 NodeStore
  useEffect(() => {
    const currentNodes = (activeTabId && tabNodes[activeTabId]) ? tabNodes[activeTabId] : [];
    useNodeStore.getState().setNodes(currentNodes);
  }, [activeTabId, tabNodes]);

  const setVariables = useCallback((updater: any) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    setTabVariables(prev => ({
      ...prev,
      [tId]: typeof updater === 'function' ? updater(prev[tId] || {}) : updater
    }));
  }, []);

  const setCanvas = useCallback((updater: CanvasState | ((prev: CanvasState) => CanvasState), targetGroupId?: string) => {
    const gid = targetGroupId || activeGroupIdRef.current;
    useViewportStore.getState().setViewport(gid, updater);
  }, []);

  // Sync groups canvas to store initially or when groups change from outside
  useEffect(() => {
    groups.forEach(g => {
      const storeState = useViewportStore.getState().viewports[g.id];
      if (!storeState || storeState.x !== g.canvas.x || storeState.y !== g.canvas.y || storeState.scale !== g.canvas.scale) {
        useViewportStore.getState().setViewport(g.id, g.canvas);
      }
    });
  }, [groups]);

  const setHistory = useCallback((updater: any) => {
    const tId = activeTabIdRef.current;
    if (!tId) return;
    setTabHistory(prev => ({
      ...prev,
      [tId]: typeof updater === 'function' ? updater(prev[tId] || { past: [], future: [] }) : updater
    }));
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
    if (!tabNodes[id]) {
      const source = initialData || eventsRef.current[id] || functionsRef.current[id] || macrosRef.current[id];
      if (source) {
        const { nodes: n, variables: v } = deserializeSubGraph(source);
        setTabNodes(prev => ({ ...prev, [id]: n }));
        setTabVariables(prev => ({ ...prev, [id]: v }));
        setTabHistory(prev => ({ ...prev, [id]: { past: [], future: [] } }));
      }
    }
    const tabSource = eventsRef.current[id] || functionsRef.current[id] || macrosRef.current[id];
    const type = forceType || (tabSource as any)?.type;
    if (type) setSelectedInfo(id, type as any);
  }, [tabNodes, setActiveTabId, setSelectedInfo]);

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
    const newG: EditorGroup = { id: nid, tabs: [...src.tabs], activeTabId: src.activeTabId, canvas: { ...src.canvas }, selectedNodeIds: [...src.selectedNodeIds] };
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

  // --- Synchronization & Persistence ---
  const syncActiveToCollection = useCallback(() => {
    Object.keys(tabNodes).forEach(id => {
      const liveNodes = tabNodes[id];
      const liveVars = tabVariables[id] || {};
      const existing = eventsRef.current[id] || functionsRef.current[id] || macrosRef.current[id];
      if (!existing) return;
      const subGraph = serializeSubGraph(id, existing.name, existing.type as any, liveNodes, existing.canvas, liveVars, existing.inputs || [], existing.outputs || []);
      if (eventsRef.current[id]) setEvents(prev => ({ ...prev, [id]: { ...prev[id], ...subGraph } }));
      else if (functionsRef.current[id]) setFunctions(prev => ({ ...prev, [id]: { ...prev[id], ...subGraph } }));
      else if (macrosRef.current[id]) setMacros(prev => ({ ...prev, [id]: { ...prev[id], ...subGraph } }));
    });
  }, [tabNodes, tabVariables]);

  const saveGraphAs = useCallback(async () => {
    try {
      syncActiveToCollection();
      const project = serializeProject(globalVariablesRef.current, eventsRef.current, functionsRef.current, macrosRef.current);
      const path = await save({ filters: [{ name: "JSON", extensions: ["json"] }] });
      if (path) { await writeTextFile(path, JSON.stringify(project, null, 2)); setCurrentPath(path); showToast("项目已保存", "success", 2000); }
    } catch (e) { console.error(e); }
  }, [syncActiveToCollection, showToast]);

  const saveGraph = useCallback(async () => {
    if (!currentPath) return saveGraphAs();
    syncActiveToCollection();
    const project = serializeProject(globalVariablesRef.current, eventsRef.current, functionsRef.current, macrosRef.current);
    await writeTextFile(currentPath, JSON.stringify(project, null, 2));
    showToast("项目已保存", "success", 2000);
  }, [currentPath, saveGraphAs, syncActiveToCollection, showToast]);

  const importGraph = useCallback(async (json?: string) => {
    try {
      let content = json; let path: string | null = null;
      if (!content) {
        const selected = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
        if (!selected || Array.isArray(selected)) return;
        path = selected as string; content = await readTextFile(path);
      }
      if (!content) return;
      const p = deserializeProject(content);
      setGlobalVariables(p.globalVariables); setEvents(p.events); setFunctions(p.functions); setMacros(p.macros);
      setCurrentPath(path);
      const first = Object.values(p.events)[0] || Object.values(p.functions)[0];
      if (first) openSubGraph(first.id, first.name, first.type as any, first);
    } catch (e) { console.error(e); }
  }, [openSubGraph]);

  const executeGraph = useCallback(async () => {
    try {
      syncActiveToCollection();
      const project = serializeProject(globalVariablesRef.current, eventsRef.current, functionsRef.current, macrosRef.current);
      const res: string = await invoke("execute_graph", { projectJson: JSON.stringify(project) });
      res.split('\n').filter(l => l.trim()).forEach(log => { if (log.includes("[Error]")) showToast(log, "error", 5000); });
      showToast("执行完成", "info", 2000);
    } catch (e) { showToast(`执行失败: ${e}`, "error", 5000); }
  }, [syncActiveToCollection, showToast]);

  // --- Subgraph Creation & Sync ---
  const syncSubGraphInstanceNodes = useCallback((nodes: any[], subGraphId: string, inputs?: PinDefinition[], outputs?: PinDefinition[], name?: string) => {
    return nodes.map(n => {
      if (n.subGraphId !== subGraphId) return n;
      const newNode = (n instanceof BaseNode) ? n.clone() : Object.assign(Object.create(Object.getPrototypeOf(n)), n);
      if (name) newNode.title = name;
      const synchronizePins = (newPinDefs: PinDefinition[], existingPins: Pin[], direction: 'input' | 'output') => {
        const execPins = existingPins.filter(p => p.type === 'exec');
        const dataPins = existingPins.filter(p => p.type !== 'exec');
        const newDataPins = newPinDefs.map(newDef => {
          const newPinId = `${newNode.id}_${direction === 'input' ? 'in' : 'out'}_${newDef.id}`;
          const existingPin = dataPins.find(p => p.id === newPinId) || dataPins.find(p => p.name === newDef.name && p.type === newDef.type);
          return { id: newPinId, nodeId: newNode.id, name: newDef.name, type: newDef.type as any, direction, links: existingPin ? existingPin.links : [] };
        });
        return [...execPins, ...newDataPins];
      };
      if (inputs) newNode.inputs = synchronizePins(inputs, n.inputs, 'input');
      if (outputs) newNode.outputs = synchronizePins(outputs, n.outputs, 'output');
      return newNode;
    });
  }, []);

  const updateFunction = useCallback((id: string, data: Partial<SubGraphData>) => {
    setFunctions(prev => {
      const next = { ...prev };
      Object.keys(next).forEach(k => next[k] = { ...next[k], nodes: syncSubGraphInstanceNodes(next[k].nodes, id, data.inputs, data.outputs, data.name) });
      next[id] = { ...next[id], ...data };
      functionsRef.current = next;
      return next;
    });
    setMacros(prev => {
      const next = { ...prev };
      Object.keys(next).forEach(k => next[k] = { ...next[k], nodes: syncSubGraphInstanceNodes(next[k].nodes, id, data.inputs, data.outputs, data.name) });
      macrosRef.current = next;
      return next;
    });
    setEvents(prev => {
      const next = { ...prev };
      Object.keys(next).forEach(k => next[k] = { ...next[k], nodes: syncSubGraphInstanceNodes(next[k].nodes, id, data.inputs, data.outputs, data.name) });
      eventsRef.current = next;
      return next;
    });
    setTabNodes(prev => {
      const next = { ...prev };
      Object.keys(next).forEach(tid => {
        next[tid] = syncSubGraphInstanceNodes(next[tid], id, data.inputs, data.outputs, data.name);
        if (tid === id) {
          next[tid] = next[tid].map(n => {
            if (!n.isInternal) return n;
            const clone = n.clone();
            if (data.name && (n.type === 'function_entry' || n.type === 'macro_inputs')) clone.title = data.name;
            if (n.type === "function_entry" && data.inputs) syncInternalNodePins(clone, data.inputs, true);
            if (n.type === "function_return" && data.outputs) syncInternalNodePins(clone, data.outputs, false);
            return clone;
          });
        }
      });
      return next;
    });
  }, [syncSubGraphInstanceNodes]);

  const updateEvent = useCallback((id: string, data: Partial<SubGraphData>) => {
    setEvents(prev => ({ ...prev, [id]: { ...prev[id], ...data } }));
    if (data.name) setTabs(prev => prev.map(t => t.id === id ? { ...t, title: data.name! } : t));
  }, [setTabs]);

  const updateMacro = useCallback((id: string, data: Partial<SubGraphData>) => {
    updateFunction(id, data);
  }, [updateFunction]);

  const addFunction = useCallback((name: string) => {
    const id = `func-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "function_entry", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Then", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "function_return", "Return", "Internal", { x: 550, y: 150 }, [{ name: "In", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name, type: "function", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    setFunctions(prev => ({ ...prev, [id]: sub }));
    openSubGraph(id, name, "function", sub);
  }, [openSubGraph]);

  const addEvent = useCallback((name: string) => {
    const id = `event-${crypto.randomUUID()}`;
    const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", name, "Internal", { x: 50, y: 150 }, [], [{ name: "Exec", type: "exec" }])];
    const sub: SubGraphData = { id, name, type: "event", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    setEvents({ ...eventsRef.current, [id]: sub }); // Use ref to avoid stale state in rapid calls
    openSubGraph(id, name, "event", sub);
  }, [openSubGraph]);

  const addMacro = useCallback((name: string) => {
    const id = `macro-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_inputs", "Inputs", "Internal", { x: 50, y: 150 }, [], [{ name: "In", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_outputs", "Outputs", "Internal", { x: 550, y: 150 }, [{ name: "Out", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name, type: "macro", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    setMacros(prev => ({ ...prev, [id]: sub }));
    openSubGraph(id, name, "macro", sub);
  }, [openSubGraph]);

  const deleteFunction = useCallback((id: string) => { setFunctions(prev => { const n = { ...prev }; delete n[id]; return n; }); closeTab(id); }, [closeTab]);
  const deleteEvent = useCallback((id: string) => { setEvents(prev => { const n = { ...prev }; delete n[id]; return n; }); closeTab(id); }, [closeTab]);
  const deleteMacro = useCallback((id: string) => { setMacros(prev => { const n = { ...prev }; delete n[id]; return n; }); closeTab(id); }, [closeTab]);

  // --- Variable Persistence ---
  const addVariable = useCallback((name: string, type: string, isGlobal: boolean = false) => {
    const id = `var-${crypto.randomUUID()}`;
    const v = { name, type, value: type === "int" ? 0 : type === "bool" ? false : type === "float" ? 0.0 : "" };
    if (isGlobal) setGlobalVariables(prev => ({ ...prev, [id]: v }));
    else setVariables((prev: any) => ({ ...prev, [id]: v }));
  }, [setVariables]);

  const updateVariable = useCallback((id: string, data: any) => {
    const isGlobal = !!globalVariablesRef.current[id];
    if (isGlobal) setGlobalVariables(prev => ({ ...prev, [id]: { ...prev[id], ...data } }));
    else setVariables((prev: any) => ({ ...prev, [id]: { ...prev[id], ...data } }));
  }, [setVariables]);

  const deleteVariable = useCallback((id: string) => {
    if (globalVariablesRef.current[id]) setGlobalVariables(prev => { const n = { ...prev }; delete n[id]; return n; });
    else setVariables((prev: any) => { const n = { ...prev }; delete n[id]; return n; });
  }, [setVariables]);

  const promoteVariable = useCallback((id: string) => {
    const v = variablesRef.current[id] || tabVariables[activeTabIdRef.current || ''][id];
    if (!v) return;
    setVariables((prev: any) => { const n = { ...prev }; delete n[id]; return n; });
    setGlobalVariables(prev => ({ ...prev, [id]: v }));
  }, [setVariables]);

  const demoteVariable = useCallback((id: string) => {
    const v = globalVariablesRef.current[id]; if (!v) return;
    setGlobalVariables(prev => { const n = { ...prev }; delete n[id]; return n; });
    setVariables((prev: any) => ({ ...prev, [id]: v }));
  }, [setVariables]);

  // --- Edit Actions (Undo/Redo/Clipboard) ---
  const saveHistory = useCallback(() => {
    const tid = activeTabIdRef.current; if (!tid) return;
    const existing = eventsRef.current[tid] || functionsRef.current[tid] || macrosRef.current[tid];
    const serial = serializeSubGraph("h", "h", "event", nodesRef.current, canvasRef.current, variablesRef.current, existing?.inputs || [], existing?.outputs || []);
    setHistory((prev: any) => ({ past: [...prev.past, serial].slice(-50), future: [] }));
  }, [setHistory]);

  const undo = useCallback(() => {
    const tid = activeTabIdRef.current; if (!tid) return;
    setHistory((prev: any) => {
      if (prev.past.length === 0) return prev;
      const past = [...prev.past]; const prevState = past.pop()!;
      const { nodes: n, variables: v } = deserializeSubGraph(prevState);
      const curr = serializeSubGraph(tid, "h", "event", nodesRef.current, canvasRef.current, variablesRef.current);
      setTabNodes(pk => ({ ...pk, [tid]: n })); setTabVariables(pk => ({ ...pk, [tid]: v }));
      return { past, future: [curr, ...prev.future] };
    });
  }, [setHistory]);

  const redo = useCallback(() => {
    const tid = activeTabIdRef.current; if (!tid) return;
    setHistory((prev: any) => {
      if (prev.future.length === 0) return prev;
      const fut = [...prev.future]; const nextState = fut.shift()!;
      const { nodes: n, variables: v } = deserializeSubGraph(nextState);
      const curr = serializeSubGraph(tid, "h", "event", nodesRef.current, canvasRef.current, variablesRef.current);
      setTabNodes(pk => ({ ...pk, [tid]: n })); setTabVariables(pk => ({ ...pk, [tid]: v }));
      return { past: [...prev.past, curr], future: fut };
    });
  }, [setHistory]);

  const deleteSelected = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    if (sIds.size === 0) return;

    setNodes((prev: BaseNode[]) => {
      const idsToDelete = new Set(prev.filter(n => sIds.has(n.id) && !n.isInternal).map(n => n.id));
      if (idsToDelete.size === 0) return prev;
      return prev.filter(n => !idsToDelete.has(n.id)).map(n => {
        const clone = n.clone();
        clone.inputs.forEach(p => p.links = p.links.filter(l => !idsToDelete.has(l.split('-')[0])));
        clone.outputs.forEach(p => p.links = p.links.filter(l => !idsToDelete.has(l.split('-')[0])));
        return clone;
      });
    });
    setSelectedNodeIds([]);
  }, [setNodes, setSelectedNodeIds]);

  /* --- Clipboard & Edit Actions --- */
  const clipboardRef = useRef<BaseNode[]>([]);
  const copy = useCallback(() => {
    const sIds = new Set(selectedNodeIdsRef.current);
    const sel = nodesRef.current.filter(n => sIds.has(n.id) && !n.isInternal);
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
        p.id = `${nid}-${crypto.randomUUID().slice(0, 8)}`;
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

  // --- Interaction Elements & Handlers ---
  const connectPins = useCallback((a: string, b: string) => {
    // ... (existing logic)
    const findPin = (id: string) => {
      for (const n of nodesRef.current) { const p = [...n.inputs, ...n.outputs].find(x => x.id === id); if (p) return { pin: p, node: n }; }
      return null;
    };
    const resA = findPin(a); const resB = findPin(b);
    if (!resA || !resB || !isCompatiblePins(resA.pin, resB.pin)) return;
    saveHistory();
    setNodes((prev: BaseNode[]) => prev.map(n => {
      const newNode = n.clone(); let changed = false;
      const oldLinksToRemove = new Set<string>();
      if (isSingleLinkPin(resA.pin) && resA.pin.links.length > 0) resA.pin.links.forEach(l => oldLinksToRemove.add(l));
      if (isSingleLinkPin(resB.pin) && resB.pin.links.length > 0) resB.pin.links.forEach(l => oldLinksToRemove.add(l));
      [...newNode.inputs, ...newNode.outputs].forEach(p => { if (oldLinksToRemove.has(p.id)) { p.links = p.links.filter(l => l !== a && l !== b); changed = true; } });
      if (n.id === resA.node.id) if (updatePinLink(newNode, a, b)) changed = true;
      if (n.id === resB.node.id) if (updatePinLink(newNode, b, a)) changed = true;
      return changed ? newNode : n;
    }));
  }, [saveHistory, setNodes]);

  const pinIndex = useMemo(() => {
    const map = new Map<string, Pin>();
    nodes.forEach(n => { n.inputs.forEach(p => map.set(p.id, p)); n.outputs.forEach(p => map.set(p.id, p)); });
    return map;
  }, [nodes]);
  const pinIndexRef = useRef(pinIndex); useEffect(() => { pinIndexRef.current = pinIndex; }, [pinIndex]);

  const [gesture, setGesture] = useState<Gesture>(null);
  const gestureRef = useRef<Gesture>(null); useEffect(() => { gestureRef.current = gesture; }, [gesture]);
  const [pendingConnection, setPendingConnection] = useState<Pin | null>(null);
  const [selection, setSelection] = useState<{ startX: number, startY: number, currentX: number, currentY: number } | null>(null);
  const selectionRef = useRef(selection); useEffect(() => { selectionRef.current = selection; }, [selection]);
  const [contextMenu, setContextMenu] = useState<{ x: number, y: number, visible: boolean } | null>(null);

  const onCanvasPointerDown = useCallback((e: React.PointerEvent, groupId?: string) => {
    // Button 1 (Middle) or Button 2 (Right) or Alt+Left (Button 0) for panning
    if (e.button === 1 || e.button === 2 || (e.button === 0 && e.altKey)) {
      setGesture({ type: "pan", lastX: e.clientX, lastY: e.clientY, moved: false, groupId });
      return;
    }
    if (e.button === 0) {
      if (!e.shiftKey) { setSelectedNodeIds([], groupId); }
      setGesture({ type: "select", startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, groupId });
    }
  }, [setSelectedNodeIds]);

  const onNodePointerDown = useCallback((nodeId: string, e: React.PointerEvent, groupId?: string) => {
    e.stopPropagation(); if (e.button !== 0) return;
    const gid = groupId || activeGroupIdRef.current;
    const currentSelected = groups.find(g => g.id === gid)?.selectedNodeIds || [];

    if (e.shiftKey) {
      if (currentSelected.includes(nodeId)) {
        setSelectedNodeIds(prev => prev.filter(id => id !== nodeId), gid);
      } else {
        setSelectedNodeIds(prev => [...prev, nodeId], gid);
      }
    } else {
      if (!currentSelected.includes(nodeId)) {
        setSelectedNodeIds([nodeId], gid);
      }
    }
    setGesture({ type: "drag", nodeId, lastX: e.clientX, lastY: e.clientY, moved: false, groupId });
  }, [setSelectedNodeIds, groups]);

  const onPinPointerDown = useCallback((pinId: string, e: React.PointerEvent, groupId?: string) => {
    e.stopPropagation();

    // Alt + Click to Disconnect
    if (e.altKey && e.button === 0) {
      saveHistory();
      setNodes(prev => prev.map(n => {
        const newNode = n.clone();
        let changed = false;
        [...newNode.inputs, ...newNode.outputs].forEach(p => {
          // Remove if this pin is the target
          if (p.id === pinId) {
            if (p.links.length > 0) { p.links = []; changed = true; }
          }
          // Remove if this pin links TO the target
          else if (p.links.includes(pinId)) {
            p.links = p.links.filter(l => l !== pinId);
            changed = true;
          }
        });
        return changed ? newNode : n;
      }));
      return;
    }

    if (e.button !== 0) return;
    const pin = pinIndexRef.current.get(pinId); if (!pin) return;
    setGesture({ type: "connect", startPin: pin, startX: e.clientX, startY: e.clientY, currentX: e.clientX, currentY: e.clientY, groupId });
  }, [saveHistory, setNodes]);

  const onCanvasWheel = useCallback((e: React.WheelEvent, targetGroupId?: string) => {
    if (e.ctrlKey) {
      e.preventDefault(); const delta = -e.deltaY; const factor = Math.pow(1.1, delta / 100);
      setCanvas(prev => ({ ...prev, scale: clamp(prev.scale * factor, 0.1, 5) }), targetGroupId);
    } else { setCanvas(prev => ({ ...prev, x: prev.x - e.deltaX, y: prev.y - e.deltaY }), targetGroupId); }
  }, [setCanvas]);

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      const g = gestureRef.current; if (!g) return;
      if (g.type === "pan") {
        const dx = e.clientX - g.lastX, dy = e.clientY - g.lastY;
        useViewportStore.getState().setViewport(g.groupId || activeGroupIdRef.current, prev => ({
          ...prev,
          x: prev.x + dx,
          y: prev.y + dy
        }));
        g.lastX = e.clientX; g.lastY = e.clientY; g.moved = true;
        return;
      } else if (g.type === "select") { g.currentX = e.clientX; g.currentY = e.clientY; setSelection({ ...g }); }
      else if (g.type === "connect") { g.currentX = e.clientX; g.currentY = e.clientY; }
      else if (g.type === "drag") {
        const dx = (e.clientX - g.lastX) / canvasRef.current.scale;
        const dy = (e.clientY - g.lastY) / canvasRef.current.scale;
        if (Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
          g.moved = true;
          const sIds = selectedNodeIdsRef.current;
          // 极致优化：直接更新 NodeStore，只有选中的节点组件会运行其内部代码
          sIds.forEach(id => {
            useNodeStore.getState().updateNodePosition(id, dx, dy);
          });
          g.lastX = e.clientX; g.lastY = e.clientY;
        }
      }
      setGesture({ ...g });
    };

    const onUp = (e: PointerEvent) => {
      const g = gestureRef.current; if (!g) return;
      if (g.type === "pan") {
        if (!g.moved && e.button === 2) {
          setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
        }
        const gid = g.groupId || activeGroupIdRef.current;
        const finalCanvas = useViewportStore.getState().viewports[gid];
        if (finalCanvas) {
          setGroups(prev => prev.map(group => group.id === gid ? { ...group, canvas: finalCanvas } : group));
        }
      }
      else if (g.type === "select") {
        const rect = selectionRef.current;
        if (rect) {
          const x1 = Math.min(rect.startX, rect.currentX), y1 = Math.min(rect.startY, rect.currentY);
          const x2 = Math.max(rect.startX, rect.currentX), y2 = Math.max(rect.startY, rect.currentY);
          const gid = g.groupId || activeGroupIdRef.current;
          const newSelectedIds: string[] = [];

          nodesRef.current.forEach(n => {
            const el = document.getElementById(n.id); if (!el) return;
            const r = el.getBoundingClientRect();
            const overlap = !(r.left > x2 || r.right < x1 || r.top > y2 || r.bottom < y1);
            if (overlap) newSelectedIds.push(n.id);
          });
          setSelectedNodeIds(newSelectedIds, gid);
        }
        setSelection(null);
      } else if (g.type === "connect") {
        const target = (e.target as HTMLElement).closest("[data-pin-id]");
        if (target) connectPins(g.startPin.id, target.getAttribute("data-pin-id")!);
        else { setPendingConnection(g.startPin); setContextMenu({ x: e.clientX, y: e.clientY, visible: true }); }
      } else if (g.type === "drag" && g.moved) {
        // 拖拽结束：同步 Store 中的最新位置到持久化状态
        const sIds = selectedNodeIdsRef.current;
        const storeNodes = useNodeStore.getState().nodes;
        setNodes(prev => prev.map(n => sIds.includes(n.id) ? storeNodes[n.id] : n));
        saveHistory();
      }
      setGesture(null);
    };

    window.addEventListener("pointermove", onMove); window.addEventListener("pointerup", onUp);
    return () => { window.removeEventListener("pointermove", onMove); window.removeEventListener("pointerup", onUp); };
  }, [setNodes, setCanvas, saveHistory, connectPins]);

  // --- Initial Project Seed ---
  useEffect(() => {
    if (Object.keys(eventsRef.current).length === 0 && activeGroup.tabs.length === 0) {
      const id = "default-event"; const name = "Event Graph"; const type = "event";
      const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", "On Run", "Internal", { x: 100, y: 100 }, [], [{ name: "Exec", type: "exec" }])];
      setEvents({ [id]: { id, name, type, nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] } });
      setGroups([{ id: "main-group", tabs: [{ id, title: name, type }], activeTabId: id, canvas: { x: 0, y: 0, scale: 1 }, selectedNodeIds: [] }]);
      setSelectedInfo(id, type); setTabNodes({ [id]: tNodes }); setTabHistory({ [id]: { past: [], future: [] } });
      setSelectedNodeIds([], "main-group");
    }
  }, []);

  return (
    <CanvasContext.Provider value={{
      setCanvas, nodes, setNodes, onCanvasWheel, onCanvasPointerDown, onNodePointerDown, onPinPointerDown, selection, gesture, setGesture, contextMenu, setContextMenu,
      saveGraphAs, saveGraph, importGraph, executeGraph, variables, globalVariables,
      selectedItemId, selectedItemType, setSelectedInfo,
      addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
      events, addEvent, updateEvent, deleteEvent,
      functions, addFunction, updateFunction, deleteFunction,
      macros, addMacro, updateMacro, deleteMacro,
      undo, redo, copy, paste, cut, deleteSelected, canUndo: history.past.length > 0, canRedo: history.future.length > 0, saveHistory, connectPins,
      groups, activeGroupId, setActiveGroupId, splitEditorRight, closeGroup,
      activeTabId, setActiveTabId: handleSetActiveTabId, openSubGraph, addTab: () => addEvent("New Item"), closeTab, openSettingsTab, pendingConnection, setPendingConnection,
      tabNodes, tabVariables,
      selectedNodeIds, setSelectedNodeIds
    }}>
      {children}
    </CanvasContext.Provider>
  );
};
