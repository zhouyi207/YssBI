import React, { useState, useCallback } from "react";
import { Node } from "@/shared/types/ui";
import { useNodeExecution } from "@/features/core/node";
import { getNodeClassName, getNodeBackgroundStyle, getNodeMinSize } from "@/features/domain/node/utils";
import { NodeContextMenu } from "../ContextMenu";

interface NodeContainerProps {
  node: Node;
  graphId?: string;
  selected?: boolean;
  dragDelta?: { x: number; y: number };
  dimmed?: boolean;
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  children: React.ReactNode;
}

export const NodeContainer = React.memo<NodeContainerProps>(({
  node,
  graphId,
  selected,
  dragDelta,
  dimmed,
  onPointerDown,
  children,
}) => {
  const dx = dragDelta ? dragDelta.x : 0;
  const dy = dragDelta ? dragDelta.y : 0;
  const posX = node.position.x + dx;
  const posY = node.position.y + dy;
  const { isExecuting, isCompleted, hasError } = useNodeExecution(node.id, graphId);

  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY });
  }, []);

  const className = getNodeClassName({
    selected: selected || !!contextMenu,
    isExecuting,
    hasError,
    isCompleted,
  });

  const background = getNodeBackgroundStyle({
    isExecuting,
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
        transition: "border-color 200ms, box-shadow 200ms, background 200ms, opacity 150ms",
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      onPointerDown={(e) => onPointerDown?.(node.id, e)}
      onContextMenu={handleContextMenu}
    >
      {children}

      {isExecuting && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-yellow-400 rounded-full animate-ping" />
      )}
      {hasError && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full" />
      )}
      {isCompleted && !isExecuting && (
        <div className="absolute -top-1.5 -right-1.5 w-4 h-4 bg-green-500 rounded-full shadow-lg shadow-green-500/40" />
      )}

      {contextMenu && (
        <NodeContextMenu
          position={contextMenu}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
});
