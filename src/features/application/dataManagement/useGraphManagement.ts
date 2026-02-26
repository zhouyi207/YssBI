import { useCallback, useRef } from 'react';
import { Graph } from '@/shared/types/domain';
import { useGraphMetaStore, useGraphDataStore, getGraphById } from '@/features/core/dataStore';
import { GraphService } from '@/services/graph/graphService';
import { getUniqueName } from '@/shared/utils';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import { logger } from '@/utils/appLogger';

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
        logger.graph.warn(`Cleaning up expired action for ${id}`, 'GraphManagement');
        clearTimeout(action.timeout);
        pendingActionsRef.current.delete(id);
      }
    }
  }, []);

  // Events
  const addEvent = useCallback(async (name?: string) => {
    logger.graph.debug(`addEvent called with name: ${name}`, 'GraphManagement');
    
    const metaStore = useGraphMetaStore.getState();
    const events: Record<string, Graph> = {};
    for (const [id, meta] of Object.entries(metaStore.graphs)) {
      if (meta.type === 'event') {
        const g = getGraphById(id);
        if (g) events[id] = g as unknown as Graph;
      }
    }
    
    const finalName = getUniqueName(name || "New Event", Object.values(events));
    
    logger.graph.debug(`Creating event: ${finalName}`, 'GraphManagement');
    
    try {
      // 调用后端 API 创建 Event，获取 ID
      const id = await GraphService.createEvent(finalName);
      
      logger.graph.info(`Event creation request sent, ID: ${id}`, 'GraphManagement');
      
      const timeoutId = setTimeout(() => {
        const action = pendingActionsRef.current.get(id);
        if (action) {
          logger.graph.warn(`EventCreated event not received for ${id}`, 'GraphManagement');
          pendingActionsRef.current.delete(id);
          showToast?.(`创建 Event 超时: ${action.name}`, 'error');
        }
      }, 10000);
      
      // 注册待处理操作：当后端事件到达时打开这个 event
      pendingActionsRef.current.set(id, {
        callback: () => {
          const graph = getGraphById(id);
          if (graph) {
            logger.graph.debug(`Opening newly created event: ${id}`, 'GraphManagement');
            openGraph(id, graph.name, "event", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      
      // 切换到 events 标签页
      switchSidebarTab('graphs');
      
      // 清理过期的 actions
      cleanupExpiredActions();
      
    } catch (error) {
      logger.graph.error(`Failed to create event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      showToast?.(`创建 Event 失败: ${error}`, 'error');
      throw error;
    }
  }, [openGraph, switchSidebarTab, showToast, cleanupExpiredActions]);

  // 处理 Event 创建事件的回调
  const handleEventCreated = useCallback((id: string, data: any) => {
    logger.graph.debug(`handleEventCreated: ${id}`, 'GraphManagement');
    const action = pendingActionsRef.current.get(id);
    if (action) {
      clearTimeout(action.timeout);
      action.callback();
      pendingActionsRef.current.delete(id);
    }
  }, []);

  // 处理 Event 创建失败事件的回调
  const handleEventCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleEventCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    
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
      await GraphService.updateEvent(id, fullData as any);
      useGraphMetaStore.getState().updateGraph(id, data as any);
      if (data.nodes || data.pins || data.connections) {
        useGraphDataStore.getState().addGraphFromData(id, { ...currentGraph, ...data } as any);
      }
    } catch (error) {
      logger.graph.error(`Failed to update event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
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
      logger.graph.error(`Failed to delete event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, [closeTab]);

  // Functions
  const addFunction = useCallback(async (name?: string) => {
    logger.graph.debug(`addFunction called with name: ${name}`, 'GraphManagement');
    
    const metaStore = useGraphMetaStore.getState();
    const functions: Record<string, Graph> = {};
    for (const [id, meta] of Object.entries(metaStore.graphs)) {
      if (meta.type === 'function') {
        const g = getGraphById(id);
        if (g) functions[id] = g as unknown as Graph;
      }
    }
    
    const finalName = getUniqueName(name || "New Function", Object.values(functions));
    
    logger.graph.debug(`Creating function: ${finalName}`, 'GraphManagement');
    
    try {
      const id = await GraphService.createFunction(finalName);
      
      logger.graph.info(`Function creation request sent, ID: ${id}`, 'GraphManagement');
      
      const timeoutId = setTimeout(() => {
        const action = pendingActionsRef.current.get(id);
        if (action) {
          logger.graph.warn(`FunctionCreated event not received for ${id}`, 'GraphManagement');
          pendingActionsRef.current.delete(id);
          showToast?.(`创建 Function 超时: ${action.name}`, 'error');
        }
      }, 10000);
      
      pendingActionsRef.current.set(id, {
        callback: () => {
          const graph = getGraphById(id);
          if (graph) {
            logger.graph.debug(`Opening newly created function: ${id}`, 'GraphManagement');
            openGraph(id, graph.name, "function", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      switchSidebarTab('graphs');
      cleanupExpiredActions();
      
    } catch (error) {
      logger.graph.error(`Failed to create function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      showToast?.(`创建 Function 失败: ${error}`, 'error');
      throw error;
    }
  }, [openGraph, switchSidebarTab, showToast, cleanupExpiredActions]);

  const handleFunctionCreated = useCallback((id: string, data: any) => {
    logger.graph.debug(`handleFunctionCreated: ${id}`, 'GraphManagement');
    const action = pendingActionsRef.current.get(id);
    if (action) {
      clearTimeout(action.timeout);
      action.callback();
      pendingActionsRef.current.delete(id);
    }
  }, []);

  const handleFunctionCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleFunctionCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    
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
      await GraphService.updateFunction(id, fullData as any);
      useGraphMetaStore.getState().updateGraph(id, data as any);
      if (data.nodes || data.pins || data.connections) {
        useGraphDataStore.getState().addGraphFromData(id, { ...currentGraph, ...data } as any);
      }
    } catch (error) {
      logger.graph.error(`Failed to update function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
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
      logger.graph.error(`Failed to delete function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, [closeTab]);

  // Macros
  const addMacro = useCallback(async (name?: string) => {
    logger.graph.debug(`addMacro called with name: ${name}`, 'GraphManagement');
    
    const metaStore = useGraphMetaStore.getState();
    const macros: Record<string, Graph> = {};
    for (const [id, meta] of Object.entries(metaStore.graphs)) {
      if (meta.type === 'macro') {
        const g = getGraphById(id);
        if (g) macros[id] = g as unknown as Graph;
      }
    }
    
    const finalName = getUniqueName(name || "New Macro", Object.values(macros));
    
    logger.graph.debug(`Creating macro: ${finalName}`, 'GraphManagement');
    
    try {
      const id = await GraphService.createMacro(finalName);
      
      logger.graph.info(`Macro creation request sent, ID: ${id}`, 'GraphManagement');
      
      const timeoutId = setTimeout(() => {
        const action = pendingActionsRef.current.get(id);
        if (action) {
          logger.graph.warn(`MacroCreated event not received for ${id}`, 'GraphManagement');
          pendingActionsRef.current.delete(id);
          showToast?.(`创建 Macro 超时: ${action.name}`, 'error');
        }
      }, 10000);
      
      pendingActionsRef.current.set(id, {
        callback: () => {
          const graph = getGraphById(id);
          if (graph) {
            logger.graph.debug(`Opening newly created macro: ${id}`, 'GraphManagement');
            openGraph(id, graph.name, "macro", graph);
          }
        },
        timestamp: Date.now(),
        timeout: timeoutId,
        name: finalName,
      });
      switchSidebarTab('graphs');
      cleanupExpiredActions();
      
    } catch (error) {
      logger.graph.error(`Failed to create macro: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      showToast?.(`创建 Macro 失败: ${error}`, 'error');
      throw error;
    }
  }, [openGraph, switchSidebarTab, showToast, cleanupExpiredActions]);

  const handleMacroCreated = useCallback((id: string, data: any) => {
    logger.graph.debug(`handleMacroCreated: ${id}`, 'GraphManagement');
    const action = pendingActionsRef.current.get(id);
    if (action) {
      clearTimeout(action.timeout);
      action.callback();
      pendingActionsRef.current.delete(id);
    }
  }, []);

  const handleMacroCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleMacroCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    
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
      await GraphService.updateMacro(id, fullData as any);
      useGraphMetaStore.getState().updateGraph(id, data as any);
      if (data.nodes || data.pins || data.connections) {
        useGraphDataStore.getState().addGraphFromData(id, { ...currentGraph, ...data } as any);
      }
    } catch (error) {
      logger.graph.error(`Failed to update macro: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
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
      logger.graph.error(`Failed to delete macro: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
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
      logger.graph.debug(`handleNodeCreated: graphId=${graphId}, nodeId=${nodeId}`, 'GraphManagement');
      // 不再重复 addNode，NodeCreatedHandler 已更新 Store
    }, []),
    
    handleNodeDeleted: useCallback((graphId: string, nodeId: string) => {
      logger.graph.debug(`handleNodeDeleted: graphId=${graphId}, nodeId=${nodeId}`, 'GraphManagement');
      
      // 更新 dataStore（持久化）
      useGraphDataStore.getState().deleteNode(nodeId);
      
      logger.graph.debug('Node removed from ProjectStore', 'GraphManagement');
      
      // TODO: 如果该 Graph 当前正在编辑，也需要更新 EditorStore
    }, []),
  };
}
