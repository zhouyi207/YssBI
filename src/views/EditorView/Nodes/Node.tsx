import React, { useMemo } from "react";
import { Pin as PinModel } from "@/shared/types/domain";
import type { UINode } from "@/shared/types/ui";
import { NodeContainer } from "./NodeContainer";
import { DefaultNodeLayout } from "./DefaultNodeLayout";
import { MathNodeLayout } from "./MathNodeLayout";
import { RerouteNodeLayout } from './RerouteNodeLayout';
import { uiNodeIsReroute } from '@/features/core/dataStore/nodeView';
import { isPinCompatible } from "@/shared/utils/pinCompatibility";

export interface NodeProps {
  id: string;
  node: UINode;
  selected?: boolean;
  activePinId?: string | null;
  activePin?: PinModel | null;
  graphPath?: string;
  groupId?: string;
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
  const { node, onPointerDown, selected, activePin } = props;

  if (!node) return null;

  const nodeDimmed = useMemo(() => {
    if (!activePin) return false;
    if (activePin.nodeId === node.id) return false;
    const allPins = [...node.inputs, ...node.outputs];
    return !allPins.some(pin => isPinCompatible(pin, activePin));
  }, [activePin, node]);

  return (
    <NodeContainer
      node={node}
      graphPath={props.graphPath}
      groupId={props.groupId}
      selected={selected}
      onPointerDown={onPointerDown}
      dimmed={nodeDimmed}
    >
      {uiNodeIsReroute(node) ? (
        <RerouteNodeLayout {...props} />
      ) : node.uiStyle === "math" ? (
        <MathNodeLayout {...props} />
      ) : (
        <DefaultNodeLayout {...props} />
      )}
    </NodeContainer>
  );
}, (prev, next) => {
  return (
    prev.selected === next.selected &&
    prev.activePinId === next.activePinId &&
    prev.activePin === next.activePin &&
    prev.node === next.node
  );
});
