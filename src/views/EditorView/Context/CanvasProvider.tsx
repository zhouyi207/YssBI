import React, { useRef, useState, useCallback, useEffect, useMemo } from "react";
import { CanvasContext } from "./CanvasContext";
import { CanvasState, SubGraphData } from "../Types/canvas";
import { BaseNode } from "../Types/nodes";
import { VariableDefinition } from "../Types/variables";
import { deserializeSubGraph, serializeSubGraph } from "../Utils/io";
import { ProjectService } from "../../../services/project/projectService";
import { useUI } from "./UIProvider";
import { getNodeDefinition } from "@/features/node-registry";
import { createInternalNode } from "../Utils/internalNodes";
import { useViewportStore } from "@/features/canvas/stores";
import { useNodeStore } from "@/features/node-registry/stores";
import { useTabVariables } from "@/features/node-registry/stores/useNodeStore";
import { useProjectStore } from "@/features/project";
import { useCanvasInteraction } from "@/features/canvas/hooks";
import { useLayoutStore, LayoutState } from "../../../features/layoutStore/layoutStore";
import { useShallow } from 'zustand/react/shallow';
import { deleteNodeInBackend } from "../Utils/backendNodeOps";


/* ================= Helper Functions ================= */

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

const getUniqueName = (baseName: string, items: Record<string, { name: string }>) => {
  const names = Object.values(items).map(i => i.name);
  let name = baseName;
  let counter = 1;
  while (names.includes(name)) {
    name = `${baseName}_${counter}`;
    counter++;
  }
  return name;
};

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
  const dataframes = useProjectStore(useCallback(s => s.dataframes, []));

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
  const [selectedItemType, setSelectedItemType] = useState<'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null>(null);

  const setSelectedInfo = useCallback((id: string | null, type: 'variable' | 'event' | 'function' | 'macro' | 'data' | 'setting' | null) => {
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
  // 注意：不再使用自动延迟同步，改为在特定操作完成时手动触发
  // 例如：拖动结束、创建节点、删除节点等

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
      let p: any;
      let path: string | null = null;

      if (json) {
        // 如果提供了 JSON 内容，解析后同步到后端
        const result = await ProjectService.loadProject(json);
        if (!result) return;
        p = result.project;
        path = result.path;

        // 手动同步到后端状态管理器（触发事件）
        await ProjectService.setProjectData(p, path || undefined, true);
      } else {
        // 从文件加载，直接使用 loadProjectToState（会自动触发事件）
        const result = await ProjectService.loadProjectToState();
        if (!result) return;
        p = result.project;
        path = result.path;
      }

      // 清除旧的 tabs 数据
      useNodeStore.getState().clearTabs();

      // 清除 layoutStore 中的 tabs
      const layoutStore = useLayoutStore.getState();
      const editorGroupId = layoutStore.activeEditorGroupId || 'default_editor';
      const editorNode = layoutStore.nodes[editorGroupId];
      if (editorNode?.data?.tabs) {
        layoutStore.updateNode(editorGroupId, {
          data: { ...editorNode.data, tabs: [], activeTabId: undefined }
        });
      }

      // 加载新项目（同步到前端状态）
      useProjectStore.getState().loadProject(p, path);

      // 打开第一个子图
      const first = Object.values(p.events)[0] || Object.values(p.functions)[0];
      if (first) openSubGraph(first.id, first.name, first.type as any, first);

      showToast("项目已加载", "success", 2000);
    } catch (e) {
      console.error(e);
      showToast("加载项目失败", "error", 3000);
    }
  }, [openSubGraph, showToast]);

  const executeGraph = useCallback(async () => {
    try {
      syncActiveToCollection();

      // 获取当前活跃的 tab
      const currentTabId = activeTabIdRef.current;
      if (!currentTabId) {
        showToast("请先打开一个 Event 才能执行", "warning", 3000);
        return;
      }

      const st = useProjectStore.getState();

      // 检查当前 tab 是否是 event
      const currentEvent = st.events[currentTabId];
      if (!currentEvent) {
        showToast("只能执行 Event，当前打开的不是 Event", "warning", 3000);
        return;
      }

      // 只执行当前的 event
      const eventsToExecute = { [currentTabId]: currentEvent };

      console.log(`[Execute] 执行当前 Event: ${currentEvent.name} (${currentTabId})`);

      const res = await ProjectService.executeProject(
        st.globalVariables,
        eventsToExecute,  // 只传入当前 event
        st.functions,
        st.macros,
        st.dataframes      // 确保也传入 dataframes 元数据
      );

      // 显示所有日志输出
      const logs = res.split('\n').filter(l => l.trim());
      logs.forEach(log => {
        if (log.includes("[Error]")) {
          showToast(log, "error", 5000);
        } else if (log.includes("[NODE PRINT]")) {
          // 提取打印内容并显示
          const printContent = log.replace(/.*\[NODE PRINT\]:\s*/, '');
          showToast(`输出: ${printContent}`, "info", 3000);
          console.log(printContent); // 同时输出到控制台
        } else if (log.includes("[System] Received event")) {
          showToast(log, "info", 2000);
        }
      });

      showToast(`执行完成: ${currentEvent.name}`, "success", 2000);
    } catch (e) {
      console.error("执行失败:", e);
      showToast(`执行失败: ${e}`, "error", 5000);
    }
  }, [syncActiveToCollection, showToast]);

  const executeAllEvents = useCallback(async () => {
    try {
      syncActiveToCollection();
      const st = useProjectStore.getState();

      const eventCount = Object.keys(st.events).length;
      if (eventCount === 0) {
        showToast("没有可执行的 Event", "warning", 3000);
        return;
      }

      console.log(`[Execute] 执行所有 Events (共 ${eventCount} 个)`);

      const res = await ProjectService.executeProject(
        st.globalVariables,
        st.events,  // 执行所有 events
        st.functions,
        st.macros,
        st.dataframes // 确保也传入 dataframes 元数据
      );

      // 显示所有日志输出
      const logs = res.split('\n').filter(l => l.trim());
      logs.forEach(log => {
        if (log.includes("[Error]")) {
          showToast(log, "error", 5000);
        } else if (log.includes("[NODE PRINT]")) {
          // 提取打印内容并显示
          const printContent = log.replace(/.*\[NODE PRINT\]:\s*/, '');
          showToast(`输出: ${printContent}`, "info", 3000);
          console.log(printContent); // 同时输出到控制台
        } else if (log.includes("[System] Received event")) {
          showToast(log, "info", 2000);
        }
      });

      showToast(`执行完成: 共执行 ${eventCount} 个 Events`, "success", 2000);
    } catch (e) {
      console.error("执行失败:", e);
      showToast(`执行失败: ${e}`, "error", 5000);
    }
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

  const addDataFrame = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New DataFrame", st.dataframes);
    const id = `df-${crypto.randomUUID()}`;
    const df: import("../Types/canvas").DataFrameData = {
      id,
      name: finalName,
      columns: [],
      rows: []
    };
    st.addDataFrame(id, df);
    setSelectedInfo(id, 'data');
    switchSidebarTab('data');
  }, [setSelectedInfo, switchSidebarTab]);

  const updateDataFrame = useCallback((id: string, data: Partial<import("../Types/canvas").DataFrameData>) => {
    useProjectStore.getState().updateDataFrame(id, data);
  }, []);

  const deleteDataFrame = useCallback((id: string) => {
    useProjectStore.getState().deleteDataFrame(id);
    if (selectedItemId === id) setSelectedInfo(null, null);
  }, [selectedItemId, setSelectedInfo]);

  const addEvent = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Event", st.events);
    const id = `event-${crypto.randomUUID()}`;
    const tNodes = [createInternalNode(`node-${crypto.randomUUID()}`, "event_on_run", finalName, ["Internal"], { x: 50, y: 150 }, [], [{ name: "Exec", type: "exec" }])];
    const sub: SubGraphData = { id, name: finalName, type: "event", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    st.addEvent(id, sub);
    openSubGraph(id, finalName, "event", sub);
    switchSidebarTab('events');
  }, [openSubGraph, switchSidebarTab]);

  const addFunction = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Function", st.functions);
    const id = `func-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "function_entry", finalName, ["Internal"], { x: 50, y: 150 }, [], [{ name: "Then", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "function_return", "Return", ["Internal"], { x: 550, y: 150 }, [{ name: "In", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name: finalName, type: "function", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    st.addFunction(id, sub);
    openSubGraph(id, finalName, "function", sub);
    switchSidebarTab('functions');
  }, [openSubGraph, switchSidebarTab]);

  const addMacro = useCallback((name?: string) => {
    const st = useProjectStore.getState();
    const finalName = getUniqueName(name || "New Macro", st.macros);
    const id = `macro-${crypto.randomUUID()}`;
    const tNodes = [
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_inputs", "Inputs", ["Internal"], { x: 50, y: 150 }, [], [{ name: "In", type: "exec" }]),
      createInternalNode(`node-${crypto.randomUUID()}`, "macro_outputs", "Outputs", ["Internal"], { x: 550, y: 150 }, [{ name: "Out", type: "exec" }], [])
    ];
    const sub: SubGraphData = { id, name: finalName, type: "macro", nodes: tNodes, canvas: { x: 0, y: 0, scale: 1 }, variables: {}, inputs: [], outputs: [] };
    st.addMacro(id, sub);
    openSubGraph(id, finalName, "macro", sub);
    switchSidebarTab('macros');
  }, [openSubGraph, switchSidebarTab]);

  const deleteFunction = useCallback((id: string) => { useProjectStore.getState().deleteFunction(id); closeTab(id); }, [closeTab]);
  const deleteEvent = useCallback((id: string) => { useProjectStore.getState().deleteEvent(id); closeTab(id); }, [closeTab]);
  const deleteMacro = useCallback((id: string) => { useProjectStore.getState().deleteMacro(id); closeTab(id); }, [closeTab]);

  const addVariable = useCallback(async (name?: string, type: string = "int", isGlobal: boolean = false) => {
    const tid = activeTabIdRef.current;
    let scopeId: string | null = null;

    if (!isGlobal && tid) {
      scopeId = tid;
    }

    try {
      const newVar = await ProjectService.createVariable(scopeId, name, type);

      // 更新前端状态
      if (scopeId) {
        useNodeStore.getState().addVariable(scopeId, newVar.id, newVar);
      } else {
        useProjectStore.getState().addGlobalVariable(newVar.id, newVar);
      }

      switchSidebarTab('variables');
    } catch (e) {
      console.error("Failed to create variable:", e);
    }
  }, [switchSidebarTab]);

  const updateVariable = useCallback((id: string, data: Partial<VariableDefinition>) => {
    const st = useProjectStore.getState();
    const isGlobal = !!st.globalVariables[id];

    // 1. 更新变量定义
    if (isGlobal) {
      st.updateGlobalVariable(id, data);
    } else {
      const nodeStore = useNodeStore.getState();
      let found = false;
      for (const [tid, tabState] of Object.entries(nodeStore.tabs)) {
        if (tabState.variables[id]) {
          nodeStore.updateVariable(tid, id, data);
          found = true;
          break;
        }
      }
      if (!found && !isGlobal) {
        console.warn(`[CanvasProvider] Variable ${id} not found in any scope.`);
      }
    }

    // 2. 更新所有引用该变量的节点 (Pin Type & Variable Name)
    const nodeStore = useNodeStore.getState();
    Object.keys(nodeStore.tabs).forEach(tid => {
      const nodes = nodeStore.getNodes(tid);
      // 检查是否有节点引用此变量
      const needsUpdate = nodes.some(n => n.variableId === id);
      if (!needsUpdate) return;

      const newNodes = nodes.map(n => {
        if (n.variableId !== id) return n;

        // 克隆节点以触发 React 更新
        const clone = n.clone();

        if (data.name) clone.variableName = data.name;
        if (data.data_type) {
          clone.variableType = data.data_type;

          // 更新 Pin 类型 (从而更新颜色)
          if (clone.type === "get_variable") {
            clone.outputs.forEach(p => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
          } else if (clone.type === "set_variable") {
            clone.inputs.forEach(p => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
            clone.outputs.forEach(p => {
              if (p.type !== "exec") p.type = data.data_type!;
            });
          }
        }
        return clone;
      });

      nodeStore.setNodes(tid, newNodes);
    });

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

    // 同步删除到后端
    const tid = activeTabIdRef.current;
    if (tid && idsToDelete.size > 0) {
      console.log(`[BACKEND SYNC] Deleting nodes from backend:`, Array.from(idsToDelete));
      Promise.all(Array.from(idsToDelete).map(id => deleteNodeInBackend(tid, id))).catch(e => {
        console.error('[CanvasProvider] Failed to sync node deletions:', e);
      });
    }
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

  const paste = useCallback(async (pos?: { x: number; y: number }) => {
    if (clipboardRef.current.length === 0) return;
    const tid = activeTabIdRef.current;
    if (!tid) return;

    saveHistory();
    const clipboard = clipboardRef.current.filter(n => getNodeDefinition(n.type));

    // 计算位置偏移
    let tX = pos ? pos.x : -canvasRef.current.x / canvasRef.current.scale + 100;
    let tY = pos ? pos.y : -canvasRef.current.y / canvasRef.current.scale + 100;
    const minX = Math.min(...clipboard.map(n => n.position.x));
    const minY = Math.min(...clipboard.map(n => n.position.y));
    const offX = tX - minX, offY = tY - minY;

    // 准备节点数据
    const tempNodes = clipboard.map(n => {
      const clone = n.clone();
      clone.position = { x: n.position.x + offX, y: n.position.y + offY };
      return clone;
    });

    // 序列化为后端格式（会自动提取内部连接）
    const serializedData = serializeSubGraph("temp", "temp", "event", tempNodes, { x: 0, y: 0, scale: 1 }, {}, [], []);

    try {
      console.log('[CanvasProvider] Pasting nodes via backend...');
      console.log(`[CanvasProvider] Pasting ${serializedData.nodes.length} nodes with ${serializedData.connections.length} connections`);
      
      // 使用新的 createNodesWithConnections 方法，一次性创建节点和连接
      const newSerializedNodes = await ProjectService.createNodesWithConnections(
        tid, 
        serializedData.nodes, 
        serializedData.connections
      );

      // 获取更新后的连接列表
      const updatedConnections = await ProjectService.getConnections(tid);

      // 反序列化回 BaseNode（包含更新后的连接）
      const tempResData: SubGraphData = {
        id: tid,
        name: "temp",
        type: "event",
        nodes: newSerializedNodes,
        connections: updatedConnections,  // 使用后端返回的连接
        canvas: { x: 0, y: 0, scale: 1 },
        variables: {},
        inputs: [],
        outputs: []
      };
      const { nodes: newBaseNodes } = deserializeSubGraph(tempResData);

      setNodes((prev) => [...prev, ...newBaseNodes]);
      setSelectedNodeIds(newBaseNodes.map(n => n.id));

      console.log('[CanvasProvider] Paste completed successfully with connections preserved');
    } catch (e) {
      console.error('[CanvasProvider] Failed to paste nodes:', e);
      showToast("粘贴失败", "error", 2000);
    }
  }, [saveHistory, setNodes, setSelectedNodeIds, activeTabIdRef, showToast]);


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

  // 初始化：检查是否有已有数据，如果有则打开第一个子图
  // 注意：不再自动创建默认事件，用户需要手动创建


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
    saveGraphAs, saveGraph, importGraph, executeGraph, executeAllEvents, variables, globalVariables,
    selectedItemId, selectedItemType, setSelectedInfo,
    addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
    events, addEvent, updateEvent, deleteEvent,
    functions, addFunction, updateFunction, deleteFunction,
    macros, addMacro, updateMacro, deleteMacro,
    dataframes, addDataFrame, updateDataFrame, deleteDataFrame,
    undo, redo, copy, paste, cut, deleteSelected, canUndo: history.past.length > 0, canRedo: history.future.length > 0, saveHistory, connectPins,
    activeGroupId: activeGroupId || 'default_editor',
    activeEditorGroupId: activeEditorGroupId || 'default_editor',
    setActiveGroupId: handleSetActiveGroupId,
    splitEditorRight, closeGroup,
    activeTabId, setActiveTabId, openSubGraph, closeTab, openSettingsTab, pendingConnection, setPendingConnection,
    groups,
    selectedNodeIds, setSelectedNodeIds
  }), [
    setCanvas, setNodes, onCanvasWheel, onCanvasPointerDown, onNodePointerDown, onPinPointerDown, contextMenu,
    saveGraphAs, saveGraph, importGraph, executeGraph, executeAllEvents, variables, globalVariables,
    selectedItemId, selectedItemType, setSelectedInfo,
    addVariable, updateVariable, deleteVariable, promoteVariable, demoteVariable,
    events, addEvent, updateEvent, deleteEvent,
    functions, addFunction, updateFunction, deleteFunction,
    macros, addMacro, updateMacro, deleteMacro,
    dataframes, addDataFrame, updateDataFrame, deleteDataFrame,
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
