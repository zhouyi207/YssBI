import { useCallback } from "react";
import { getGraphById } from "@/features/core/dataStore";
import { getViewport } from "@/features/core/viewport";
import { deserializeGraph } from "@/features/core/dataStore";
import { DEFAULT_VIEWPORT } from "@/app/appConfig/default";
import { useNodeRegistryStore } from "@/features/core/nodeRegister";
import { executeCommand } from "@/features/core/history";
import type { Pin } from "@/shared/types/domain/pin";
import { logger } from '@/utils/appLogger';

export interface PaletteItem {
  nodeType: string;
  overrides?: {
    subGraphId?: string;
    variableId?: string;
    dataframeId?: string;
  };
}

export interface VariableDropMenu {
  worldX: number;
  worldY: number;
  variableId: string;
  variableName: string;
}

export function useCanvasOverlayHandlers({
  canvasRef,
  groupId,
  activeTabId,
  functions,
  variables,
  Variables,
  pendingConnection,
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
  variables: Record<string, any>;
  Variables: Record<string, any>;
  pendingConnection: Pin | null;
  setContextMenu: (menu: { x: number; y: number; visible: boolean } | null) => void;
  setPendingConnection: (pin: any) => void;
  setVariableDropMenu: (menu: VariableDropMenu | null) => void;
  createNode: (nodeType: string, position: { x: number; y: number }, params?: Record<string, unknown>) => Promise<{ nodeId: string; pinIds: string[] } | undefined>;
  setCanvas: (updater: any, targetGraphId?: string) => void;
}) {
  const handleNodePaletteSelect = useCallback(
    async (item: PaletteItem, contextMenu: { x: number; y: number }) => {
      if (!contextMenu || !canvasRef.current) return;

      const internalNodeTypes = [
        "event_on_run",
        "function_entry",
        "function_return",
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

      const rect = canvasRef.current.getBoundingClientRect();
      const currentCanvas = activeTabId ? getViewport(activeTabId) : DEFAULT_VIEWPORT;
      const x = (contextMenu.x - rect.left - currentCanvas.x) / currentCanvas.scale;
      const y = (contextMenu.y - rect.top - currentCanvas.y) / currentCanvas.scale;

      if (item.nodeType === "Functions:Call Function") {
        const subId = item.overrides?.subGraphId;
        if (!subId) { setContextMenu(null); setPendingConnection(null); return; }
        const subData = functions[subId];
        if (!subData) { setContextMenu(null); setPendingConnection(null); return; }
      }

      const sourcePinForConnect = pendingConnection;
      const definition =
        sourcePinForConnect && activeTabId
          ? useNodeRegistryStore.getState().getDefinition(item.nodeType)
          : undefined;

      if (sourcePinForConnect && activeTabId && definition) {
        // 从 pin 拖拽创建：单步乐观完成「建节点 + 自动连线」，节点与连线同时即时出现。
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
      canvasRef,
      groupId,
      activeTabId,
      functions,
      pendingConnection,
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
        logger.graph.warn('Variable no longer exists', 'CanvasOverlay');
        setVariableDropMenu(null);
        return;
      }
      await createNode("Variables:Get Variable", { x: menu.worldX, y: menu.worldY }, {
        variableId: menu.variableId,
      });
      setVariableDropMenu(null);
    },
    [variables, Variables, createNode, setVariableDropMenu]
  );

  const handleVariableDropSet = useCallback(
    async (menu: VariableDropMenu) => {
      const varId = menu.variableId;
      if (!(varId in variables) && !(varId in Variables)) {
        logger.graph.warn('Variable no longer exists', 'CanvasOverlay');
        setVariableDropMenu(null);
        return;
      }
      await createNode("Variables:Set Variable", { x: menu.worldX, y: menu.worldY }, {
        variableId: menu.variableId,
      });
      setVariableDropMenu(null);
    },
    [variables, Variables, createNode, setVariableDropMenu]
  );

  return {
    handleNodePaletteSelect,
    handleVariableDropGet,
    handleVariableDropSet,
  };
}
