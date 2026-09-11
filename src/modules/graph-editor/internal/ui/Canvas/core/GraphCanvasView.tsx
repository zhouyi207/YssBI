import type { ReactNode, RefObject } from "react";

export interface GraphCanvasViewProps {
  canvasElementRef: RefObject<HTMLDivElement | null>;
  panelInstanceId: string;
  graphPath?: string;
  graphKind: "event" | "function";
  viewportGridSlot: ReactNode;
  graphContentSlot: ReactNode;
  overlaySlot?: ReactNode;
}

export function GraphCanvasView({
  canvasElementRef,
  panelInstanceId,
  graphPath,
  graphKind,
  viewportGridSlot,
  graphContentSlot,
  overlaySlot,
}: GraphCanvasViewProps) {
  return (
    <div
      ref={canvasElementRef}
      data-editor-panel-instance-id={panelInstanceId}
      tabIndex={-1}
      data-editor-graph-path={graphPath}
      data-editor-graph-kind={graphKind}
      className="relative h-full w-full select-none overflow-hidden bg-[var(--workbench-bg)] outline-none"
    >
      {viewportGridSlot}
      <div className="absolute inset-0">{graphContentSlot}</div>
      {overlaySlot}
    </div>
  );
}
