import { useCallback } from "react";
import { findInternalNodeInGraph } from "@/features/core/dataStore";
import { getViewport, editorViewportScope, type EditorViewport } from "@/features/core/viewport";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";
import type { NodeCatalogItem } from "@/features/domain/nodeCatalog";
import type { Pin } from "@/shared/types/domain/pin";
import type { EditorFunctions } from "@/features/core/editor";
import type { CreateNodeFn } from "./canvasDrop";
import { notifyNodeCreationUnavailable } from './editorMutationAvailability';

export function useCanvasOverlayHandlers({
  canvasElementRef,
  groupId,
  activeTabId,
  setContextMenu,
  setPendingConnection,
  setCanvas,
}: {
  canvasElementRef: React.RefObject<HTMLDivElement | null>;
  groupId: string;
  activeTabId: string | null;
  functions: EditorFunctions;
  pendingConnection: Pin | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: Pin | null) => void;
  createNode: CreateNodeFn;
  setCanvas: (
    updater: EditorViewport | ((prev: EditorViewport) => EditorViewport),
    targetGraphPath?: string,
  ) => void;
}) {
  const handleNodePaletteSelect = useCallback(
    async (item: NodeCatalogItem, contextMenu: { x: number; y: number }) => {
      if (!contextMenu || !canvasElementRef.current) return;

      const internalNodeTypes = [
        "event_on_run",
        "function_entry",
        "function_return",
      ];

      const scope = activeTabId ? editorViewportScope(groupId, activeTabId) : null;
      const currentCanvas = scope ? getViewport(scope) : DEFAULT_VIEWPORT;

      if (internalNodeTypes.includes(item.nodeType)) {
        const existingNode = activeTabId
          ? findInternalNodeInGraph(activeTabId, item.nodeType)
          : undefined;
        if (existingNode) {
          const rect = canvasElementRef.current.getBoundingClientRect();
          const centerX = rect.width / 2;
          const centerY = rect.height / 2;
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

      notifyNodeCreationUnavailable();
      setContextMenu(null);
      setPendingConnection(null);
    },
    [
      canvasElementRef,
      groupId,
      activeTabId,
      setContextMenu,
      setPendingConnection,
      setCanvas,
    ]
  );

  return {
    handleNodePaletteSelect,
  };
}
