import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import { createNodeFromDescriptor } from '@/features/application/nodeCatalog/createNodeFromDescriptor';
import { DEFAULT_LANGUAGE } from '@/shared/types/settings';
import { useActiveEditorGroup } from '@/features/core/editor/hooks/useActiveEditorGroup';
import { executeCommand } from '@/features/core/history';
import { canDeleteNode } from '@/features/core/dataStore/graphNodeSelectors';
import { logger } from '@/utils/appLogger';

export function useNodeManagement() {
  const { activeTabId } = useActiveEditorGroup();
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;

  const createNode = useCallback(
    async (
      descriptor: NodeCreationDescriptor,
      position: { x: number; y: number },
    ): Promise<boolean> => {
      if (!activeTabId) return false;
      const outcome = await createNodeFromDescriptor({
        graphPath: activeTabId,
        locale,
        descriptor,
        position,
      });
      return outcome.status === 'applied';
    },
    [activeTabId, locale],
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

  return { createNode, deleteNode, deleteNodes };
}
