import { useCallback, useRef } from 'react';
import { Graph } from '@/shared/types/domain';
import { useGraphMetaStore, useGraphDataStore, getGraphById } from '@/features/core/dataStore';
import { GraphService } from '@/services/graph/graphService';
import { getUniqueName } from '@/shared/utils';
import { useSidebarTab } from './useSidebarTab';

/** 兜底：若 EventCreated 未到达，用 get_graph 拉取并打开（解决监听器竞态导致的超时） */
async function fulfillPendingGraph(
  id: string,
  graphType: 'event' | 'function' | 'macro',
  openGraph: (id: string, name: string, type: string, data?: any) => void,
  pendingActionsRef: React.RefObject<Map<string, { callback: () => void; timeout: NodeJS.Timeout }>>
) {
  try {
    const graph = await GraphService.getGraph(id);
    const action = pendingActionsRef.current.get(id);
    if (!action) return; // 事件已处理
    clearTimeout(action.timeout);
    pendingActionsRef.current.delete(id);
    useGraphMetaStore.getState().addGraph({ id: graph.id, name: graph.name, type: graphType, entryNodeId: (graph as any).entryNodeId });
    useGraphDataStore.getState().addGraphFromData(id, graph as any);
    openGraph(id, graph.name, graphType, graph);
  } catch (e) {
    console.warn(`[useGraphManagement] Fallback get_graph for ${id} failed:`, e);
  }
}

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
    
    const metaStore = useGraphMetaStore.getState();
    const events: Record<string, Graph> = {};
    for (const [id, meta] of Object.entries(metaStore.graphs)) {
      if (meta.type === 'event') {
        const g = getGraphById(id);
        if (g) events[id] = g as Graph;
      }
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
          const graph = getGraphById(id);
          if (graph) {
            console.log("[useGraphManagement] Opening newly created event:", id);
            openGraph(id, graph.name, "event", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      // 兜底：若 EventCreated 未到达（监听器竞态等），用 get_graph 拉取并打开
      fulfillPendingGraph(id, 'event', openGraph, pendingActionsRef);
      
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
    const currentGraph = getGraphById(id);
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateEvent(id, fullData);
      useGraphMetaStore.getState().updateGraph(id, data as any);
      if (data.nodes || data.pins || data.connections) {
        useGraphDataStore.getState().addGraphFromData(id, { ...currentGraph, ...data } as any);
      }
    } catch (error) {
      console.error("[useGraphManagement] Failed to update event:", error);
      throw error;
    }
  }, []);

  const deleteEvent = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useGraphDataStore.getState().clearGraph(id);
      useGraphMetaStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete event:", error);
      throw error;
    }
  }, [closeTab]);

  // Functions
  const addFunction = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addFunction called with name:", name);
    
    const metaStore = useGraphMetaStore.getState();
    const functions: Record<string, Graph> = {};
    for (const [id, meta] of Object.entries(metaStore.graphs)) {
      if (meta.type === 'function') {
        const g = getGraphById(id);
        if (g) functions[id] = g as Graph;
      }
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
          const graph = getGraphById(id);
          if (graph) {
            console.log("[useGraphManagement] Opening newly created function:", id);
            openGraph(id, graph.name, "function", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      fulfillPendingGraph(id, 'function', openGraph, pendingActionsRef);
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
    const currentGraph = getGraphById(id);
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateFunction(id, fullData);
      useGraphMetaStore.getState().updateGraph(id, data as any);
      if (data.nodes || data.pins || data.connections) {
        useGraphDataStore.getState().addGraphFromData(id, { ...currentGraph, ...data } as any);
      }
    } catch (error) {
      console.error("[useGraphManagement] Failed to update function:", error);
      throw error;
    }
  }, []);

  const deleteFunction = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useGraphDataStore.getState().clearGraph(id);
      useGraphMetaStore.getState().deleteGraph(id);
      closeTab(id);
    } catch (error) {
      console.error("[useGraphManagement] Failed to delete function:", error);
      throw error;
    }
  }, [closeTab]);

  // Macros
  const addMacro = useCallback(async (name?: string) => {
    console.log("[useGraphManagement] addMacro called with name:", name);
    
    const metaStore = useGraphMetaStore.getState();
    const macros: Record<string, Graph> = {};
    for (const [id, meta] of Object.entries(metaStore.graphs)) {
      if (meta.type === 'macro') {
        const g = getGraphById(id);
        if (g) macros[id] = g as Graph;
      }
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
          const graph = getGraphById(id);
          if (graph) {
            console.log("[useGraphManagement] Opening newly created macro:", id);
            openGraph(id, graph.name, "macro", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      fulfillPendingGraph(id, 'macro', openGraph, pendingActionsRef);
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
    const currentGraph = getGraphById(id);
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateMacro(id, fullData);
      useGraphMetaStore.getState().updateGraph(id, data as any);
      if (data.nodes || data.pins || data.connections) {
        useGraphDataStore.getState().addGraphFromData(id, { ...currentGraph, ...data } as any);
      }
    } catch (error) {
      console.error("[useGraphManagement] Failed to update macro:", error);
      throw error;
    }
  }, []);

  const deleteMacro = useCallback(async (id: string) => {
    try {
      await GraphService.removeGraph(id);
      useGraphDataStore.getState().clearGraph(id);
      useGraphMetaStore.getState().deleteGraph(id);
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
    
    // Nodes：NodeCreatedHandler 已直接更新 Store，此处仅做可选 UI 扩展（如选中新节点）
    handleNodeCreated: useCallback((graphId: string, nodeId: string, data: any) => {
      console.log("[useGraphManagement] handleNodeCreated:", graphId, nodeId, data);
      // 不再重复 addNode，NodeCreatedHandler 已更新 Store
    }, []),
    
    handleNodeDeleted: useCallback((graphId: string, nodeId: string) => {
      console.log("[useGraphManagement] handleNodeDeleted:", graphId, nodeId);
      
      // 更新 dataStore（持久化）
      useGraphDataStore.getState().deleteNode(nodeId);
      
      console.log("[useGraphManagement] Node removed from ProjectStore");
      
      // TODO: 如果该 Graph 当前正在编辑，也需要更新 EditorStore
    }, []),
  };
}
