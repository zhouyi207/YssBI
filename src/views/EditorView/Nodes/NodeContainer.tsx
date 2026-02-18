import React from "react";
import { Node } from "@/shared/types/ui";
import { useNodeExecution } from "@/features/core/node";
import { getNodeClassName, getNodeBackgroundStyle, getNodeMinSize } from "@/features/domain/node/utils";

interface NodeContainerProps {
  node: Node;
  selected?: boolean;
  dragDelta?: { x: number; y: number };
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  children: React.ReactNode;
}

export const NodeContainer = React.memo<NodeContainerProps>(({
  node,
  selected,
  dragDelta,
  onPointerDown,
  children,
}) => {
  const dx = selected && dragDelta ? dragDelta.x : 0;
  const dy = selected && dragDelta ? dragDelta.y : 0;
  const posX = node.position.x + dx;
  const posY = node.position.y + dy;
  const { isExecuting, isCompleted, hasError } = useNodeExecution(node.id);

  const className = getNodeClassName({
    selected,
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
        transition: "border-color 200ms, box-shadow 200ms, background 200ms",
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      onPointerDown={(e) => onPointerDown?.(node.id, e)}
    >
      {children}

      {isExecuting && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-yellow-400 rounded-full animate-ping" />
      )}
      {hasError && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-red-500 rounded-full" />
      )}
      {isCompleted && !isExecuting && (
        <div className="absolute -top-1 -right-1 w-3 h-3 bg-green-500 rounded-full opacity-50" />
      )}
    </div>
  );
});
