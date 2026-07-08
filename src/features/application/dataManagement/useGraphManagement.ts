import { useCallback } from 'react';
import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import {
  createGraphResource,
  deleteGraph,
  duplicateGraph,
  renameGraph,
  type GraphResourceKind,
} from '@/features/application/dataManagement/graphActions';
import { deleteFunctionWithConfirm } from '@/features/application/dataManagement/deleteGraphWithConfirm';
import { uiStore } from '@/features/core/ui/UIStore';
import { logger } from '@/utils/appLogger';
import { resourceKey, useResourceStore } from '@/features/core/resource';

import type { Graph } from '@/shared/types/domain';

type OpenGraphOptions = {
  initialData?: Graph;
  pinned?: boolean;
  targetGroupId?: string;
};

type OpenGraphFn = (
  id: string,
  name: string,
  type: 'event' | 'function',
  options?: OpenGraphOptions,
) => void | Promise<void>;

/**
 * Graph Management Hook
 *
 * 作为编辑器 UI 的 graph 操作门面：
 * - graph resource CRUD 委托给 graphActions
 * - 创建后自动打开时，经 openGraphInEditor → switchEditorTab 激活正文
 * - toast/logger/sidebar 切换等 UI 编排留在这里
 */
export function useGraphManagement(
  openGraph: OpenGraphFn,
) {
  const switchSidebarTab = useSidebarTab();

  const openCreatedGraph = useCallback(async (path: string, kind: 'event' | 'function') => {
    const name =
      useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path;
    await openGraph(path, name, kind);
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
      const id = await createGraphResource('event', baseName);

      logger.graph.info(`Event creation request sent, ID: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        await openCreatedGraph(id, 'event');
      }

      switchSidebarTab('graphs');
    } catch (error) {
      logger.graph.error(`Failed to create event: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      uiStore.showToast(`创建 Event 失败: ${error instanceof Error ? error.message : String(error)}`, 'error');
      throw error;
    }
  }, [openCreatedGraph, switchSidebarTab]);

  const handleEventCreated = useCallback((id: string) => {
    logger.graph.debug(`handleEventCreated: ${id}`, 'GraphManagement');
  }, []);

  const handleEventCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleEventCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    uiStore.showToast(`创建 Event 失败: ${error}`, 'error');
  }, []);

  const deleteEvent = useCallback(async (id: string) => {
    try {
      await deleteGraph(id, 'event');
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
      const id = await createGraphResource('function', baseName);

      logger.graph.info(`Function creation request sent, ID: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        await openCreatedGraph(id, 'function');
      }

      switchSidebarTab('graphs');
    } catch (error) {
      logger.graph.error(`Failed to create function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      uiStore.showToast(`创建 Function 失败: ${error instanceof Error ? error.message : String(error)}`, 'error');
      throw error;
    }
  }, [openCreatedGraph, switchSidebarTab]);

  const handleFunctionCreated = useCallback((id: string) => {
    logger.graph.debug(`handleFunctionCreated: ${id}`, 'GraphManagement');
  }, []);

  const handleFunctionCreatedFailed = useCallback((name: string, error: string) => {
    logger.graph.error(`handleFunctionCreatedFailed: ${name} - ${error}`, 'GraphManagement');
    uiStore.showToast(`创建 Function 失败: ${error}`, 'error');
  }, []);

  const deleteFunction = useCallback(async (id: string) => {
    try {
      await deleteFunctionWithConfirm(id);
    } catch (error) {
      logger.graph.error(`Failed to delete function: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  const renameGraphItem = useCallback(async (id: string, name: string, kind: GraphResourceKind) => {
    try {
      await renameGraph(id, name, kind);
    } catch (error) {
      logger.graph.error(`Failed to rename graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  const duplicateGraphItem = useCallback(async (id: string) => {
    try {
      await duplicateGraph(id);
    } catch (error) {
      logger.graph.error(`Failed to duplicate graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  const createGraph = useCallback((kind: GraphResourceKind) => {
    return kind === 'event' ? addEvent() : addFunction();
  }, [addEvent, addFunction]);

  return {
    addEvent,
    deleteEvent,
    handleEventCreated,
    handleEventCreatedFailed,
    addFunction,
    deleteFunction,
    handleFunctionCreated,
    handleFunctionCreatedFailed,
    renameGraph: renameGraphItem,
    duplicateGraph: duplicateGraphItem,
    createGraph,
  };
}
