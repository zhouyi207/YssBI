import React from "react";
import { Node } from "@/shared/types/ui";
import { useNodeExecution } from "@/features/domain/node/hooks";
import { getNodeClassName, getNodeBackgroundStyle, getNodeMinSize } from "@/features/domain/node/utils";

interface NodeContainerProps {
  node: Node;
  selected?: boolean;
  /** 拖拽时的视觉偏移，仅用于渲染，不写回 store */
  dragDelta?: { x: number; y: number };
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  children: React.ReactNode;
}

/**
 * Node Container Component
 * 
 * 职责：
 * - 提供节点的容器和基础样式
 * - 处理执行状态的视觉反馈
 * - 处理节点的交互事件
 * 
 * 这是一个纯展示组件，业务逻辑在 hooks 中
 */
export const NodeContainer: React.FC<NodeContainerProps> = ({
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
        // 只对特定属性应用过渡，排除 transform 以避免拖动延迟
        transition: "border-color 200ms, box-shadow 200ms, background 200ms",
        // 强制开启硬件加速的抗锯齿，并保持文本清晰
        WebkitFontSmoothing: "antialiased",
        MozOsxFontSmoothing: "grayscale",
      }}
      onPointerDown={(e) => onPointerDown?.(node.id, e)}
    >
      {children}

      {/* 执行状态指示器 */}
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
};
