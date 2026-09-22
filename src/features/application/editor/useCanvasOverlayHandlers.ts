import { useCallback, type RefObject } from "react";
import { createNodeFromDescriptor } from "@/features/application/nodeCatalog/createNodeFromDescriptor";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { useEditorStore } from "@/features/core/editor";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from "@/features/core/graphInteraction/graphInteractionStore";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import { formatErrorMessage } from "@/shared/utils/formatErrorMessage";
import { logger } from "@/features/application/observability/appLogger";
import { clientToWorldInCanvas } from "./canvasDrop";

function interactionStillMatches(
  panelInstanceId: string,
  groupId: string,
  graphPath: string,
  menu: { x: number; y: number },
  sourceAddress: PortAddressDto | null,
): boolean {
  const panel = workbenchLayoutRead.getPanel(panelInstanceId);
  if (
    !panel?.visible ||
    panel.groupId !== groupId ||
    panel.metadata.role !== "editor" ||
    panel.metadata.resourceRef !== graphPath
  )
    return false;
  const contextMenu = useEditorStore.getState().contextMenu;
  if (
    !contextMenu?.visible ||
    contextMenu.panelInstanceId !== panelInstanceId ||
    contextMenu.graphPath !== graphPath ||
    contextMenu.x !== menu.x ||
    contextMenu.y !== menu.y
  )
    return false;
  const interaction = getCanvasInteraction(useGraphInteractionStore.getState(), graphPath, groupId);
  if (interaction.type !== "pendingNodeCreation") return sourceAddress === null;
  return (
    sourceAddress !== null &&
    portAddressKey(interaction.session.source) === portAddressKey(sourceAddress)
  );
}

export function useCanvasOverlayHandlers({
  canvasElementRef,
  panelInstanceId,
  groupId,
  activeResourceRef,
  pendingConnection,
  setContextMenu,
  setPendingConnection,
}: {
  canvasElementRef: RefObject<HTMLDivElement | null>;
  panelInstanceId: string;
  groupId: string;
  activeResourceRef: string | null;
  pendingConnection: PortAddressDto | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (port: PortAddressDto | null) => void;
}) {
  const handleNodePaletteSelect = useCallback(
    async (
      descriptor: NodeCreationDescriptor,
      locale: string,
      contextMenu: { x: number; y: number },
    ) => {
      const canvasElement = canvasElementRef.current;
      if (!canvasElement || !activeResourceRef) return;

      const position = clientToWorldInCanvas(
        canvasElement,
        groupId,
        activeResourceRef,
        contextMenu.x,
        contextMenu.y,
      );
      try {
        const sourceAddress = pendingConnection;
        const outcome = await createNodeFromDescriptor({
          graphPath: activeResourceRef,
          locale,
          descriptor,
          position,
          connectFrom: sourceAddress,
        });
        if (
          outcome.status !== "applied" ||
          !interactionStillMatches(
            panelInstanceId,
            groupId,
            activeResourceRef,
            contextMenu,
            sourceAddress,
          )
        )
          return;
        setContextMenu(null);
        setPendingConnection(null);
      } catch (error) {
        const message = formatErrorMessage(error, "Unknown mutation error");
        logger.graph.error(
          `Failed to create node '${descriptor.nodeTypeId}' in '${activeResourceRef}': ${message}`,
          "NodePalette",
        );
      }
    },
    [
      canvasElementRef,
      panelInstanceId,
      groupId,
      activeResourceRef,
      pendingConnection,
      setContextMenu,
      setPendingConnection,
    ],
  );

  return {
    handleNodePaletteSelect,
  };
}
