import { useState, useEffect, useCallback } from "react";
import { useGestureStore } from "@/features/core/gesture";
import { useGraphDataStore } from "@/features/core/dataStore";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { executeCommand } from "@/features/core/history";
import { useLocalizedNodeCatalog } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { BUILTIN_NODE_TYPE_IDS, type VariableNodeTypeId } from "@/features/domain/nodeCatalog";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import type { Pin } from "@/shared/types/domain/pin";
import {
  isGraphResourceDragState,
  isNodeTemplateDragState,
  type SidebarDragState,
} from "@/features/core/dnd";
import type { EditorVariables } from "@/features/core/editor";
import {
  clientToWorldInCanvas,
  findResourceNodeSpawnTemplate,
  isPointInsideCanvas,
  spawnNodeFromTemplate,
  type CreateNodeFn,
  type VariableDropMenu,
} from "./canvasDrop";

export type { VariableDropMenu } from "./canvasDrop";

interface UseCanvasDropParams {
  canvasElementRef: React.RefObject<HTMLDivElement | null>;
  panelInstanceId: string;
  groupId: string;
  graphPath: string | null;
  variables: EditorVariables;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: Pin | null) => void;
  createNode: CreateNodeFn;
  /** Preview canvases skip pointer-only listeners but keep their activatable drop route. */
  enabled?: boolean;
}

/**
 * Canvas drop logic: template drop, variable drop menu, add input, click outside, context menu.
 */
export function useCanvasDrop({
  canvasElementRef,
  panelInstanceId,
  groupId,
  graphPath,
  variables,
  setContextMenu,
  setPendingConnection,
  createNode,
  enabled = true,
}: UseCanvasDropParams) {
  const [variableDropMenu, setVariableDropMenu] = useState<VariableDropMenu | null>(null);
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

  const handleNodeAddInput = useCallback(
    (nodeId: string) => {
      if (!graphPath) return;
      const store = useGraphDataStore.getState();
      const template = store
        .getGraphNodePins(graphPath, nodeId)
        .map((pinId) => store.getGraphPin(graphPath, pinId))
        .find((pin) => pin?.instanceKind === "userCreated" && pin.templateKey)?.templateKey;
      if (!template) return;
      void executeCommand(graphPath, "AddRepeatablePin", { nodeId, template });
    },
    [graphPath],
  );

  const handleNodeRemovePin = useCallback(
    (nodeId: string, pinId: string) => {
      if (!graphPath) return Promise.resolve();
      return executeCommand(graphPath, "RemoveRepeatablePin", { nodeId, pinId }).then(
        () => undefined,
      );
    },
    [graphPath],
  );

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

  const spawnFromVariableMenu = useCallback(
    async (menu: VariableDropMenu, nodeTypeId: VariableNodeTypeId) => {
      setVariableDropMenu(null);
      const resourcePath = variables[menu.variableId]?.resourcePath;
      const template =
        resourcePath && catalog
          ? findResourceNodeSpawnTemplate(catalog.items, resourcePath, "variable", nodeTypeId)
          : null;
      if (!template) {
        refreshCatalog();
        return;
      }
      await spawnNodeFromTemplate(template, { x: menu.worldX, y: menu.worldY }, { createNode });
    },
    [catalog, createNode, refreshCatalog, variables],
  );

  const handleVariableDropGet = useCallback(
    (menu: VariableDropMenu) => spawnFromVariableMenu(menu, BUILTIN_NODE_TYPE_IDS.getVariable),
    [spawnFromVariableMenu],
  );

  const handleVariableDropSet = useCallback(
    (menu: VariableDropMenu) => spawnFromVariableMenu(menu, BUILTIN_NODE_TYPE_IDS.setVariable),
    [spawnFromVariableMenu],
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
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleNodeRemovePin,
    handleContextMenu,
    handleVariableDropGet,
    handleVariableDropSet,
    handleSidebarCanvasDrop,
  };
}
