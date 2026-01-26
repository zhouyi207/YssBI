import React, { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, SubGraphData } from "../Types/canvas";
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
import { useLayoutStore, LayoutState } from "../../../store/layoutStore";
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
  const activeGroupId = useLayoutStore(useCallback((s: LayoutState) => s.activeGroupId, []));
  const activeEditorGroupId = useLayoutStore(useCallback((s: LayoutState) => s.activeEditorGroupId, []));
  const activeGroupIdRef = useRef(activeGroupId || '');
  useEffect(() => {
    activeGroupIdRef.current = activeGroupId || '';
  }, [activeGroupId]);

  const activeNodeSelector = useCallback((s: LayoutState) => activeGroupId ? s.nodes[activeGroupId] : null, [activeGroupId]);
  const activeNode = useLayoutStore(activeNodeSelector);

  // 获取真正活跃的编辑器节点（用于添加变量、节点等逻辑）
  const activeEditorNodeSelector = useCallback((s: LayoutState) => activeEditorGroupId ? s.nodes[activeEditorGroupId] : null, [activeEditorGroupId]);
  const activeEditorNode = useLayoutStore(activeEditorNodeSelector);
  
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  const groupNodesSelector = useCallback((s: LayoutState) =>
    Object.values(s.nodes)
      .filter(n => n.type === 'component' && n.data?.tabs),
    []);
  const groupNodes = useLayoutStore(useShallow(groupNodesSelector));

  const groups = useMemo(() => groupNodes.map(n => ({
    id: n.id,
    tabs: (n.data?.tabs || []).map(t => ({
      ...t,
      type: t.type || 'event'
    })) as any[],
    activeTabId: n.data?.activeTabId || null,
    selectedNodeIds: n.data?.params?.selectedNodeIds || []
  })), [groupNodes]);

  const setActiveTabId = useCallback((id: string | null, targetGroupId?: string) => {
    const groupId = targetGroupId || activeGroupId;
    if (groupId) {
      useLayoutStore.getState().updateNode(groupId, {
        data: {
          ...useLayoutStore.getState().nodes[groupId].data,
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

  const historySelector = useCallback((s: any) => (activeTabId && s.tabs[activeTabId]) ? s.tabs[activeTabId].history : EMPTY_HISTORY, [activeTabId, EMPTY_HISTORY]);
  const history = useNodeStore(useShallow(historySelector));

  // Current Refs
  const variablesRef = useRef(variables); useEffect(() => { variablesRef.current = variables; }, [variables]);

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

  const [selectedItemId, setSelectedItemId] = useState<string | null>(null);
  const [selectedItemType, setSelectedItemType] = useState<'variable' | 'event' | 'function' | 'macro' | 'setting' | null>(null);

  const setSelectedInfo = useCallback((id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'setting' | null) => {
    setSelectedItemId(id);
    setSelectedItemType(type);
  }, []);

  const handleSetActiveTabId = useCallback((newId: string | null, forceType?: 'event' | 'function' | 'macro' | 'setting', initialData?: SubGraphData, targetGroupId?: string) => {
    setActiveTabId(newId, targetGroupId);
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

  const switchSidebarTab = useCallback((tab: 'events' | 'functions' | 'macros' | 'variables') => {
    const layoutStore = useLayoutStore.getState();
    const sidebarNode = layoutStore.nodes['sidebar'];
    if (sidebarNode) {
      layoutStore.updateNode('sidebar', {
        data: { ...sidebarNode.data, visible: true, currentTab: tab }
      });
      // Also ensure it's not collapsed
      if ((sidebarNode.pixelSize || 0) < 50) {
        layoutStore.updateNode('sidebar', { pixelSize: 260 });
      }
    }
  }, []);

  const openSubGraph = useCallback((id: string, name: string, type: "event" | "function" | "macro", initialData?: SubGraphData) => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || layoutStore.activeGroupId || 'default_editor';

    // 使用 layoutStore 添加标签
    layoutStore.addTab(targetGroupId, {
      id,
      title: name,
      component: 'GraphEditor',
      type
    });

    // 激活目标编辑器组，确保选项卡显示为激活状态
    layoutStore.setActiveGroup(targetGroupId);

    // 传递 targetGroupId 确保在正确的组上设置 activeTabId
    handleSetActiveTabId(id, type, initialData, targetGroupId);
  }, [handleSetActiveTabId]);

  const openSettingsTab = useCallback(() => {
    const layoutStore = useLayoutStore.getState();
    const targetGroupId = layoutStore.activeEditorGroupId || 'default_editor';
    layoutStore.openSettings();
    handleSetActiveTabId("settings", "setting", undefined, targetGroupId);
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

  // Helper to get unique name
  const getUniqueName = useCallback((baseName: string, items: Record<string, { name: string }>) => {
    const names = Object.values(items).map(i => i.name);
    let name = baseName;
    let counter = 1;
    while (names.includes(name)) {
      name = `${baseName}_${counter}`;
      counter++;
    }
    return name;
  }, []);

  const addEvent = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Event", st.events);
    const id = `event-${crypto.randomUUID()}`;
    const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", finalName, "Internal", { x: 50, y: 150 }, [], [{ name: "Exec", type: "exec" }])];
    const sub: SubGraphData = { id, name: finalName, type: "event", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    st.addEvent(id, sub);
    openSubGraph(id, finalName, "event", sub);
    switchSidebarTab('events');
  }, [getUniqueName, openSubGraph, switchSidebarTab]);

  const addFunction = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Function", st.functions);
    const id = `func-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "function_entry", finalName, "Internal", { x: 50, y: 150 }, [], [{ name: "Then", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "function_return", "Return", "Internal", { x: 550, y: 150 }, [{ name: "In", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name: finalName, type: "function", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    st.addFunction(id, sub);
    openSubGraph(id, finalName, "function", sub);
    switchSidebarTab('functions');
  }, [getUniqueName, openSubGraph, switchSidebarTab]);

  const addMacro = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Macro", st.macros);
    const id = `macro-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_inputs", "Inputs", "Internal", { x: 50, y: 150 }, [], [{ name: "In", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_outputs", "Outputs", "Internal", { x: 550, y: 150 }, [{ name: "Out", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name: finalName, type: "macro", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    st.addMacro(id, sub);
    openSubGraph(id, finalName, "macro", sub);
    switchSidebarTab('macros');
  }, [getUniqueName, openSubGraph, switchSidebarTab]);

  const deleteFunction = useCallback((id: string) => { useProjectStore.getState().deleteFunction(id); closeTab(id); }, [closeTab]);
  const deleteEvent = useCallback((id: string) => { useProjectStore.getState().deleteEvent(id); closeTab(id); }, [closeTab]);
  const deleteMacro = useCallback((id: string) => { useProjectStore.getState().deleteMacro(id); closeTab(id); }, [closeTab]);

  const addVariable = useCallback((name?: string, type: string = "int", isGlobal: boolean = false) => {
    const st = useProjectStore.getState();
    const allVars = { ...st.globalVariables };
    // Also include local variables of current tab if any
    const tid = activeTabIdRef.current;
    if (tid) {
      const tabVars = useNodeStore.getState().tabs[tid]?.variables || {};
      Object.assign(allVars, tabVars);
    }

    const finalName = getUniqueName(name || "New Variable", allVars);
    const id = `var-${crypto.randomUUID()}`;
    const v = { name: finalName, type, value: type === "int" ? 0 : type === "bool" ? false : type === "float" ? 0.0 : "" };
    
    if (isGlobal) st.addGlobalVariable(id, v);
    else {
      if (tid) useNodeStore.getState().addVariable(tid, id, v);
    }
    switchSidebarTab('variables');
  }, [getUniqueName, switchSidebarTab]);

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
      const id = "default-event"; const name = "New Graph"; const type = "event";
      const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", "On Run", "Internal", { x: 100, y: 100 }, [], [{ name: "Exec", type: "exec" }])];
      st.addEvent(id, { id, name, type, nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] });

      const targetGroupId = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId || 'default_editor';
      useLayoutStore.getState().addTab(targetGroupId, { id, title: name, component: 'GraphEditor', type });

      setSelectedInfo(id, type);
      useNodeStore.getState().initTab(id, tNodes, {});
    }
  }, []);

  const handleSetActiveGroupId = useCallback((id: string) => useLayoutStore.getState().setActiveGroup(id), []);

  const lastMousePosRef = useRef({ x: 0, y: 0 });

  const getActiveCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
    const gid = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId || 'default_editor';
    const el = document.getElementById(`layout-node-${gid}`);
    if (!el) return { x: 0, y: 0 };
    const rect = el.getBoundingClientRect();
    const currentCanvas = useViewportStore.getState().viewports[gid] || DEFAULT_VIEWPORT;
    return {
      x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
      y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale
    };
  }, []);

  // Global Key & Mouse Listeners
  useEffect(() => {
    const handlePointerMove = (e: PointerEvent) => {
      lastMousePosRef.current = { x: e.clientX, y: e.clientY };
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      // 全局记录修饰键状态 (给 DND 等逻辑使用)
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;

      if (e.key === 'Alt') {
        e.preventDefault(); // 阻止浏览器默认行为（如 Windows 上的菜单聚焦），提高响应速度
        if (e.repeat) return; // Ignore repeats
        useLayoutStore.getState().setAltPressed(true);
      }

      // 处理快捷键
      const isInput =
        document.activeElement?.tagName === "INPUT" ||
        document.activeElement?.tagName === "TEXTAREA" ||
        (document.activeElement as HTMLElement)?.isContentEditable;

      const isControlKey = e.ctrlKey || e.metaKey;

      if (isInput) {
        // 在输入框中，仅允许特定的全局快捷键通过
        const allowedInInput =
          (isControlKey && ["s", "z", "y", "n", "o", "w"].includes(e.key.toLowerCase())) ||
          (isControlKey && e.key === "Tab");

        if (!allowedInInput) return;
      }

      // 快捷键映射
      if (e.key === "Delete" || e.key === "Backspace") {
        deleteSelected();
      } else if (isControlKey && e.key.toLowerCase() === "z") {
        if (e.shiftKey) redo(); else undo();
      } else if (isControlKey && e.key.toLowerCase() === "y") {
        redo();
      } else if (isControlKey && e.key.toLowerCase() === "c") {
        copy();
      } else if (isControlKey && e.key.toLowerCase() === "x") {
        cut();
      } else if (isControlKey && e.key.toLowerCase() === "v") {
        paste(getActiveCanvasLocalPoint(lastMousePosRef.current.x, lastMousePosRef.current.y));
      } else if (isControlKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        if (e.shiftKey) saveGraphAs(); else saveGraph();
      } else if (isControlKey && e.key.toLowerCase() === "o") {
        e.preventDefault();
        importGraph();
      } else if (isControlKey && e.key.toLowerCase() === "n") {
        e.preventDefault();
        addEvent();
      } else if (isControlKey && e.key.toLowerCase() === "w") {
        e.preventDefault();
        const tid = activeTabIdRef.current;
        if (tid) closeTab(tid);
      } else if (isControlKey && e.key === "Tab") {
        e.preventDefault();
        // 这里逻辑较复杂，暂时保留在 CanvasProvider 中通过 ref 或直接访问 store
        const gid = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId;
        if (gid) {
          const node = useLayoutStore.getState().nodes[gid];
          const tabs = node?.data?.tabs || [];
          const activeTabId = node?.data?.activeTabId;
          if (tabs.length > 1) {
            const currentIndex = tabs.findIndex(t => t.id === activeTabId);
            const nextIndex = e.shiftKey ? (currentIndex - 1 + tabs.length) % tabs.length : (currentIndex + 1) % tabs.length;
            setActiveTabId(tabs[nextIndex].id);
          }
        }
      } else if (isControlKey && e.key === "\\") {
        e.preventDefault();
        const gid = useLayoutStore.getState().activeEditorGroupId || useLayoutStore.getState().activeGroupId;
        if (gid) splitEditorRight(gid);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      (window as any)._lastAltKey = e.altKey;
      (window as any)._lastCtrlKey = e.ctrlKey;
      if (e.key === 'Alt') {
        useLayoutStore.getState().setAltPressed(false);
      }
    };

    const handleBlur = () => {
      useLayoutStore.getState().setAltPressed(false);
    };

    window.addEventListener('keydown', handleKeyDown, { capture: true });
    window.addEventListener('keyup', handleKeyUp, { capture: true });
    window.addEventListener('pointermove', handlePointerMove, { capture: true });
    window.addEventListener('blur', handleBlur);

    return () => {
      window.removeEventListener('keydown', handleKeyDown, { capture: true });
      window.removeEventListener('keyup', handleKeyUp, { capture: true });
      window.removeEventListener('pointermove', handlePointerMove, { capture: true });
      window.removeEventListener('blur', handleBlur);
    };
  }, [deleteSelected, undo, redo, copy, cut, paste, saveGraph, saveGraphAs, importGraph, addEvent, closeTab, setActiveTabId, splitEditorRight, getActiveCanvasLocalPoint]);

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
    activeTabId, setActiveTabId, openSubGraph, closeTab, openSettingsTab, pendingConnection, setPendingConnection,
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
    activeTabId, setActiveTabId, openSubGraph, closeTab, openSettingsTab, pendingConnection,
    groups, selectedNodeIds, setSelectedNodeIds
  ]);

  return (
    <CanvasContext.Provider value={contextValue}>
      {children}
    </CanvasContext.Provider>
  );
};
