import {
  memo,
  type CSSProperties,
  type MouseEventHandler,
  type PointerEventHandler,
  type ReactNode,
} from "react";

export interface GraphNodeViewProps {
  nodeId: string;
  className: string;
  style: CSSProperties;
  contentSlot: ReactNode;
  executionBadgeSlot?: ReactNode;
  diagnosticBadgeSlot?: ReactNode;
  contextMenuSlot?: ReactNode;
  onPointerDown?: PointerEventHandler<HTMLDivElement>;
  onContextMenu: MouseEventHandler<HTMLDivElement>;
}

export const GraphNodeView = memo(function GraphNodeView({
  nodeId,
  className,
  style,
  contentSlot,
  executionBadgeSlot,
  diagnosticBadgeSlot,
  contextMenuSlot,
  onPointerDown,
  onContextMenu,
}: GraphNodeViewProps) {
  return (
    <div
      id={nodeId}
      data-node-id={nodeId}
      className={className}
      style={style}
      onPointerDown={onPointerDown}
      onContextMenu={onContextMenu}
    >
      {contentSlot}
      {executionBadgeSlot}
      {diagnosticBadgeSlot}
      {contextMenuSlot}
    </div>
  );
});
