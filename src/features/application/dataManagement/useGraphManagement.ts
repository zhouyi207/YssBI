import { useCallback } from 'react';
import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import { useSidebarTab } from '@/features/application/editor/useSidebarTab';
import { useSidebarStore } from '@/features/core/sidebar';
import {
  createGraphResource,
  duplicateGraphResource,
  renameResource,
  type GraphResourceKind,
} from '@/features/application/resource/resourceActions';
import { deleteGraphWithConfirm } from '@/features/application/dataManagement/deleteGraphWithConfirm';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';
import { resourceKey, useResourceStore } from '@/features/core/resource';

type OpenGraphOptions = {
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
 * - graph resource CRUD 委托给 resourceActions（file-first：创建写盘 + refreshResourceIndex）
 * - 创建后自动打开时，经 openGraphInEditor → switchEditorTab 从文件加载正文
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

      logger.graph.info(`Event created at path: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        await openCreatedGraph(id, 'event');
      }

      switchSidebarTab('graphs');
      useSidebarStore.getState().setSectionExpanded('graphsEvent', true);
    } catch (error) {
      const message = formatErrorMessage(error);
      logger.graph.error(`Failed to create event: ${message}`, 'GraphManagement');
      logger.notify.error(`创建 Event 失败: ${message}`, "UI");
      throw error;
    }
  }, [openCreatedGraph, switchSidebarTab]);

  const deleteEvent = useCallback(async (id: string) => {
    try {
      await deleteGraphWithConfirm(id, 'event');
    } catch (error) {
      logger.graph.error(`Failed to delete event: ${formatErrorMessage(error)}`, 'GraphManagement');
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

      logger.graph.info(`Function created at path: ${id}`, 'GraphManagement');

      if (openAfterCreate) {
        await openCreatedGraph(id, 'function');
      }

      switchSidebarTab('graphs');
      useSidebarStore.getState().setSectionExpanded('graphsFunction', true);
    } catch (error) {
      const message = formatErrorMessage(error);
      logger.graph.error(`Failed to create function: ${message}`, 'GraphManagement');
      logger.notify.error(`创建 Function 失败: ${message}`, "UI");
      throw error;
    }
  }, [openCreatedGraph, switchSidebarTab]);

  const deleteFunction = useCallback(async (id: string) => {
    try {
      await deleteGraphWithConfirm(id, 'function');
    } catch (error) {
      logger.graph.error(`Failed to delete function: ${formatErrorMessage(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  const renameGraphItem = useCallback(async (id: string, name: string, kind: GraphResourceKind) => {
    try {
      await renameResource({ id, kind }, name);
    } catch (error) {
      logger.graph.error(`Failed to rename graph: ${error instanceof Error ? error.message : String(error)}`, 'GraphManagement');
      throw error;
    }
  }, []);

  const duplicateGraphItem = useCallback(async (id: string) => {
    try {
      await duplicateGraphResource(id);
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
    addFunction,
    deleteFunction,
    renameGraph: renameGraphItem,
    duplicateGraph: duplicateGraphItem,
    createGraph,
  };
}
