import React from "react";
import { Pin as PinModel } from "@/shared/types/domain";
import { Node as NodeModel } from "@/shared/types/ui";
import { NodeContainer } from "./NodeContainer";
import { DefaultNodeLayout } from "./DefaultNodeLayout";
import { MathNodeLayout } from "./MathNodeLayout";

export interface NodeProps {
  id: string;
  node: NodeModel;
  scale: number;
  selected?: boolean;
  dragDelta?: { x: number; y: number };
  activePinId?: string | null;
  subgraphId?: string;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  onPinPointerDown?: (e: React.PointerEvent, pin: PinModel) => void;
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
}

/**
 * Node Component (Refactored)
 * 
 * 职责：
 * - 组合 NodeContainer 和不同的 Layout 组件
 * - 根据 uiStyle 选择合适的布局
 * - 纯展示组件，业务逻辑在 features 中
 * 
 * 重构说明：
 * - 拆分为 NodeContainer（容器）、DefaultNodeLayout、MathNodeLayout
 * - 执行状态逻辑移到 useNodeExecution hook
 * - 样式逻辑移到 useNodeStyle hook 和 utils
 * - 提高可测试性和可维护性
 */
export const Node = React.memo<NodeProps>((props) => {
  const { node, onPointerDown, selected, dragDelta } = props;

  if (!node) return null;

  return (
    <NodeContainer
      node={node}
      graphId={props.subgraphId}
      selected={selected}
      dragDelta={dragDelta}
      onPointerDown={onPointerDown}
    >
      {node.uiStyle === "math" ? (
        <MathNodeLayout {...props} />
      ) : (
        <DefaultNodeLayout {...props} />
      )}
    </NodeContainer>
  );
}, (prev, next) => {
  const dragDeltaSame =
    (prev.dragDelta?.x === next.dragDelta?.x && prev.dragDelta?.y === next.dragDelta?.y) ||
    (!prev.dragDelta && !next.dragDelta);
  return (
    prev.selected === next.selected &&
    prev.activePinId === next.activePinId &&
    prev.node === next.node &&
    prev.scale === next.scale &&
    dragDeltaSame
  );
});
