import { useCallback, useRef } from 'react';
import { Graph } from '@/shared/types/domain';
import { useProjectStore } from '@/features/core/project';
import { GraphService } from '@/services/graph/graphService';
import { getUniqueName } from '@/shared/utils';
import { useSidebarTab } from './useSidebarTab';

interface PendingAction {
    callback: () => void;
    timestamp: number;
    timeout: NodeJS.Timeout;
    name: string;
}

/**
 * Graph Management Hook
 * 
 * 负责 Event/Function/Macro 的创建、更新、删除逻辑
 * - 生成唯一名称
 * - 调用 GraphService 与后端通信（后端会创建完整的 Graph 结构并返回 ID）
 * - 后端通过事件系统通知前端，由 projectSync 更新状态
 * - 通过 pendingActions 跟踪待处理的操作，使用 ID 关联
 * - 添加超时机制，防止内存泄漏
 */
export function useGraphManagement(
  openGraph: (id: string, name: string, type: any, data?: any) => void,
  closeTab: (id: string) => void,
  showToast?: (message: string, type: 'success' | 'error' | 'info') => void
) {
  const switchSidebarTab = useSidebarTab();
  
  // 使用 ref 存储待处理的操作（使用 ID 作为 key）
  const pendingActionsRef = useRef<Map<string, PendingAction>>(new Map());

  // 清理过期的 pending actions
  const cleanupExpiredActions = useCallback(() => {
    const now = Date.now();
    const EXPIRY_TIME = 30000; // 30 秒
    
    for (const [id, action] of pendingActionsRef.current.entries()) {
      if (now - action.timestamp > EXPIRY_TIME) {
        console.warn(`[useGraphManagement] Cleaning up expired action for ${id}`);
        clearTimeout(action.timeout);
        pendingActionsRef.current.delete(id);
      }
    }
  }, []);

  // Events
  const addEvent = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addEvent called with name:", name);
    
    const store = useProjectStore.getState();
    // 从 graphs 中筛选出 events
    const events: Record<string, Graph> = {};
    for (const [id, graph] of Object.entries(store.graphs)) {
      if (graph.type === 'event') events[id] = graph;
    }
    
    const finalName = getUniqueName(name || "New Event", Object.values(events));
    
    console.log("[useGraphManagement] Creating event:", { name: finalName });
    
    try {
      // 调用后端 API 创建 Event，获取 ID
      const id = await GraphService.createEvent(finalName);
      
      console.log("[useGraphManagement] Event creation request sent, ID:", id);
      
      // 设置超时
      const timeoutId = setTimeout(() => {
        const action = pendingActionsRef.current.get(id);
        if (action) {
          console.warn(`[useGraphManagement] Timeout waiting for EventCreated event for ${id}`);
          pendingActionsRef.current.delete(id);
          showToast?.(`创建 Event 超时: ${action.name}`, 'error');
        }
      }, 10000); // 10 秒超时
      
      // 注册待处理操作：当后端事件到达时打开这个 event
      pendingActionsRef.current.set(id, {
        callback: () => {
          const updatedStore = useProjectStore.getState();
          const graph = updatedStore.graphs[id];
          
          if (graph) {
            console.log("[useGraphManagement] Opening newly created event:", id);
            openGraph(id, graph.name, "event", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      // 切换到 events 标签页
      switchSidebarTab('events');
      
      // 清理过期的 actions
      cleanupExpiredActions();
      
    } catch (error) {
      console.error("[useGraphManagement] Failed to create event:", error);
      showToast?.(`创建 Event 失败: ${error}`, 'error');
      throw error;
    }
  }, [openGraph, switchSidebarTab, showToast, cleanupExpiredActions]);

  // 处理 Event 创建事件的回调
  const handleEventCreated = useCallback((id: string, data: any) => {
    console.log("[useGraphManagement] handleEventCreated:", id, data);
    const action = pendingActionsRef.current.get(id);
    if (action) {
      clearTimeout(action.timeout);
      action.callback();
      pendingActionsRef.current.delete(id);
    }
  }, []);

  // 处理 Event 创建失败事件的回调
  const handleEventCreatedFailed = useCallback((name: string, error: string) => {
    console.error("[useGraphManagement] handleEventCreatedFailed:", name, error);
    
    // 查找对应的 pending action（通过名称）
    for (const [id, action] of pendingActionsRef.current.entries()) {
      if (action.name === name) {
        clearTimeout(action.timeout);
        pendingActionsRef.current.delete(id);
        showToast?.(`创建 Event 失败: ${error}`, 'error');
        break;
      }
    }
  }, [showToast]);

  const updateEvent = useCallback(async (id: string, data: Partial<Graph>) => {
    const store = useProjectStore.getState();
    const currentGraph = store.graphs[id];
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateEvent(id, fullData);
      store.updateGraph(id, data);
    } catch (error) {
      console.error("[useGraphManagement] Failed to update event:", error);
      throw error;
    }
  }, []);

  const deleteEvent = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useProjectStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete event:", error);
      throw error;
    }
  }, [closeTab]);

  // Functions
  const addFunction = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addFunction called with name:", name);
    
    const store = useProjectStore.getState();
    const functions: Record<string, Graph> = {};
    for (const [id, graph] of Object.entries(store.graphs)) {
      if (graph.type === 'function') functions[id] = graph;
    }
    
    const finalName = getUniqueName(name || "New Function", Object.values(functions));
    
    console.log("[useGraphManagement] Creating function:", { name: finalName });
    
    try {
      const id = await GraphService.createFunction(finalName);
      
      console.log("[useGraphManagement] Function creation request sent, ID:", id);
      
      const timeoutId = setTimeout(() => {
        const action = pendingActionsRef.current.get(id);
        if (action) {
          console.warn(`[useGraphManagement] Timeout waiting for FunctionCreated event for ${id}`);
          pendingActionsRef.current.delete(id);
          showToast?.(`创建 Function 超时: ${action.name}`, 'error');
        }
      }, 10000);
      
      pendingActionsRef.current.set(id, {
        callback: () => {
          const updatedStore = useProjectStore.getState();
          const graph = updatedStore.graphs[id];
          
          if (graph) {
            console.log("[useGraphManagement] Opening newly created function:", id);
            openGraph(id, graph.name, "function", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      switchSidebarTab('functions');
      cleanupExpiredActions();
      
    } catch (error) {
      console.error("[useGraphManagement] Failed to create function:", error);
      showToast?.(`创建 Function 失败: ${error}`, 'error');
      throw error;
    }
  }, [openGraph, switchSidebarTab, showToast, cleanupExpiredActions]);

  const handleFunctionCreated = useCallback((id: string, data: any) => {
    console.log("[useGraphManagement] handleFunctionCreated:", id, data);
    const action = pendingActionsRef.current.get(id);
    if (action) {
      clearTimeout(action.timeout);
      action.callback();
      pendingActionsRef.current.delete(id);
    }
  }, []);

  const handleFunctionCreatedFailed = useCallback((name: string, error: string) => {
    console.error("[useGraphManagement] handleFunctionCreatedFailed:", name, error);
    
    for (const [id, action] of pendingActionsRef.current.entries()) {
      if (action.name === name) {
        clearTimeout(action.timeout);
        pendingActionsRef.current.delete(id);
        showToast?.(`创建 Function 失败: ${error}`, 'error');
        break;
      }
    }
  }, [showToast]);

  const updateFunction = useCallback(async (id: string, data: Partial<Graph>) => {
    const store = useProjectStore.getState();
    const currentGraph = store.graphs[id];
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateFunction(id, fullData);
      store.updateGraph(id, data);
    } catch (error) {
      console.error("[useGraphManagement] Failed to update function:", error);
      throw error;
    }
  }, []);

  const deleteFunction = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useProjectStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete function:", error);
      throw error;
    }
  }, [closeTab]);

  // Macros
  const addMacro = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addMacro called with name:", name);
    
    const store = useProjectStore.getState();
    const macros: Record<string, Graph> = {};
    for (const [id, graph] of Object.entries(store.graphs)) {
      if (graph.type === 'macro') macros[id] = graph;
    }
    
    const finalName = getUniqueName(name || "New Macro", Object.values(macros));
    
    console.log("[useGraphManagement] Creating macro:", { name: finalName });
    
    try {
      const id = await GraphService.createMacro(finalName);
      
      console.log("[useGraphManagement] Macro creation request sent, ID:", id);
      
      const timeoutId = setTimeout(() => {
        const action = pendingActionsRef.current.get(id);
        if (action) {
          console.warn(`[useGraphManagement] Timeout waiting for MacroCreated event for ${id}`);
          pendingActionsRef.current.delete(id);
          showToast?.(`创建 Macro 超时: ${action.name}`, 'error');
        }
      }, 10000);
      
      pendingActionsRef.current.set(id, {
        callback: () => {
          const updatedStore = useProjectStore.getState();
          const graph = updatedStore.graphs[id];
          
          if (graph) {
            console.log("[useGraphManagement] Opening newly created macro:", id);
            openGraph(id, graph.name, "macro", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      switchSidebarTab('macros');
      cleanupExpiredActions();
      
    } catch (error) {
      console.error("[useGraphManagement] Failed to create macro:", error);
      showToast?.(`创建 Macro 失败: ${error}`, 'error');
      throw error;
    }
  }, [openGraph, switchSidebarTab, showToast, cleanupExpiredActions]);

  const handleMacroCreated = useCallback((id: string, data: any) => {
    console.log("[useGraphManagement] handleMacroCreated:", id, data);
    const action = pendingActionsRef.current.get(id);
    if (action) {
      clearTimeout(action.timeout);
      action.callback();
      pendingActionsRef.current.delete(id);
    }
  }, []);

  const handleMacroCreatedFailed = useCallback((name: string, error: string) => {
    console.error("[useGraphManagement] handleMacroCreatedFailed:", name, error);
    
    for (const [id, action] of pendingActionsRef.current.entries()) {
      if (action.name === name) {
        clearTimeout(action.timeout);
        pendingActionsRef.current.delete(id);
        showToast?.(`创建 Macro 失败: ${error}`, 'error');
        break;
      }
    }
  }, [showToast]);

  const updateMacro = useCallback(async (id: string, data: Partial<Graph>) => {
    const store = useProjectStore.getState();
    const currentGraph = store.graphs[id];
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateMacro(id, fullData);
      store.updateGraph(id, data);
    } catch (error) {
      console.error("[useGraphManagement] Failed to update macro:", error);
      throw error;
    }
  }, []);

  const deleteMacro = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useProjectStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete macro:", error);
      throw error;
    }
  }, [closeTab]);

  return {
    // Events
    addEvent,
    updateEvent,
    deleteEvent,
    handleEventCreated,
    handleEventCreatedFailed,

    // Functions
    addFunction,
    updateFunction,
    deleteFunction,
    handleFunctionCreated,
    handleFunctionCreatedFailed,

    // Macros
    addMacro,
    updateMacro,
    deleteMacro,
    handleMacroCreated,
    handleMacroCreatedFailed,
    
    // Nodes (TODO: 实现节点创建和删除的处理)
    handleNodeCreated: useCallback((graphId: string, nodeId: string, data: any) => {
      console.log("[useGraphManagement] handleNodeCreated:", graphId, nodeId, data);
      
      // 将后端的 NodeInstanceDTO 转换为前端的 Node 对象
      const node = {
        id: data.id || nodeId,
        node_type: data.node_type,
        category: data.category || [],
        title: data.title || data.node_type,
        inputs: [], // TODO: 需要从 Pin 数据转换
        outputs: [], // TODO: 需要从 Pin 数据转换
        ui_style: data.ui_style || 'default',
        description: data.description,
        position: data.position || { x: 0, y: 0 },
        isInternal: false,
      };
      
      // 更新 ProjectStore（持久化）
      const projectStore = useProjectStore.getState();
      projectStore.addNodeToGraph(graphId, node);
      
      console.log("[useGraphManagement] Node added to ProjectStore");
      
      // TODO: 如果该 Graph 当前正在编辑，也需要更新 EditorStore
    }, []),
    
    handleNodeDeleted: useCallback((graphId: string, nodeId: string) => {
      console.log("[useGraphManagement] handleNodeDeleted:", graphId, nodeId);
      
      // 更新 ProjectStore（持久化）
      const projectStore = useProjectStore.getState();
      projectStore.removeNodeFromGraph(graphId, nodeId);
      
      console.log("[useGraphManagement] Node removed from ProjectStore");
      
      // TODO: 如果该 Graph 当前正在编辑，也需要更新 EditorStore
    }, []),
  };
}
