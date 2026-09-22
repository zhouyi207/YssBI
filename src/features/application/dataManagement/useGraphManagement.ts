import { useCallback } from "react";

import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from "@/shared/constants/defaultResourceNames";
import { revealWorkbenchView } from "@/modules/workbench/public";
import { PROJECT_TREE_CATEGORY_IDS, useSidebarStore } from "@/features/core/sidebar";
import {
  createGraphResource,
  duplicateGraphResource,
  renameResource,
  type GraphResourceKind,
} from "@/features/application/resource/resourceActions";
import { deleteGraphWithConfirm } from "@/features/application/dataManagement/deleteGraphWithConfirm";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { logger } from "@/features/application/observability/appLogger";
import { resourceKey, useResourceStore } from "@/features/core/resource";

type OpenGraphOptions = {
  targetGroupId?: string;
};

type OpenGraphFn = (
  id: string,
  name: string,
  type: "event" | "function",
  options?: OpenGraphOptions,
) => void | Promise<void>;

/**
 * Graph Management Hook
 *
 * 作为编辑器 UI 的 graph 操作门面：
 * - graph resource CRUD 委托给 resourceActions，由发布协调器统一提交并刷新索引
 * - 创建后自动打开时，经 openGraphInEditor → panel session activation 从文件加载正文
 * - toast/logger/sidebar 切换等 UI 编排留在这里
 */
export function useGraphManagement(openGraph: OpenGraphFn) {
  const openCreatedGraph = useCallback(
    async (path: string, kind: "event" | "function") => {
      const name =
        useResourceStore.getState().resources[resourceKey({ id: path, kind })]?.name ?? path;
      await openGraph(path, name, kind);
    },
    [openGraph],
  );

  /** 创建后是否自动打开。 */
  type AddGraphOptions = { openAfterCreate?: boolean };

  const addGraph = useCallback(
    async (kind: GraphResourceKind, name?: string, options?: AddGraphOptions) => {
      const openAfterCreate = options?.openAfterCreate ?? false;
      const baseName = name || (kind === "event" ? DEFAULT_EVENT_NAME : DEFAULT_FUNCTION_NAME);
      logger.graph.debug(
        `Creating ${kind}: ${baseName}, openAfterCreate: ${openAfterCreate}`,
        "GraphManagement",
      );
      try {
        const id = await createGraphResource(kind, baseName);
        logger.graph.info(`${kind} created at path: ${id}`, "GraphManagement");
        if (openAfterCreate) await openCreatedGraph(id, kind);
        void revealWorkbenchView("project");
        useSidebarStore
          .getState()
          .setCategoryExpanded(
            "project",
            kind === "event"
              ? PROJECT_TREE_CATEGORY_IDS.events
              : PROJECT_TREE_CATEGORY_IDS.functions,
            true,
          );
      } catch (error) {
        logger.graph.error(
          `Failed to create ${kind}: ${formatErrorMessage(error)}`,
          "GraphManagement",
        );
        throw error;
      }
    },
    [openCreatedGraph],
  );

  const deleteGraph = useCallback(async (kind: GraphResourceKind, id: string) => {
    try {
      await deleteGraphWithConfirm(id, kind);
    } catch (error) {
      logger.graph.error(
        `Failed to delete ${kind}: ${formatErrorMessage(error)}`,
        "GraphManagement",
      );
      throw error;
    }
  }, []);

  const addEvent = useCallback(
    (name?: string, options?: AddGraphOptions) => addGraph("event", name, options),
    [addGraph],
  );
  const addFunction = useCallback(
    (name?: string, options?: AddGraphOptions) => addGraph("function", name, options),
    [addGraph],
  );
  const deleteEvent = useCallback((id: string) => deleteGraph("event", id), [deleteGraph]);
  const deleteFunction = useCallback((id: string) => deleteGraph("function", id), [deleteGraph]);

  const renameGraphItem = useCallback(async (id: string, name: string, kind: GraphResourceKind) => {
    try {
      await renameResource({ id, kind }, name);
    } catch (error) {
      logger.graph.error(
        `Failed to rename graph: ${error instanceof Error ? error.message : String(error)}`,
        "GraphManagement",
      );
      throw error;
    }
  }, []);

  const duplicateGraphItem = useCallback(async (id: string) => {
    try {
      await duplicateGraphResource(id);
    } catch (error) {
      logger.graph.error(
        `Failed to duplicate graph: ${error instanceof Error ? error.message : String(error)}`,
        "GraphManagement",
      );
      throw error;
    }
  }, []);

  const createGraph = useCallback(
    (kind: GraphResourceKind) => {
      return addGraph(kind);
    },
    [addGraph],
  );

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
