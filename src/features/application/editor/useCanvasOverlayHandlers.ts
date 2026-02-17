import { useCallback } from "react";
import { getGraphById } from "@/features/core/dataStore";
import { useViewportStore } from "@/features/core/viewport";
import { createNodeFromTemplate, deserializeGraph } from "@/features/core/dataStore";
import { buildCreateNodeRequest } from "@/shared/utils/editor";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";

export interface PaletteItem {
  nodeType: string;
  overrides?: { subGraphId?: string };
}

export interface VariableDropMenu {
  worldX: number;
  worldY: number;
  variableId: string;
  variableName: string;
  variableType: string;
  variableIsArray?: boolean;
}

/**
 * Node palette select and variable drop menu handlers.
 * Extracted from CanvasOverlays.tsx - view should only consume this hook.
 */
export function useCanvasOverlayHandlers({
  canvasRef,
  groupId,
  activeTabId,
  functions,
  macros,
  scale,
  variables,
  Variables,
  setContextMenu,
  setPendingConnection,
  setVariableDropMenu,
  createNode,
  setCanvas,
}: {
  canvasRef: React.RefObject<HTMLDivElement | null>;
  groupId: string;
  activeTabId: string | null;
  functions: Record<string, any>;
  macros: Record<string, any>;
  scale: number;
  variables: Record<string, any>;
  Variables: Record<string, any>;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: any) => void;
  setVariableDropMenu: (menu: VariableDropMenu | null) => void;
  createNode: (nodeType: string, position: { x: number; y: number }) => Promise<void>;
  setCanvas: (updater: any, targetGroupId?: string) => void;
}) {
  const handleNodePaletteSelect = useCallback(
    async (item: PaletteItem, contextMenu: { x: number; y: number }) => {
      if (!contextMenu || !canvasRef.current) return;

      const internalNodeTypes = [
        "event_on_run",
        "function_entry",
        "function_return",
        "macro_inputs",
        "macro_outputs",
      ];

      if (internalNodeTypes.includes(item.nodeType)) {
        const graphData = getGraphById(activeTabId || "");
        const currentNodes = graphData ? deserializeGraph(graphData).nodes : [];
        const existingNode = currentNodes.find(
          (n: any) => n.nodeType === item.nodeType && n.isInternal
        );
        if (existingNode) {
          const rect = canvasRef.current.getBoundingClientRect();
          const centerX = rect.width / 2;
          const centerY = rect.height / 2;
          const currentCanvas =
            useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
          setCanvas({
            ...currentCanvas,
            x: centerX - existingNode.position.x * currentCanvas.scale,
            y: centerY - existingNode.position.y * currentCanvas.scale,
          });
          setContextMenu(null);
          setPendingConnection(null);
          return;
        }
      }

      const rect = canvasRef.current.getBoundingClientRect();
      const currentCanvas =
        useViewportStore.getState().viewports[groupId] || DEFAULT_VIEWPORT;
      const x = (contextMenu.x - rect.left - currentCanvas.x) / currentCanvas.scale;
      const y = (contextMenu.y - rect.top - currentCanvas.y) / currentCanvas.scale;

      let newNode: any = null;

      if (item.nodeType === "call_function" || item.nodeType === "call_macro") {
        const subId = item.overrides?.subGraphId;
        if (subId) {
          const subData = item.nodeType === "call_function" ? functions[subId] : macros[subId];
          if (subData) {
            newNode = buildCreateNodeRequest(item.nodeType, { x, y }, { subGraphId: subId });
          }
        }
      } else {
        newNode = createNodeFromTemplate(
          { x, y },
          currentCanvas.scale,
          item.nodeType,
          item.overrides
        );
      }

      if (newNode) {
        await createNode(newNode.nodeType, newNode.position);
      }
      setContextMenu(null);
      setPendingConnection(null);
    },
    [
      canvasRef,
      groupId,
      activeTabId,
      functions,
      macros,
      createNode,
      setContextMenu,
      setPendingConnection,
      setCanvas,
    ]
  );

  const handleVariableDropGet = useCallback(
    async (menu: VariableDropMenu) => {
      const varId = menu.variableId;
      if (!(varId in variables) && !(varId in Variables)) {
        console.warn("Variable no longer exists.");
        setVariableDropMenu(null);
        return;
      }
      const newNode = createNodeFromTemplate(
        { x: menu.worldX, y: menu.worldY },
        scale,
        "get_variable",
        {
          title: `Get ${menu.variableName}`,
          variableId: menu.variableId,
          variableType: menu.variableType,
          variableName: menu.variableName,
          variableIsArray: menu.variableIsArray,
        } as any
      );
      if (newNode) {
        await createNode(newNode.nodeType, {
          x: newNode.position.x,
          y: newNode.position.y,
        });
      }
      setVariableDropMenu(null);
    },
    [variables, Variables, scale, createNode, setVariableDropMenu]
  );

  const handleVariableDropSet = useCallback(
    async (menu: VariableDropMenu) => {
      const varId = menu.variableId;
      if (!(varId in variables) && !(varId in Variables)) {
        console.warn("Variable no longer exists.");
        setVariableDropMenu(null);
        return;
      }
      const newNode = createNodeFromTemplate(
        { x: menu.worldX, y: menu.worldY },
        scale,
        "set_variable",
        {
          title: `Set ${menu.variableName}`,
          variableId: menu.variableId,
          variableType: menu.variableType,
          variableName: menu.variableName,
          variableIsArray: menu.variableIsArray,
        } as any
      );
      if (newNode) {
        await createNode(newNode.nodeType, {
          x: newNode.position.x,
          y: newNode.position.y,
        });
      }
      setVariableDropMenu(null);
    },
    [variables, Variables, scale, createNode, setVariableDropMenu]
  );

  return {
    handleNodePaletteSelect,
    handleVariableDropGet,
    handleVariableDropSet,
  };
}
