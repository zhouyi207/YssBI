
import { useCallback, type RefObject } from 'react';
import { createNodeFromDescriptor } from '@/features/application/nodeCatalog/createNodeFromDescriptor';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { Pin } from '@/shared/types/domain/pin';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { useEditorStore } from '@/features/core/editor';
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';
import { clientToWorldInCanvas } from './canvasDrop';
import { EDITOR_MUTATION_CAPABILITIES } from './editorMutationAvailability';

function readPendingConnectionAddress(pendingConnection: Pin | null): PortAddressDto | null {
  if (!pendingConnection) return null;
  return (pendingConnection as Pin & { address?: PortAddressDto }).address ?? null;
}

function getPendingConnectionAddress(pendingConnection: Pin | null): PortAddressDto | null {
  const address = readPendingConnectionAddress(pendingConnection);
  if (pendingConnection && !address) {
    throw new Error('Pending connection is missing its structured port address');
  }
  return address;
}

function samePortAddress(left: PortAddressDto, right: PortAddressDto): boolean {
  if (left.kind !== right.kind || left.nodeId !== right.nodeId) return false;
  return left.kind === 'declared' && right.kind === 'declared'
    ? left.portKey === right.portKey
    : left.kind === 'instance' && right.kind === 'instance'
      && left.templateKey === right.templateKey
      && left.instanceId === right.instanceId;
}

function interactionStillMatches(
  panelInstanceId: string,
  groupId: string,
  graphPath: string,
  menu: { x: number; y: number },
  sourceAddress: PortAddressDto | null,
): boolean {
  const panel = workbenchDockviewPort.getPanel(panelInstanceId);
  if (!panel?.active
    || panel.groupId !== groupId
    || panel.metadata.role !== 'editor'
    || panel.metadata.resourceRef !== graphPath) return false;
  const contextMenu = useEditorStore.getState().contextMenu;
  if (!contextMenu?.visible
    || contextMenu.panelInstanceId !== panelInstanceId
    || contextMenu.graphPath !== graphPath
    || contextMenu.x !== menu.x
    || contextMenu.y !== menu.y) return false;
  const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId);
  if (interaction?.type !== 'pendingNodeCreation') return sourceAddress === null;
  const currentSource = readPendingConnectionAddress(interaction.session.source as Pin | null);
  return sourceAddress === null
    ? interaction.session.source === null
    : currentSource !== null && samePortAddress(currentSource, sourceAddress);
}

export function useCanvasOverlayHandlers({
  canvasElementRef,
  panelInstanceId,
  groupId,
  activeTabId,
  pendingConnection,
  setContextMenu,
  setPendingConnection,
}: {
  canvasElementRef: RefObject<HTMLDivElement | null>;
  panelInstanceId: string;
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

      if (!EDITOR_MUTATION_CAPABILITIES.catalogDescriptors) return;

      const position = clientToWorldInCanvas(
        canvasElement,
        groupId,
        activeTabId,
        contextMenu.x,
        contextMenu.y,
      );
      try {
        const sourceAddress = getPendingConnectionAddress(pendingConnection);
        const outcome = await createNodeFromDescriptor({
          graphPath: activeTabId,
          locale,
          descriptor,
          position,
          connectFrom: sourceAddress,
        });
        if (outcome.status !== 'applied'
          || !interactionStillMatches(panelInstanceId, groupId, activeTabId, contextMenu, sourceAddress)) return;
        setContextMenu(null);
        setPendingConnection(null);
      } catch (error) {
        const message = formatErrorMessage(error, 'Unknown mutation error');
        logger.graph.error(
          `Failed to create node '${descriptor.nodeTypeId}' in '${activeTabId}': ${message}`,
          'NodePalette',
        );
      }
    },
    [
      canvasElementRef,
      panelInstanceId,
      groupId,
      activeTabId,
      pendingConnection,
      setContextMenu,
      setPendingConnection,
    ],
  );

  return {
    handleNodePaletteSelect,
  };
}
