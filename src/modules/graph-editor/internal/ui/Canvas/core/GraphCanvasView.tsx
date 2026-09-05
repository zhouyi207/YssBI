import type { MouseEventHandler, PointerEventHandler, ReactNode, RefObject } from "react";

export interface GraphCanvasViewProps {
  canvasElementRef: RefObject<HTMLDivElement | null>;
  selectionBoxRef: RefObject<HTMLDivElement | null>;
  panelInstanceId: string;
  graphPath?: string;
  graphKind: "event" | "function";
  viewportGridSlot: ReactNode;
  connectionPreviewSlot: ReactNode;
  graphContentSlot: ReactNode;
  overlaySlot?: ReactNode;
  onCanvasPointerDown?: PointerEventHandler<HTMLDivElement>;
  onCanvasContextMenu?: MouseEventHandler<HTMLDivElement>;
}

export function GraphCanvasView({
  canvasElementRef,
  selectionBoxRef,
  panelInstanceId,
  graphPath,
  graphKind,
  viewportGridSlot,
  connectionPreviewSlot,
  graphContentSlot,
  overlaySlot,
  onCanvasPointerDown,
  onCanvasContextMenu,
}: GraphCanvasViewProps) {
  return (
    <div
      ref={canvasElementRef}
      data-editor-panel-instance-id={panelInstanceId}
      tabIndex={-1}
      data-editor-graph-path={graphPath}
      data-editor-graph-kind={graphKind}
      className="relative h-full w-full select-none overflow-hidden bg-[var(--workbench-bg)]"
    >
      {viewportGridSlot}

      <div
        className="absolute inset-0"
        onPointerDown={onCanvasPointerDown}
        onContextMenu={onCanvasContextMenu}
      >
        {connectionPreviewSlot}
        {graphContentSlot}
      </div>

      <div ref={selectionBoxRef} aria-hidden />
      {overlaySlot}
    </div>
  );
}
