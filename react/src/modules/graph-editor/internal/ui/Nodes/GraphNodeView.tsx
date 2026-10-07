import { useGraphFlowInteraction } from "../Canvas/core/GraphFlowContext";
import { memo, type CSSProperties, type MouseEventHandler, type ReactNode } from "react";

export interface GraphNodeViewProps {
  nodeId: string;
  executionState?: string;
  cacheState?: string;
  className: string;
  style: CSSProperties;
  contentSlot: ReactNode;
  executionBadgeSlot?: ReactNode;
  diagnosticBadgeSlot?: ReactNode;
  contextMenuSlot?: ReactNode;
  onContextMenu: MouseEventHandler<HTMLDivElement>;
}

export const GraphNodeView = memo(function GraphNodeView({
  nodeId,
  executionState,
  cacheState,
  className,
  style,
  contentSlot,
  executionBadgeSlot,
  diagnosticBadgeSlot,
  contextMenuSlot,
  onContextMenu,
}: GraphNodeViewProps) {
  const dimmed = useGraphFlowInteraction((state) => state.dimmedNodes.has(nodeId));
  return (
    <div
      id={nodeId}
      data-node-id={nodeId}
      data-graph-state={executionState}
      data-cache-state={cacheState}
      className={className}
      style={dimmed ? { ...style, opacity: 0.35 } : style}
      onContextMenu={onContextMenu}
    >
      {contentSlot}
      {executionBadgeSlot}
      {diagnosticBadgeSlot}
      {contextMenuSlot}
    </div>
  );
});
