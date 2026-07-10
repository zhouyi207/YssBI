import { useCallback } from 'react';
import { NodeService } from '@/services';
import type { BatchCreateNodeRequest } from '@/shared/types/dto/batchCreateNode';
import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams';
import { useLayoutStore, LayoutState } from '@/features/core/layout/layoutStore';
import { executeCommand } from '@/features/core/history';
import { isShellNode } from '@/features/core/dataStore/graphNodeSelectors';
import { logger } from '@/utils/appLogger';

/**
 * Node Management Hook (CQRS Pattern)
 *
 * 命令流：UI → NodeService / executeCommand → Backend
 * 事件流：Backend → ProjectListener → NodeEventHandler（直接更新 Store）
 */
export function useNodeManagement() {
  const activeEditorNode = useLayoutStore((s: LayoutState) =>
    s.activeEditorGroupId ? s.nodes[s.activeEditorGroupId] : null,
  );
  const activeTabId = activeEditorNode?.data?.activeTabId || null;

  const createNode = useCallback(
    async (
      nodeType: string,
      position: { x: number; y: number },
      params?: NodeSpawnParams,
    ): Promise<{ nodeId: string; pinIds: string[] } | undefined> => {
      if (!activeTabId) {
        logger.graph.warn('Cannot create node: no active tab', 'NodeManagement');
        return undefined;
      }

      try {
        const context = await executeCommand(activeTabId, 'CreateNode', {
          nodeType,
          x: position.x,
          y: position.y,
          params,
        });
        return context as { nodeId: string; pinIds: string[] } | undefined;
      } catch (error) {
        logger.graph.error(
          `Failed to create node: ${error instanceof Error ? error.message : String(error)}`,
          'NodeManagement',
        );
        throw error;
      }
    },
    [activeTabId],
  );

  const createNodes = useCallback(
    async (requests: BatchCreateNodeRequest[]): Promise<string[]> => {
      if (!activeTabId || requests.length === 0) {
        logger.graph.warn('Cannot create nodes: no active tab or empty requests', 'NodeManagement');
        return [];
      }

      try {
        return await NodeService.batchCreateNodes(activeTabId, requests);
      } catch (error) {
        logger.graph.error(
          `Failed to create nodes: ${error instanceof Error ? error.message : String(error)}`,
          'NodeManagement',
        );
        return [];
      }
    },
    [activeTabId],
  );

  const deleteNode = useCallback(
    async (nodeId: string): Promise<boolean> => {
      if (!activeTabId) {
        logger.graph.warn('Cannot delete node: no active tab', 'NodeManagement');
        return false;
      }

      if (isShellNode(activeTabId, nodeId)) {
        logger.graph.warn('Skip deleting system-managed shell node', 'NodeManagement');
        return false;
      }

      try {
        await NodeService.deleteNode(activeTabId, nodeId);
        return true;
      } catch (error) {
        logger.graph.error(
          `Failed to delete node: ${error instanceof Error ? error.message : String(error)}`,
          'NodeManagement',
        );
        return false;
      }
    },
    [activeTabId],
  );

  const deleteNodes = useCallback(
    async (nodeIds: string[]): Promise<string[]> => {
      if (!activeTabId || nodeIds.length === 0) {
        logger.graph.warn('Cannot delete nodes: no active tab or empty node IDs', 'NodeManagement');
        return [];
      }

      const deletableIds = nodeIds.filter((id) => !isShellNode(activeTabId, id));
      if (deletableIds.length === 0) return [];

      try {
        const results = await Promise.allSettled(
          deletableIds.map((id) => NodeService.deleteNode(activeTabId, id)),
        );

        const deletedIds: string[] = [];
        results.forEach((result, index) => {
          if (result.status === 'fulfilled') {
            deletedIds.push(deletableIds[index]);
          } else {
            logger.graph.error(
              `Failed to delete node: ${deletableIds[index]} - ${String(result.reason)}`,
              'NodeManagement',
            );
          }
        });

        return deletedIds;
      } catch (error) {
        logger.graph.error(
          `Failed to delete nodes: ${error instanceof Error ? error.message : String(error)}`,
          'NodeManagement',
        );
        return [];
      }
    },
    [activeTabId],
  );

  return {
    createNode,
    createNodes,
    deleteNode,
    deleteNodes,
  };
}
