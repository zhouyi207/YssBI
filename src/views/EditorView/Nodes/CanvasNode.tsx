import React, { useCallback } from "react";
import type { Pin as PinModel } from "@/shared/types/domain";
import { useNodeView } from "@/features/core/dataStore/useNodeView";
import { Node } from "./Node";

export interface CanvasNodeProps {
  id: string;
  graphPath?: string;
  groupId?: string;
  selected?: boolean;
  activePin?: PinModel | null;
  onPointerDown?: (nodeId: string, e: React.PointerEvent) => void;
  onAddInput?: (id: string) => void;
  onRemovePin?: (nodeId: string, pinId: string) => void;
  onPinClick?: (pinId: string, direction: "input" | "output") => void;
  /** 注意：与上游 `onPinPointerDown(pin, e)` 同序，内部再适配为 Node 的 (e, pin) */
  onPinPointerDown?: (pin: PinModel, e: React.PointerEvent) => void;
  onPinValueChange?: (pinId: string, value: unknown) => void;
}

/**
 * CanvasNode - 画布节点容器
 *
 * 仅通过 `useNodeView(id, graphPath)` 订阅该节点自身的 store 切片，再渲染纯展示组件 `Node`。
 * 配合稳定的交互回调与 `React.memo`，一次图变更只会让受影响的节点重渲染，
 * 而不会牵动整张画布。
 */
export const CanvasNode = React.memo(function CanvasNode(props: CanvasNodeProps) {
  const {
    id,
    graphPath,
    groupId,
    selected,
    activePin,
    onPointerDown,
    onAddInput,
    onRemovePin,
    onPinClick,
    onPinPointerDown,
    onPinValueChange,
  } = props;

  const node = useNodeView(id, graphPath);

  const handlePinPointerDown = useCallback(
    (e: React.PointerEvent, pin: PinModel) => {
      onPinPointerDown?.(pin, e);
    },
    [onPinPointerDown],
  );

  if (!node) return null;

  return (
    <Node
      id={id}
      node={node}
      selected={selected}
      activePinId={activePin?.id}
      activePin={activePin}
      graphPath={graphPath}
      groupId={groupId}
      onPointerDown={onPointerDown}
      onAddInput={onAddInput}
      onRemovePin={onRemovePin}
      onPinClick={onPinClick}
      onPinPointerDown={handlePinPointerDown}
      onPinValueChange={onPinValueChange}
    />
  );
});
