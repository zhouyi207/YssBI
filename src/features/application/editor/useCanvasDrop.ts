import { useState, useEffect, useCallback, useRef } from "react";
import { useViewportStore } from "@/features/core/viewport";
import { useVariableStore } from "@/features/core/dataStore";
import { createNodeFromTemplate } from "@/features/core/dataStore";
import { buildCreateNodeRequest } from "@/shared/utils/editor";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";
import { useSidebarDragStore, canvasDropHandlerStore } from "@/features/core/sidebarDrag";

export interface VariableDropMenu {
  x: number;
  y: number;
  worldX: number;
  worldY: number;
  variableId: string;
  variableName: string;
  variableType: string;
  variableIsArray?: boolean;
}

interface UseCanvasDropParams {
  canvasRef: React.RefObject<HTMLDivElement | null>;
  groupId: string;
  variables: Record<string, any>;
  functions: Record<string, any>;
  macros: Record<string, any>;
  setNodes: (updater: (prev: any[]) => any[]) => void;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: any) => void;
  saveHistory: () => void;
  createNode: (nodeType: string, position: { x: number; y: number }) => Promise<void>;
}

/**
 * Canvas drop logic: template drop, variable drop menu, add input, click outside, context menu.
 * Extracted from Canvas.tsx - view should only consume this hook.
 */
export function useCanvasDrop({
  canvasRef,
  groupId,
  variables,
  functions,
  macros,
  setNodes,
  setContextMenu,
  setPendingConnection,
  saveHistory,
  createNode,
}: UseCanvasDropParams) {
  const activeDrag = useSidebarDragStore((s) => s.activeDrag);
  const [variableDropMenu, setVariableDropMenu] = useState<VariableDropMenu | null>(null);
  const prevDragRef = useRef(activeDrag);

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
    (id: string) => {
      saveHistory();
      setNodes((prev) =>
        prev.map((node) => {
          if (node.id === id) {
            const newNode = node.clone();
            const newIndex = newNode.inputs.length;
            // TODO: 需要后端 add_node_input API，由后端分配 pin ID。当前为本地临时 ID。
            newNode.addInput({
              id: `pending_${id}_input_${newIndex}`,
              nodeId: id,
              name: String.fromCharCode(65 + newIndex),
              type: "int",
              direction: "input",
              links: [],
            });
            return newNode;
          }
          return node;
        })
      );
    },
    [saveHistory, setNodes]
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
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

      const elements = document.elementsFromPoint(dragState.x, dragState.y);
      const pinEl = elements.find((e) => e.closest("[data-pin-id]"))?.closest("[data-pin-id]");
      const targetPinId = pinEl?.getAttribute("data-pin-id");

      if (dragState.template.category === "Data") {
        const newNode = createNodeFromTemplate(
          { x, y },
          currentCanvas.scale,
          dragState.template.nodeType,
          { variableId: dragState.template.variableId, variableName: dragState.template.variableName }
        );
        if (newNode) {
          await createNode(newNode.nodeType, { x: newNode.position.x, y: newNode.position.y });
          // TODO: connect after node created - backend returns nodeId, need pin IDs for connection
        }
        return;
      }

      if (dragState.template.category === "Variable") {
        const allVariables = useVariableStore.getState().variables;
        if (
          !variables[dragState.template.variableId] &&
          !allVariables[dragState.template.variableId]
        ) {
          console.warn("Variable no longer exists. Aborting drop.");
          return;
        }

        let spawnType: "get_variable" | "set_variable" | null = null;
        if (event.altKey) spawnType = "set_variable";
        else if (event.ctrlKey) spawnType = "get_variable";

        if (spawnType) {
          const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, spawnType, {
            variableId: dragState.template.variableId,
            variableName: dragState.template.variableName,
            variableType: dragState.template.variableType,
            variableIsArray: dragState.template.variableIsArray,
          } as any);
          if (newNode) {
            await createNode(newNode.nodeType, { x: newNode.position.x, y: newNode.position.y });
          }
          return;
        }

        if (targetPinId) {
          const newNode = createNodeFromTemplate({ x, y }, currentCanvas.scale, "get_variable", {
            variableId: dragState.template.variableId,
            variableName: dragState.template.variableName,
            variableType: dragState.template.variableType,
            variableIsArray: dragState.template.variableIsArray,
          } as any);
          if (newNode) {
            await createNode(newNode.nodeType, { x: newNode.position.x, y: newNode.position.y });
          }
          return;
        }

        setVariableDropMenu({
          x: dragState.x,
          y: dragState.y,
          worldX: x,
          worldY: y,
          variableId: dragState.template.variableId,
          variableName: dragState.template.variableName,
          variableType: dragState.template.variableType,
          variableIsArray: dragState.template.variableIsArray,
        });
        return;
      }

      if (
        dragState.template.nodeType === "call_function" ||
        dragState.template.nodeType === "call_macro"
      ) {
        const type = dragState.template.nodeType;
        const subId = dragState.template.subGraphId;
        const subData = type === "call_function" ? functions[subId] : macros[subId];
        if (!subData) return;

        const req = buildCreateNodeRequest(type, { x, y }, { subGraphId: subId });
        await createNode(req.nodeType, req.position);
        return;
      }

      const newNode = createNodeFromTemplate(
        { x, y },
        currentCanvas.scale,
        dragState.template.nodeType
      );
      if (newNode) {
            await createNode(newNode.nodeType, { x: newNode.position.x, y: newNode.position.y });
      }
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
    if (prevDragRef.current && !activeDrag) {
      const last = prevDragRef.current;
      if (last.type === "node-template") {
        handleDropTemplate(last, {
          altKey: (window as any)._lastAltKey || false,
          ctrlKey: (window as any)._lastCtrlKey || false,
        } as any);
      }
    }
    prevDragRef.current = activeDrag;
  }, [activeDrag, variables, handleDropTemplate]);

  useEffect(() => {
    canvasDropHandlerStore.setHandler(groupId, handleDropTemplate as any);
    return () => canvasDropHandlerStore.setHandler(groupId, null);
  }, [groupId, handleDropTemplate]);

  return {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleContextMenu,
  };
}
