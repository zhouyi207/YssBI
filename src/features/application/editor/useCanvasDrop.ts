import { useState, useEffect, useCallback } from "react";
import { useGestureStore } from "@/features/core/gesture";
import { useGraphDataStore } from "@/features/core/dataStore";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { executeCommand } from "@/features/core/history";
import { useNodeRegistryStore } from "@/features/core/nodeRegister/useNodeRegistryStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import type { Pin } from '@/shared/types/domain/pin';
import type { NodeTemplateDragState } from '@/features/core/dnd';
import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import {
  clientToWorldInCanvas,
  isPointInsideCanvas,
  spawnNodeFromTemplate,
  spawnVariableFromMenu,
  type CreateNodeFn,
  type VariableDropMenu,
  type VariableNodeType,
} from "./canvasDrop";

export type { VariableDropMenu } from "./canvasDrop";

interface UseCanvasDropParams {
  canvasElementRef: React.RefObject<HTMLDivElement | null>;
  groupId: string;
  graphPath: string | null;
  variables: EditorVariables;
  functions: EditorFunctions;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: Pin | null) => void;
  createNode: CreateNodeFn;
  /** When false, skip drop handlers and global dismiss listeners (preview canvases). */
  enabled?: boolean;
}

/**
 * Canvas drop logic: template drop, variable drop menu, add input, click outside, context menu.
 */
export function useCanvasDrop({
  canvasElementRef,
  groupId,
  graphPath,
  variables,
  functions,
  setContextMenu,
  setPendingConnection,
  createNode,
  enabled = true,
}: UseCanvasDropParams) {
  const [variableDropMenu, setVariableDropMenu] = useState<VariableDropMenu | null>(null);

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

      setContextMenu(null);
      setPendingConnection(null);
      if (variableDropMenu) {
        setVariableDropMenu(null);
      }
    };
    return addGlobalEventListener(window, "pointerdown", handleClickOutside, { capture: true });
  }, [enabled, canvasElementRef, setContextMenu, setPendingConnection, variableDropMenu]);

  const handleNodeAddInput = useCallback(
    (nodeId: string) => {
      if (!graphPath) return;
      const nodeData = useGraphDataStore.getState().getGraphNode(graphPath, nodeId);
      const nodeType = nodeData?.nodeType;
      let slotIndex = 0;
      if (nodeType) {
        const def = useNodeRegistryStore.getState().getDefinition(nodeType);
        const idx = def?.pinSlots.findIndex(s => s.slotKind === 'repeatable') ?? -1;
        if (idx >= 0) slotIndex = idx;
      }
      executeCommand(graphPath, 'AddRepeatablePin', { nodeId, slotIndex });
    },
    [graphPath]
  );

  const handleNodeRemovePin = useCallback(
    (nodeId: string, pinId: string) => {
      if (!graphPath) return Promise.resolve();
      return executeCommand(graphPath, 'RemoveRepeatablePin', { nodeId, pinId }).then(() => undefined).catch((err) => {
        uiStore.showToast(err instanceof Error ? err.message : String(err), "error");
        throw err;
      });
    },
    [graphPath]
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
    async (dragState: NodeTemplateDragState, event: MouseEvent | PointerEvent) => {
      const el = canvasElementRef.current;
      if (!el) return;

      if (!isPointInsideCanvas(el, dragState.x, dragState.y)) return;

      const worldPosition = clientToWorldInCanvas(el, groupId, graphPath, dragState.x, dragState.y);

      await spawnNodeFromTemplate(
        dragState.template,
        worldPosition,
        { x: dragState.x, y: dragState.y },
        event,
        {
          variables,
          functions,
          createNode,
          onVariableMenu: setVariableDropMenu,
        },
      );
    },
    [canvasElementRef, groupId, graphPath, variables, functions, createNode],
  );

  useEffect(() => {
    if (!enabled) return;
    canvasDropHandlerStore.setHandler(groupId, (dragState, event) =>
      handleDropTemplate(dragState, event as MouseEvent),
    );
    return () => canvasDropHandlerStore.setHandler(groupId, null);
  }, [enabled, groupId, handleDropTemplate]);

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
