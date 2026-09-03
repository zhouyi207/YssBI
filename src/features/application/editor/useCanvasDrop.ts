import { useEffect, useCallback } from "react";
import { useGestureStore } from "@/features/core/gesture";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import {
  isGraphResourceDragState,
  isNodeTemplateDragState,
  type SidebarDragState,
} from "@/features/core/dnd";
import {
  clientToWorldInCanvas,
  findResourceNodeSpawnTemplate,
  isPointInsideCanvas,
  spawnNodeFromTemplate,
  type CreateNodeFn,
} from "./canvasDrop";

interface UseCanvasDropParams {
  canvasElementRef: React.RefObject<HTMLDivElement | null>;
  panelInstanceId: string;
  groupId: string;
  graphPath: string | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: PinData | null) => void;
  createNode: CreateNodeFn;
  /** Preview canvases skip pointer-only listeners but keep their activatable drop route. */
  enabled?: boolean;
}

/**
 * Canvas drop logic: template drop, variable drop menu, click outside, and context menu.
 */
export function useCanvasDrop({
  canvasElementRef,
  panelInstanceId,
  groupId,
  graphPath,
  setContextMenu,
  setPendingConnection,
  createNode,
  enabled = true,
}: UseCanvasDropParams) {
  const { catalog, refresh: refreshCatalog } = useLocalizedNodeCatalog();

  useEffect(() => {
    if (!enabled) return;
    const handleClickOutside = (e: PointerEvent) => {
      const target = e.target as HTMLElement;
      if (
        target.closest(".menu-container") ||
        target.closest(".sidebar-container") ||
        target.closest(".menubar-container")
      ) {
        return;
      }

      const canvasEl = canvasElementRef.current;
      if (canvasEl && !canvasEl.contains(target)) {
        return;
      }

      setPendingConnection(null);
    };
    return addGlobalEventListener(window, "pointerdown", handleClickOutside, { capture: true });
  }, [enabled, canvasElementRef, setPendingConnection]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      if (useGestureStore.getState().consumeSuppressContextMenu()) {
        return;
      }
      const target = e.target as HTMLElement;
      if (
        target.closest(".menubar-container") ||
        target.closest(".sidebar-container") ||
        target.closest(".menu-container")
      ) {
        return;
      }
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    },
    [setContextMenu],
  );

  const handleSidebarCanvasDrop = useCallback(
    async (dragState: SidebarDragState) => {
      const canvas = canvasElementRef.current;
      if (!canvas || !graphPath || !isPointInsideCanvas(canvas, dragState.x, dragState.y))
        return false;

      let template = isNodeTemplateDragState(dragState) ? dragState.template : null;
      if (isGraphResourceDragState(dragState)) {
        if (
          dragState.sidebarResource.type !== "function" ||
          dragState.sidebarResource.id === graphPath
        )
          return false;
        template = catalog
          ? findResourceNodeSpawnTemplate(catalog.items, dragState.sidebarResource.id, "function")
          : null;
        if (!template) {
          refreshCatalog();
          return false;
        }
      }

      if (!template) return false;
      const worldPosition = clientToWorldInCanvas(
        canvas,
        groupId,
        graphPath,
        dragState.x,
        dragState.y,
      );
      return spawnNodeFromTemplate(template, worldPosition, { createNode });
    },
    [canvasElementRef, catalog, createNode, graphPath, groupId, refreshCatalog],
  );

  useEffect(() => {
    canvasDropHandlerStore.setHandler(panelInstanceId, (dragState) =>
      handleSidebarCanvasDrop(dragState),
    );
    return () => canvasDropHandlerStore.setHandler(panelInstanceId, null);
  }, [handleSidebarCanvasDrop, panelInstanceId]);

  return {
    handleContextMenu,
  };
}
