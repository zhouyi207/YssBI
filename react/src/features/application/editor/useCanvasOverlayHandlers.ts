import { useCallback, type RefObject } from "react";
import { createNodeFromDescriptor } from "@/features/application/nodeCatalog/createNodeFromDescriptor";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import { useEditorStore, type EditorContextMenuState } from "@/features/core/editor";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from "@/features/core/graphInteraction/graphInteractionStore";
import { formatApplicationIpcError } from "@/features/application/errorReference";
import { logger } from "@/utils/frontendLogger";
import { clientToWorldInCanvas } from "./canvasDrop";
import {
  captureEditorCommandTarget,
  isEditorCommandTargetCurrent,
  type EditorCommandTarget,
} from "./editorCommandFocus";

function paletteStillMatches(target: EditorCommandTarget, menu: EditorContextMenuState): boolean {
  return (
    isEditorCommandTargetCurrent(target) &&
    menu.visible &&
    menu.panelInstanceId === target.panelInstanceId &&
    menu.groupId === target.groupId &&
    menu.graphPath === target.resourceRef &&
    useEditorStore.getState().contextMenu === menu
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
      contextMenu: EditorContextMenuState,
      parameters: Record<string, unknown> = {},
      portCounts: Record<string, number> = {},
    ) => {
      const canvasElement = canvasElementRef.current;
      const target = captureEditorCommandTarget(panelInstanceId);
      if (
        !canvasElement ||
        !activeResourceRef ||
        !target ||
        target.groupId !== groupId ||
        target.resourceRef !== activeResourceRef ||
        !paletteStillMatches(target, contextMenu)
      )
        return false;
      const interaction = getCanvasInteraction(
        useGraphInteractionStore.getState(),
        activeResourceRef,
        groupId,
      );
      if (
        interaction.type === "pendingNodeCreation"
          ? interaction.session.panelInstanceId !== panelInstanceId ||
            pendingConnection === null ||
            portAddressKey(interaction.session.source) !== portAddressKey(pendingConnection)
          : pendingConnection !== null
      )
        return false;

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
          parameters,
          portCounts,
          connectFrom: sourceAddress,
        });
        if (outcome.status !== "applied") return false;
        // Successful creation consumes its palette even if its connection gesture ended.
        // Menu identity keeps a newer palette (including one at the same position) intact.
        if (paletteStillMatches(target, contextMenu)) {
          setContextMenu(null);
          if (
            isEditorCommandTargetCurrent(target) &&
            useGraphInteractionStore.getState().interactions[activeResourceRef] === interaction
          )
            setPendingConnection(null);
        }
        return true;
      } catch (error) {
        logger.graph.error(
          `Failed to create node '${descriptor.nodeTypeId}' in '${activeResourceRef}': ${formatApplicationIpcError(error)}`,
          "NodePalette",
        );
        return false;
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
