import { useCallback } from 'react';
import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import { getGraphById, useProjectIOStore } from '@/features/core/dataStore';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import {
  createGraphResource,
  deleteResource,
} from '@/features/application/resource/resourceActions';
import { logger } from '@/utils/appLogger';

/**
 * Graph Management Hook
 * 
 * 作为编辑器 UI 的 graph 操作门面：
 * - graph resource 创建 / 删除委托给 resourceActions
 * - 创建后自动打开时，通过 ProjectIOStore.loadGraph 拉取正文
 * - toast/logger/sidebar 切换等 UI 编排留在这里
 */
export function useGraphManagement(
  openGraph: (id: string, name: string, type: any, data?: any) => void,
  showToast?: (message: string, type: 'success' | 'error' | 'info') => void
) {
  const switchSidebarTab = useSidebarTab();

  const openCreatedGraph = useCallback(async (id: string, kind: 'event' | 'function') => {
    const loaded = await useProjectIOStore.getState().loadGraph(id);
    if (!loaded) return;
    const graph = getGraphById(id);
    if (!graph) return;
    openGraph(id, graph.name, kind, graph);
  }, [openGraph]);

  /** 创建后是否自动打开（WatermarkView/Menubar 为 true，Sidebar 为 false） */
  type AddGraphOptions = { openAfterCreate?: boolean };

  // Events
  const addEvent = useCallback(async (name?: string, options?: AddGraphOptions) => {
    const openAfterCreate = options?.openAfterCreate ?? false;

    logger.graph.debug(`addEvent called with name: ${name}, openAfterCreate: ${openAfterCreate}`, 'GraphManagement');

    const baseName = name || DEFAULT_EVENT_NAME;
    logger.graph.debug(`Creating event: ${baseName}`, 'GraphManagement');

    try {
      const id = await createGraphResource('event', '', baseName);

      logger.graph.info(`Event creation request sent, ID: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        await openCreatedGraph(id, 'event');
      }

      switchSidebarTab('graphs');
    } catch (error) {
      logger.graph.error(`Failed to create event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      showToast?.(`创建 Event 失败: ${error}`, 'error');
      throw error;
    }
  }, [openCreatedGraph, switchSidebarTab, showToast]);

  // 处理 Event 创建事件的回调
  const handleEventCreated = useCallback((id: string) => {
    logger.graph.debug(`handleEventCreated: ${id}`, 'GraphManagement');
  }, []);

  // 处理 Event 创建失败事件的回调
  const handleEventCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleEventCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    
    showToast?.(`创建 Event 失败: ${error}`, 'error');
  }, [showToast]);

  const deleteEvent = useCallback(async (id: string) => {
    try {
      await deleteResource({ id, kind: 'event' });
    } catch (error) {
      logger.graph.error(`Failed to delete event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  // Functions
  const addFunction = useCallback(async (name?: string, options?: AddGraphOptions) => {
    const openAfterCreate = options?.openAfterCreate ?? false;

    logger.graph.debug(`addFunction called with name: ${name}, openAfterCreate: ${openAfterCreate}`, 'GraphManagement');

    const baseName = name || DEFAULT_FUNCTION_NAME;
    logger.graph.debug(`Creating function: ${baseName}`, 'GraphManagement');

    try {
      const id = await createGraphResource('function', '', baseName);

      logger.graph.info(`Function creation request sent, ID: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        await openCreatedGraph(id, 'function');
      }

      switchSidebarTab('graphs');
    } catch (error) {
      logger.graph.error(`Failed to create function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      showToast?.(`创建 Function 失败: ${error}`, 'error');
      throw error;
    }
  }, [openCreatedGraph, switchSidebarTab, showToast]);

  const handleFunctionCreated = useCallback((id: string) => {
    logger.graph.debug(`handleFunctionCreated: ${id}`, 'GraphManagement');
  }, []);

  const handleFunctionCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleFunctionCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    
    showToast?.(`创建 Function 失败: ${error}`, 'error');
  }, [showToast]);

  const deleteFunction = useCallback(async (id: string) => {
    try {
      await deleteResource({ id, kind: 'function' });
    } catch (error) {
      logger.graph.error(`Failed to delete function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  return {
    // Events
    addEvent,
    deleteEvent,
    handleEventCreated,
    handleEventCreatedFailed,

    // Functions
    addFunction,
    deleteFunction,
    handleFunctionCreated,
    handleFunctionCreatedFailed,
  };
}
