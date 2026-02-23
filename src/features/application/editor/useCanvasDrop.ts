import { useState, useEffect, useCallback } from "react";
import { useViewportStore } from "@/features/core/viewport";
import { useGestureStore } from "@/features/core/gesture";
import { useVariableStore } from "@/features/core/dataStore";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { executeCommand } from "@/features/core/history";
import { logger } from '@/utils/appLogger';

export interface VariableDropMenu {
  x: number;
  y: number;
  worldX: number;
  worldY: number;
  variableId: string;
  variableName: string;
  variableType: string;
  containerType?: string;
}

interface UseCanvasDropParams {
  canvasRef: React.RefObject<HTMLDivElement | null>;
  groupId: string;
  graphId: string | null;
  variables: Record<string, any>;
  functions: Record<string, any>;
  macros: Record<string, any>;
  setNodes: (updater: (prev: any[]) => any[]) => void;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: any) => void;
  createNode: (nodeType: string, position: { x: number; y: number }, params?: Record<string, unknown>) => Promise<{ nodeId: string; pinIds: string[] } | undefined>;
}

/**
 * Canvas drop logic: template drop, variable drop menu, add input, click outside, context menu.
 * Extracted from Canvas.tsx - view should only consume this hook.
 */
export function useCanvasDrop({
  canvasRef,
  groupId,
  graphId: _graphId,
  variables,
  functions,
  macros,
  setNodes,
  setContextMenu,
  setPendingConnection,
  createNode,
}: UseCanvasDropParams) {
  const [variableDropMenu, setVariableDropMenu] = useState<VariableDropMenu | null>(null);

  useEffect(() => {
    const handleClickOutside = (e: PointerEvent) => {
      const target = e.target as HTMLElement;
      if (target.closest(".menu-container") || target.closest(".sidebar-container")) {
        return;
      }
      setContextMenu(null);
      setPendingConnection(null);
      setVariableDropMenu(null);
    };
    window.addEventListener("pointerdown", handleClickOutside, true);
    return () => window.removeEventListener("pointerdown", handleClickOutside, true);
  }, [setContextMenu, setPendingConnection]);

  const handleNodeAddInput = useCallback(
    (nodeId: string) => {
      if (!_graphId) return;
      executeCommand(_graphId, 'AddRepeatablePin', { nodeId, slotIndex: 0 });
    },
    [_graphId]
  );

  const handleNodeRemovePin = useCallback(
    (nodeId: string, pinId: string) => {
      if (!_graphId) return;
      executeCommand(_graphId, 'RemoveRepeatablePin', { nodeId, pinId });
    },
    [_graphId]
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
        target.closest(".menu-container") ||
        target.closest(".hud-container")
      ) {
        return;
      }
      setContextMenu({ x: e.clientX, y: e.clientY, visible: true });
    },
    [setContextMenu]
  );

  const handleDropTemplate = useCallback(
    async (dragState: any, event: MouseEvent | PointerEvent) => {
      const el = canvasRef.current;
      if (!el) return;

      const rect = el.getBoundingClientRect();
      const isInside =
        dragState.x >= rect.left &&
        dragState.x <= rect.right &&
        dragState.y >= rect.top &&
        dragState.y <= rect.bottom;
      if (!isInside) return;

      const screenX = dragState.x - rect.left;
      const screenY = dragState.y - rect.top;
      const currentCanvas = useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
      const x = (screenX - currentCanvas.x) / currentCanvas.scale;
      const y = (screenY - currentCanvas.y) / currentCanvas.scale;

      // DataFrame 拖放
      if (dragState.template.category === "Data") {
        const nodeType = dragState.template.nodeType;
        const params = {
          dataframeId: dragState.template.variableId,
          variableName: dragState.template.variableName,
        };
        await createNode(nodeType, { x, y }, params);
        return;
      }

      // 变量拖放
      if (dragState.template.category === "Variable") {
        const allVariables = useVariableStore.getState().variables;
        if (
          !variables[dragState.template.variableId] &&
          !allVariables[dragState.template.variableId]
        ) {
          logger.graph.warn('Variable no longer exists. Aborting drop', 'CanvasDrop');
          return;
        }

        const varParams = {
          variableId: dragState.template.variableId,
          variableName: dragState.template.variableName,
          variableType: dragState.template.variableType,
        };

        let spawnType: "Variables:Get Variable" | "Variables:Set Variable" | null = null;
        if (event.altKey) spawnType = "Variables:Set Variable";
        else if (event.ctrlKey) spawnType = "Variables:Get Variable";

        if (spawnType) {
          await createNode(spawnType, { x, y }, varParams);
          return;
        }

        const elements = document.elementsFromPoint(dragState.x, dragState.y);
        const pinEl = elements.find((e) => e.closest("[data-pin-id]"))?.closest("[data-pin-id]");
        const targetPinId = pinEl?.getAttribute("data-pin-id");
        if (targetPinId) {
          await createNode("Variables:Get Variable", { x, y }, varParams);
          return;
        }

        // 否则弹出选择菜单
        setVariableDropMenu({
          x: dragState.x,
          y: dragState.y,
          worldX: x,
          worldY: y,
          variableId: dragState.template.variableId,
          variableName: dragState.template.variableName,
          variableType: dragState.template.variableType,
          containerType: dragState.template.containerType,
        });
        return;
      }

      // Function / Macro 拖放
      if (
        dragState.template.nodeType === "Functions:Call Function" ||
        dragState.template.nodeType === "Macros:Call Macro"
      ) {
        const nodeType = dragState.template.nodeType;
        const subId = dragState.template.subGraphId;
        const subData = nodeType === "Functions:Call Function" ? functions[subId] : macros[subId];
        if (!subData) return;

        await createNode(nodeType, { x, y }, { subGraphId: subId });
        return;
      }

      // 普通节点拖放
      await createNode(dragState.template.nodeType, { x, y });
    },
    [
      canvasRef,
      groupId,
      variables,
      functions,
      macros,
      createNode,
      setVariableDropMenu,
    ]
  );

  useEffect(() => {
    canvasDropHandlerStore.setHandler(groupId, handleDropTemplate as any);
    return () => canvasDropHandlerStore.setHandler(groupId, null);
  }, [groupId, handleDropTemplate]);

  return {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleNodeRemovePin,
    handleContextMenu,
  };
}
