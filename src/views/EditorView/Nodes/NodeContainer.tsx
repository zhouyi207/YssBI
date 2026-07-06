import React, { useState, useCallback } from "react";
import { Node } from "@/shared/types/ui";
import { useNodeExecution } from "@/features/core/node";
import { useExecutionStore } from "@/features/core/execution";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { getNodeClassName, getNodeBackgroundStyle, getNodeMinSize } from "@/features/domain/node/utils";
import { useCanvasContextMenuActionsOptional } from "@/features/application/editor/CanvasContextMenuContext";
import { NodeContextMenu } from "../ContextMenu";

interface NodeContainerProps {
  node: Node;
  graphId?: string;
  groupId?: string;
  selected?: boolean;
  dimmed?: boolean;
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  children: React.ReactNode;
}

export const NodeContainer = React.memo<NodeContainerProps>(({
  node,
  graphId: _graphId,
  groupId,
  selected,
  dimmed,
  onPointerDown,
  children,
}) => {
  const posX = node.position.x;
  const posY = node.position.y;
  const graphStatus = useExecutionStore((s) => (_graphId ? s.graphs[_graphId]?.status ?? 'idle' : 'idle'));
  const isReplay = useExecutionStore((s) => !!_graphId && s.isPlaying && s.playbackGraphId === _graphId);
  const useStoreExecVisual = graphStatus !== 'running' && !isReplay;

  const { isCompleted, hasError } = useNodeExecution(node.id, _graphId, useStoreExecVisual);
  const menuActions = useCanvasContextMenuActionsOptional();

  const hasLinks = useGraphDataStore((s) => {
    if (!_graphId) return false;
    const pinIds = s.getGraphNodePins(_graphId, node.id);
    return pinIds.some((pid) => s.getGraphPinConnections(_graphId, pid).length > 0);
  });

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    menuActions?.selectNode(node.id, groupId);
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, [menuActions, node.id, groupId]);

  const className = getNodeClassName({
    selected: selected || !!contextMenu,
    hasError,
    isCompleted,
  });

  const background = getNodeBackgroundStyle({
    hasError,
    isCompleted,
  });

  const minSize = getNodeMinSize(node.noHeader);

  return (
    <div
      id={node.id}
      data-node-id={node.id}
      className={className}
      style={{
        ...minSize,
        transform: `translate3d(${posX}px, ${posY}px, 0)`,
        background,
        opacity: dimmed ? 0.35 : undefined,
        transition: useStoreExecVisual ? "border-color 200ms, box-shadow 200ms, background 200ms, opacity 150ms" : undefined,
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      onPointerDown={(e) => onPointerDown?.(node.id, e)}
      onContextMenu={handleContextMenu}
    >
      {children}

      {hasError && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full" />
      )}
      {isCompleted && (
        <div className="absolute -top-1.5 -right-1.5 w-4 h-4 bg-green-500 rounded-full shadow-lg shadow-green-500/40" />
      )}

      {contextMenu && menuActions && (
        <NodeContextMenu
          position={contextMenu}
          isInternal={node.isInternal}
          hasLinks={hasLinks}
          onCopy={() => menuActions.copyNode(node.id)}
          onCut={() => void menuActions.cutNode(node.id)}
          onDuplicate={() => void menuActions.duplicateNode(node.id)}
          onDelete={() => void menuActions.deleteNode(node.id)}
          onBreakAllLinks={() => void menuActions.breakAllNodeLinks(node.id)}
          onSelectLinked={() => menuActions.selectLinkedNodes(node.id)}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
});
