import { memo, useMemo, type RefCallback } from "react";
import {
  Position,
  ViewportPortal,
  useInternalNode,
  useReactFlow,
  type ConnectionLineComponentProps,
} from "@xyflow/react";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { computeEdgePath } from "@/features/core/canvas";
import { Edge } from "./Edge";
import { useGraphFlowInteraction } from "./GraphFlowContext";
import { getFlowPinFeedback } from "./graphFlowModel";
import { useEdgePath } from "./useEdgePath";

export function GraphFlowConnection({
  fromX,
  fromY,
  toX,
  toY,
  fromPosition,
  toHandle,
}: ConnectionLineComponentProps) {
  const pathRef = useEdgePath(
    computeEdgePath(fromX, fromY, toX, toY, fromPosition === Position.Left),
  );
  const targetId = toHandle?.id ?? null;
  return useMemo(
    () => <ConnectionAppearance pathRef={pathRef} targetId={targetId} />,
    [pathRef, targetId],
  );
}

const ConnectionAppearance = memo(function ConnectionAppearance({
  pathRef,
  targetId,
}: {
  pathRef: RefCallback<SVGPathElement>;
  targetId: string | null;
}) {
  const feedback = useGraphFlowInteraction((state) =>
    targetId ? getFlowPinFeedback(state, targetId) : null,
  );
  const color =
    feedback?.kind === "invalid"
      ? "var(--status-danger)"
      : feedback?.kind === "replace"
        ? "var(--status-warning)"
        : feedback?.kind === "append"
          ? "var(--status-success)"
          : "var(--accent-color)";
  return (
    <g
      pointerEvents="none"
      data-connection-feedback={feedback?.kind}
      data-connection-invalid-reason={feedback?.kind === "invalid" ? feedback.reason : undefined}
    >
      <Edge pathRef={pathRef} color={color} />
    </g>
  );
});

export function PendingFlowConnection({
  pin,
  menu,
}: {
  pin: Pick<PinData, "id" | "nodeId" | "direction">;
  menu: { x: number; y: number };
}) {
  const node = useInternalNode(pin.nodeId);
  const { screenToFlowPosition } = useReactFlow();
  const handle = node?.internals.handleBounds?.source?.find((item) => item.id === pin.id);
  if (!node || !handle) return null;
  const start = {
    x: node.internals.positionAbsolute.x + handle.x + handle.width / 2,
    y: node.internals.positionAbsolute.y + handle.y + handle.height / 2,
  };
  const end = screenToFlowPosition(menu);
  return (
    <ViewportPortal>
      <svg
        className="absolute pointer-events-none overflow-visible"
        style={{ left: 0, top: 0, width: 1, height: 1 }}
      >
        <Edge
          x1={start.x}
          y1={start.y}
          x2={end.x}
          y2={end.y}
          startIsInput={pin.direction === "input"}
          color="var(--accent-color)"
        />
      </svg>
    </ViewportPortal>
  );
}
