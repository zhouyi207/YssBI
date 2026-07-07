import { useCallback } from "react";
import { findInternalNodeInGraph } from "@/features/core/dataStore";
import { getViewport } from "@/features/core/viewport";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";
import { useNodeRegistryStore } from "@/features/core/nodeRegister";
import { executeCommand } from "@/features/core/history";
import { CALL_FUNCTION_NODE_TYPE } from "@/features/domain/nodeDefinition";
import type { NodeCatalogItem } from "@/features/domain/nodeCatalog";
import type { Pin } from "@/shared/types/domain/pin";
import { logger } from '@/utils/appLogger';
import type { CreateNodeFn } from "./canvasDrop";

export function useCanvasOverlayHandlers({
  canvasElementRef,
  activeTabId,
  functions,
  pendingConnection,
  setContextMenu,
  setPendingConnection,
  createNode,
  setCanvas,
}: {
  canvasElementRef: React.RefObject<HTMLDivElement | null>;
  activeTabId: string | null;
  functions: Record<string, unknown>;
  pendingConnection: Pin | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: unknown) => void;
  createNode: CreateNodeFn;
  setCanvas: (updater: unknown, targetGraphId?: string) => void;
}) {
  const handleNodePaletteSelect = useCallback(
    async (item: NodeCatalogItem, contextMenu: { x: number; y: number }) => {
      if (!contextMenu || !canvasElementRef.current) return;

      const internalNodeTypes = [
        "event_on_run",
        "function_entry",
        "function_return",
      ];

      if (internalNodeTypes.includes(item.nodeType)) {
        const existingNode = activeTabId
          ? findInternalNodeInGraph(activeTabId, item.nodeType)
          : undefined;
        if (existingNode) {
          const rect = canvasElementRef.current.getBoundingClientRect();
          const centerX = rect.width / 2;
          const centerY = rect.height / 2;
          const currentCanvas = activeTabId ? getViewport(activeTabId) : DEFAULT_VIEWPORT;
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

      const rect = canvasElementRef.current.getBoundingClientRect();
      const currentCanvas = activeTabId ? getViewport(activeTabId) : DEFAULT_VIEWPORT;
      const x = (contextMenu.x - rect.left - currentCanvas.x) / currentCanvas.scale;
      const y = (contextMenu.y - rect.top - currentCanvas.y) / currentCanvas.scale;

      if (item.nodeType === CALL_FUNCTION_NODE_TYPE) {
        const subId = item.overrides?.subGraphId;
        if (!subId || !functions[subId]) {
          setContextMenu(null);
          setPendingConnection(null);
          return;
        }
      }

      const sourcePinForConnect = pendingConnection;
      const definition =
        sourcePinForConnect && activeTabId
          ? useNodeRegistryStore.getState().getDefinition(item.nodeType)
          : undefined;

      if (sourcePinForConnect && activeTabId && definition) {
        try {
          await executeCommand(activeTabId, 'CreateNodeWithConnection', {
            nodeType: item.nodeType,
            x,
            y,
            params: item.overrides ?? undefined,
            sourcePin: sourcePinForConnect,
          });
        } catch (err) {
          logger.graph.warn(`Failed to create node with connection: ${err instanceof Error ? err.message : String(err)}`, 'CanvasOverlay');
        }
      } else {
        await createNode(item.nodeType, { x, y }, item.overrides ?? undefined);
      }

      setContextMenu(null);
      setPendingConnection(null);
    },
    [
      canvasElementRef,
      activeTabId,
      functions,
      pendingConnection,
      createNode,
      setContextMenu,
      setPendingConnection,
      setCanvas,
    ]
  );

  return {
    handleNodePaletteSelect,
  };
}
