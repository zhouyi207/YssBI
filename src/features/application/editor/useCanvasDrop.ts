import { useState, useEffect, useCallback } from "react";
import { useGestureStore } from "@/features/core/gesture";
import { useGraphDataStore } from "@/features/core/dataStore";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import { executeCommand } from "@/features/core/history";
import { uiStore } from "@/features/core/ui/UIStore";
import { notifyNodeCreationUnavailable } from './editorMutationAvailability';
import { addGlobalEventListener } from "@/shared/utils/globalEvent";
import type { Pin } from '@/shared/types/domain/pin';
import {
  isGraphResourceDragState,
  isNodeTemplateDragState,
  type SidebarDragState,
} from '@/features/core/dnd';
import type { EditorFunctions, EditorVariables } from '@/features/core/editor';
import type {
  CreateNodeFn,
  VariableDropMenu,
  VariableNodeType,
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
  setContextMenu,
  setPendingConnection,
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
      const store = useGraphDataStore.getState();
      const template = store.getGraphNodePins(graphPath, nodeId)
        .map((pinId) => store.getGraphPin(graphPath, pinId))
        .find((pin) => pin?.instanceKind === 'userCreated' && pin.templateKey)
        ?.templateKey;
      if (!template) {
        uiStore.showToast('Repeatable port template is unavailable', 'error');
        return;
      }
      void executeCommand(graphPath, 'AddRepeatablePin', { nodeId, template }).then((applied) => {
        if (!applied) uiStore.showToast('Failed to add repeatable port', 'error');
      });
    },
    [graphPath]
  );

  const handleNodeRemovePin = useCallback(
    (nodeId: string, pinId: string) => {
      if (!graphPath) return Promise.resolve();
      return executeCommand(graphPath, 'RemoveRepeatablePin', { nodeId, pinId }).then((applied) => {
        if (!applied) uiStore.showToast('Failed to remove repeatable port', 'error');
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
    async (_menu: VariableDropMenu, _nodeType: VariableNodeType) => {
      notifyNodeCreationUnavailable();
      setVariableDropMenu(null);
    },
    [],
  );

  const handleVariableDropGet = useCallback(
    (menu: VariableDropMenu) => spawnFromVariableMenu(menu, "Variables:Get Variable"),
    [spawnFromVariableMenu],
  );

  const handleVariableDropSet = useCallback(
    (menu: VariableDropMenu) => spawnFromVariableMenu(menu, "Variables:Set Variable"),
    [spawnFromVariableMenu],
  );

  const handleSidebarCanvasDrop = useCallback(
    async (dragState: SidebarDragState, _event: Pick<MouseEvent | PointerEvent, 'altKey' | 'ctrlKey' | 'shiftKey'>) => {
      if (!canvasElementRef.current) return false;
      if (!isGraphResourceDragState(dragState) && !isNodeTemplateDragState(dragState)) return false;
      notifyNodeCreationUnavailable();
      return false;
    },
    [canvasElementRef],
  );

  useEffect(() => {
    if (!enabled) return;
    canvasDropHandlerStore.setHandler(groupId, (dragState, event) =>
      handleSidebarCanvasDrop(dragState, event as MouseEvent),
    );
    return () => canvasDropHandlerStore.setHandler(groupId, null);
  }, [enabled, groupId, handleSidebarCanvasDrop]);

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
