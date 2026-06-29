import { useCallback, useRef } from 'react';
import type { MouseEvent } from 'react';
import { Graph } from '@/shared/types/domain';
import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import { useGraphDataStore, getGraphById } from '@/features/core/dataStore';
import { useResourceStore } from '@/features/core/resource';
import { GraphService } from '@/services/graph/graphService';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import { renameResource } from '@/features/application/resource/resourceActions';
import { logger } from '@/utils/appLogger';

interface PendingAction {
    callback: () => void;
    timestamp: number;
    timeout: NodeJS.Timeout;
    name: string;
}

function upsertGraphResource(graph: Graph, kind: 'event' | 'function'): void {
  useResourceStore.getState().upsertResource({
    id: graph.id,
    kind,
    name: graph.name,
    uri: `yssbi://graph/${kind}/${graph.id}`,
    exists: true,
    loaded: true,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
}

function patchGraphResource(id: string, kind: 'event' | 'function', patch: Partial<{ name: string }>): void {
  useResourceStore.getState().patchResource({ id, kind }, patch);
}

/**
 * Graph Management Hook
 * 
 * 负责 Event/Function 的创建、更新、删除逻辑
 * - 生成唯一名称
 * - 调用 GraphService 与后端通信（后端会创建完整的 Graph 结构并返回 ID）
 * - 后端通过事件系统通知前端，由 projectSync 更新状态
 * - 通过 pendingActions 跟踪待处理的操作，使用 ID 关联
 * - 添加超时机制，防止内存泄漏
 */
export function useGraphManagement(
  openGraph: (id: string, name: string, type: any, data?: any) => void,
  closeTab: (id: string, e?: MouseEvent, options?: { skipDirtyPrompt?: boolean }) => void,
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

  /** 创建后是否自动打开（WatermarkView/Menubar 为 true，Sidebar 为 false） */
  type AddGraphOptions = { openAfterCreate?: boolean };

  // Events
  const addEvent = useCallback(async (name?: string, options?: AddGraphOptions) => {
    const openAfterCreate = options?.openAfterCreate ?? false;

    logger.graph.debug(`addEvent called with name: ${name}, openAfterCreate: ${openAfterCreate}`, 'GraphManagement');

    const baseName = name || DEFAULT_EVENT_NAME;
    logger.graph.debug(`Creating event: ${baseName}`, 'GraphManagement');

    try {
      const id = await GraphService.createEvent(baseName);

      logger.graph.info(`Event creation request sent, ID: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        // 方案 A：创建成功后立即用 get_graph 拉取数据并打开，不依赖事件
        const graph = await GraphService.getGraph(id);
        upsertGraphResource(graph, 'event');
        useGraphDataStore.getState().addGraphFromData(id, {
          ...graph,
          nodes: graph.nodes ?? [],
          pins: graph.pins ?? [],
          connections: graph.connections ?? { connections: [] },
          canvas: graph.canvas ?? { x: 0, y: 0, scale: 1 },
        } as any);
        logger.graph.debug(`Opening newly created event: ${id}`, 'GraphManagement');
        openGraph(id, graph.name, "event", graph);
      }

      switchSidebarTab('graphs');
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
    if (data.name !== undefined && !data.nodes && !data.pins && !data.connections) {
      await renameResource({ id, kind: 'event' }, data.name);
      return;
    }

    const currentGraph = getGraphById(id);
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateEvent(id, fullData as any);
      if (data.name !== undefined) patchGraphResource(id, 'event', { name: data.name });
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
      useResourceStore.getState().removeResource({ id, kind: 'event' });
      closeTab(id, undefined, { skipDirtyPrompt: true });
    } catch (error) {
      logger.graph.error(`Failed to delete event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, [closeTab]);

  // Functions
  const addFunction = useCallback(async (name?: string, options?: AddGraphOptions) => {
    const openAfterCreate = options?.openAfterCreate ?? false;

    logger.graph.debug(`addFunction called with name: ${name}, openAfterCreate: ${openAfterCreate}`, 'GraphManagement');

    const baseName = name || DEFAULT_FUNCTION_NAME;
    logger.graph.debug(`Creating function: ${baseName}`, 'GraphManagement');

    try {
      const id = await GraphService.createFunction(baseName);

      logger.graph.info(`Function creation request sent, ID: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        const graph = await GraphService.getGraph(id);
        upsertGraphResource(graph, 'function');
        useGraphDataStore.getState().addGraphFromData(id, {
          ...graph,
          nodes: graph.nodes ?? [],
          pins: graph.pins ?? [],
          connections: graph.connections ?? { connections: [] },
          canvas: graph.canvas ?? { x: 0, y: 0, scale: 1 },
        } as any);
        logger.graph.debug(`Opening newly created function: ${id}`, 'GraphManagement');
        openGraph(id, graph.name, "function", graph);
      }

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
    if (data.name !== undefined && !data.nodes && !data.pins && !data.connections) {
      await renameResource({ id, kind: 'function' }, data.name);
      return;
    }

    const currentGraph = getGraphById(id);
    if (!currentGraph) return;
    
    const fullData = { ...currentGraph, ...data };
    
    try {
      await GraphService.updateFunction(id, fullData as any);
      if (data.name !== undefined) patchGraphResource(id, 'function', { name: data.name });
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
      useResourceStore.getState().removeResource({ id, kind: 'function' });
      closeTab(id, undefined, { skipDirtyPrompt: true });
    } catch (error) {
      logger.graph.error(`Failed to delete function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
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
