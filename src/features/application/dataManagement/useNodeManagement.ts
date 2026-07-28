import { useCallback } from 'react';
import type { BatchCreateNodeRequest } from '@/shared/types/dto/batchCreateNode';
import type { NodeSpawnParams } from '@/shared/types/dto/nodeInstanceParams';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import { executeCommand } from '@/features/core/history';
import { canDeleteNode } from '@/features/core/dataStore/graphNodeSelectors';
import { logger } from '@/utils/appLogger';
import { notifyNodeCreationUnavailable } from '@/features/application/editor/editorMutationAvailability';

export function useNodeManagement() {
  const { activeTabId } = useActiveEditorGroup();

  const createNode = useCallback(
    async (
      _nodeType: string,
      _position: { x: number; y: number },
      _params?: NodeSpawnParams,
    ): Promise<{ nodeId: string; pinIds: string[] } | undefined> => {
      notifyNodeCreationUnavailable();
      return undefined;
    },
    [],
  );

  const createNodes = useCallback(
    async (_requests: BatchCreateNodeRequest[]): Promise<string[]> => {
      notifyNodeCreationUnavailable();
      return [];
    },
    [],
  );

  const deleteNode = useCallback(
    async (nodeId: string): Promise<boolean> => {
      if (!activeTabId) {
        logger.graph.warn('Cannot delete node: no active tab', 'NodeManagement');
        return false;
      }
      if (!canDeleteNode(activeTabId, nodeId)) return false;

      try {
        return await executeCommand(activeTabId, 'DeleteNodes', { nodeIds: [nodeId] });
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
      if (!activeTabId || nodeIds.length === 0) return [];
      const deletableIds = nodeIds.filter((id) => canDeleteNode(activeTabId, id));
      if (deletableIds.length === 0) return [];

      try {
        const applied = await executeCommand(activeTabId, 'DeleteNodes', { nodeIds: deletableIds });
        return applied ? deletableIds : [];
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

  return { createNode, createNodes, deleteNode, deleteNodes };
}
