import {
  ViewportPortal,
  useInternalNode,
  useReactFlow,
  type ConnectionLineComponentProps,
} from "@xyflow/react";
import type { PinData } from "@/features/domain/editorProjection/graphRuntimeTypes";
import { computeEdgePath } from "@/features/core/canvas/edgePath";
import { useGraphFlowContext } from "./GraphFlowContext";

export function GraphFlowConnection({
  fromX,
  fromY,
  toX,
  toY,
  toHandle,
}: ConnectionLineComponentProps) {
  const { feedbackForPin, sourcePin } = useGraphFlowContext();
  const feedback = toHandle?.id ? feedbackForPin(toHandle.id) : null;
  const color =
    feedback?.kind === "invalid"
      ? "var(--status-danger)"
      : feedback?.kind === "replace"
        ? "var(--status-warning)"
        : feedback?.kind === "append"
          ? "var(--status-success)"
          : "var(--accent-color)";
  return (
    <path
      d={computeEdgePath(fromX, fromY, toX, toY, sourcePin?.direction === "input")}
      fill="none"
      stroke={color}
      strokeWidth={2}
      pointerEvents="none"
      data-connection-feedback={feedback?.kind}
      data-connection-invalid-reason={feedback?.kind === "invalid" ? feedback.reason : undefined}
    />
  );
}

export function PendingFlowConnection({
  pin,
  menu,
}: {
  pin: PinData;
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
        <path
          d={computeEdgePath(start.x, start.y, end.x, end.y, pin.direction === "input")}
          fill="none"
          stroke="var(--accent-color)"
          strokeWidth={2}
        />
      </svg>
    </ViewportPortal>
  );
}
