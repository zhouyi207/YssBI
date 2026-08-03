import { useCallback, type RefObject } from 'react';
import { createNodeFromDescriptor } from '@/features/application/nodeCatalog/createNodeFromDescriptor';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { Pin } from '@/shared/types/domain/pin';
import { uiStore } from '@/features/core/ui/UIStore';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';
import { clientToWorldInCanvas } from './canvasDrop';
import {
  EDITOR_MUTATION_CAPABILITIES,
  notifyNodeCreationUnavailable,
} from './editorMutationAvailability';

export function useCanvasOverlayHandlers({
  canvasElementRef,
  groupId,
  activeTabId,
  setContextMenu,
  setPendingConnection,
}: {
  canvasElementRef: RefObject<HTMLDivElement | null>;
  groupId: string;
  activeTabId: string | null;
  pendingConnection: Pin | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: Pin | null) => void;
}) {
  const handleNodePaletteSelect = useCallback(
    async (
      descriptor: NodeCreationDescriptor,
      locale: string,
      contextMenu: { x: number; y: number },
    ) => {
      const canvasElement = canvasElementRef.current;
      if (!canvasElement || !activeTabId) return;

      if (!EDITOR_MUTATION_CAPABILITIES.catalogDescriptors) {
        notifyNodeCreationUnavailable();
        return;
      }

      const position = clientToWorldInCanvas(
        canvasElement,
        groupId,
        activeTabId,
        contextMenu.x,
        contextMenu.y,
      );
      setContextMenu(null);
      setPendingConnection(null);

      try {
        const outcome = await createNodeFromDescriptor({
          graphPath: activeTabId,
          locale,
          descriptor,
          position,
        });
        if (outcome.status === 'stale') return;
      } catch (error) {
        const message = formatErrorMessage(error, 'Unknown mutation error');
        logger.graph.error(
          `Failed to create node '${descriptor.nodeTypeId}' in '${activeTabId}': ${message}`,
          'NodePalette',
        );
        uiStore.showToast(`Failed to create node: ${message}`, 'error', 4000);
      }
    },
    [canvasElementRef, groupId, activeTabId, setContextMenu, setPendingConnection],
  );

  return {
    handleNodePaletteSelect,
  };
}
