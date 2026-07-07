import { useState, useEffect, useCallback } from "react";
import { uiStore } from "@/features/core/ui/UIStore";
import { useGestureStore } from "@/features/core/gesture";
import { useGraphDataStore } from "@/features/core/dataStore";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { executeCommand } from "@/features/core/history";
import { CALL_FUNCTION_NODE_TYPE } from "@/features/domain/nodeDefinition";
import { useNodeRegistryStore } from "@/features/core/nodeRegister/useNodeRegistryStore";
import { logger } from '@/utils/appLogger';
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import {
  buildVariableDropMenu,
  clientToWorldInCanvas,
  isPointInsideCanvas,
  isVariableAvailable,
  resolveVariableSpawnType,
  spawnVariableFromMenu,
  spawnVariableNode,
  type CreateNodeFn,
  type VariableDropMenu,
  type VariableNodeType,
} from "./canvasDrop";

export type { VariableDropMenu } from "./canvasDrop";

interface UseCanvasDropParams {
  canvasElementRef: React.RefObject<HTMLDivElement | null>;
  groupId: string;
  graphId: string | null;
  variables: Record<string, unknown>;
  functions: Record<string, unknown>;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: unknown) => void;
  createNode: CreateNodeFn;
}

/**
 * Canvas drop logic: template drop, variable drop menu, add input, click outside, context menu.
 */
export function useCanvasDrop({
  canvasElementRef,
  groupId,
  graphId,
  variables,
  functions,
  setContextMenu,
  setPendingConnection,
  createNode,
}: UseCanvasDropParams) {
  const [variableDropMenu, setVariableDropMenu] = useState<VariableDropMenu | null>(null);

  useEffect(() => {
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

      setContextMenu(null);
      setPendingConnection(null);
      if (variableDropMenu) {
        setVariableDropMenu(null);
      }
    };
    return addGlobalEventListener(window, "pointerdown", handleClickOutside, { capture: true });
  }, [canvasElementRef, setContextMenu, setPendingConnection, variableDropMenu]);

  const handleNodeAddInput = useCallback(
    (nodeId: string) => {
      if (!graphId) return;
      const nodeData = useGraphDataStore.getState().getGraphNode(graphId, nodeId);
      const nodeType = nodeData?.nodeType;
      let slotIndex = 0;
      if (nodeType) {
        const def = useNodeRegistryStore.getState().getDefinition(nodeType);
        const idx = def?.pinSlots.findIndex(s => s.slotKind === 'repeatable') ?? -1;
        if (idx >= 0) slotIndex = idx;
      }
      executeCommand(graphId, 'AddRepeatablePin', { nodeId, slotIndex });
    },
    [graphId]
  );

  const handleNodeRemovePin = useCallback(
    (nodeId: string, pinId: string) => {
      if (!graphId) return Promise.resolve();
      return executeCommand(graphId, 'RemoveRepeatablePin', { nodeId, pinId }).catch((err) => {
        uiStore.showToast(err instanceof Error ? err.message : String(err), "error");
        throw err;
      });
    },
    [graphId]
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
    [setContextMenu]
  );

  const spawnFromVariableMenu = useCallback(
    async (menu: VariableDropMenu, nodeType: VariableNodeType) => {
      await spawnVariableFromMenu(menu, nodeType, variables, createNode);
      setVariableDropMenu(null);
    },
    [variables, createNode],
  );

  const handleVariableDropGet = useCallback(
    (menu: VariableDropMenu) => spawnFromVariableMenu(menu, "Variables:Get Variable"),
    [spawnFromVariableMenu],
  );

  const handleVariableDropSet = useCallback(
    (menu: VariableDropMenu) => spawnFromVariableMenu(menu, "Variables:Set Variable"),
    [spawnFromVariableMenu],
  );

  const handleDropTemplate = useCallback(
    async (dragState: { x: number; y: number; template: Record<string, unknown> }, event: MouseEvent | PointerEvent) => {
      const el = canvasElementRef.current;
      if (!el) return;

      if (!isPointInsideCanvas(el, dragState.x, dragState.y)) return;

      const { x, y } = clientToWorldInCanvas(el, graphId, dragState.x, dragState.y);
      const template = dragState.template;

      if (template.category === "Data") {
        await createNode(String(template.nodeType), { x, y }, {
          dataframeId: template.variableId,
          variableName: template.variableName,
        });
        return;
      }

      if (template.category === "Variable") {
        const variableId = String(template.variableId);
        if (!isVariableAvailable(variableId, variables)) {
          logger.graph.warn('Variable no longer exists. Aborting drop', 'CanvasDrop');
          return;
        }

        const spawnType = resolveVariableSpawnType(event, dragState.x, dragState.y);
        if (spawnType === 'menu') {
          setVariableDropMenu(buildVariableDropMenu(
            dragState.x,
            dragState.y,
            { x, y },
            variableId,
            String(template.variableName),
          ));
          return;
        }

        await spawnVariableNode(spawnType, { x, y }, variableId, createNode);
        return;
      }

      if (template.nodeType === CALL_FUNCTION_NODE_TYPE) {
        const subId = String(template.subGraphId);
        if (!functions[subId]) return;
        await createNode(CALL_FUNCTION_NODE_TYPE, { x, y }, { subGraphId: subId });
        return;
      }

      await createNode(String(template.nodeType), { x, y });
    },
    [canvasElementRef, graphId, variables, functions, createNode],
  );

  useEffect(() => {
    canvasDropHandlerStore.setHandler(groupId, handleDropTemplate as Parameters<typeof canvasDropHandlerStore.setHandler>[1]);
    return () => canvasDropHandlerStore.setHandler(groupId, null);
  }, [groupId, handleDropTemplate]);

  return {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleNodeRemovePin,
    handleContextMenu,
    handleVariableDropGet,
    handleVariableDropSet,
  };
}
