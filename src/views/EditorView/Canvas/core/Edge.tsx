import React from "react";
import { computeEdgePath } from "@/features/core/canvas/edgePath";

export type EdgeKind = "exec" | "data";

/** 流动态：exec 控制流 / data 值沿 output→input 传递（原高亮流动样式） */
const FLOW_EDGE_STYLE: Record<
  EdgeKind,
  {
    stroke: string;
    flowStroke: string;
    glow: string;
    idleGlow: string;
    flowDasharray: string;
    flowAnimation: string;
    glowAnimation: string;
  }
> = {
  data: {
    stroke: "#10b981",
    flowStroke: "#6ee7b7",
    glow: "rgba(16, 185, 129, 0.5)",
    idleGlow: "rgba(16, 185, 129, 0.25)",
    flowDasharray: "14 26",
    flowAnimation: "edgeFlowData 1.2s linear infinite",
    glowAnimation: "edgeGlowData 1.6s ease-in-out infinite",
  },
  exec: {
    stroke: "#f59e0b",
    flowStroke: "#fde68a",
    glow: "rgba(245, 158, 11, 0.55)",
    idleGlow: "rgba(245, 158, 11, 0.3)",
    flowDasharray: "6 10",
    flowAnimation: "edgeFlowExec 0.65s linear infinite",
    glowAnimation: "edgeGlowExec 0.9s ease-in-out infinite",
  },
};

/** 取数态：消费者声明 data 依赖（ConnectionActive），细虚线向 input 侧轻 pulse */
const DATA_PULL_STYLE = {
  stroke: "#2dd4bf",
  flowStroke: "#99f6e4",
  glow: "rgba(45, 212, 191, 0.35)",
  idleGlow: "rgba(45, 212, 191, 0.18)",
  dasharray: "4 10",
  animation: "edgePullData 1.6s linear infinite",
  glowAnimation: "edgePullGlow 2s ease-in-out infinite",
};

interface EdgeProps {
  edgeId?: string;
  fromPinId?: string;
  toPinId?: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color?: string;
  thickness?: number;
  edgeKind?: EdgeKind;
  startIsInput?: boolean;
  /** data 取数依赖已声明 */
  isPullActive?: boolean;
  /** data 值流动 / exec 控制流经过 */
  isFlowActive?: boolean;
  isError?: boolean;
  isRunning?: boolean;
  dimmed?: boolean;
}

export function drawEdge(
  ctx: CanvasRenderingContext2D,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  color: string = "#999",
  thickness: number = 2,
  startIsInput: boolean = false
) {
  const dx = Math.abs(x1 - x2);
  const curvature = Math.max(dx * 0.5, 40);
  const dir = startIsInput ? -1 : 1;

  const c1x = x1 + curvature * dir;
  const c1y = y1;
  const c2x = x2 - curvature * dir;
  const c2y = y2;

  ctx.beginPath();
  ctx.moveTo(x1, y1);
  ctx.bezierCurveTo(c1x, c1y, c2x, c2y, x2, y2);
  ctx.strokeStyle = color;
  ctx.lineWidth = thickness;
  ctx.lineCap = "round";
  ctx.stroke();
}

export const Edge = React.memo<EdgeProps>(({
  edgeId,
  fromPinId: _fromPinId,
  toPinId: _toPinId,
  x1,
  y1,
  x2,
  y2,
  color = "#999",
  thickness = 2,
  edgeKind = "data",
  startIsInput = false,
  isPullActive = false,
  isFlowActive = false,
  isError = false,
  isRunning = false,
  dimmed = false,
}) => {
  const pathData = computeEdgePath(x1, y1, x2, y2, startIsInput);
  const flow = FLOW_EDGE_STYLE[edgeKind];
  const showPull = edgeKind === "data" && isPullActive && !isError;
  const showFlow = isFlowActive && !isError;
  const highlighted = showPull || showFlow;
  const strokeColor = isError
    ? "#ef4444"
    : showFlow
      ? flow.stroke
      : showPull
        ? DATA_PULL_STYLE.stroke
        : color;
  const strokeW = (isError || highlighted) ? thickness + 1 : thickness;
  const animatePullMotion = isRunning && showPull;
  const animateFlow = isRunning && showFlow;

  return (
    <g
      data-edge-id={edgeId}
      style={dimmed ? { opacity: 0.25, transition: "opacity 150ms" } : { transition: "opacity 150ms" }}
    >
      <path
        d={pathData}
        fill="none"
        stroke={strokeColor}
        strokeWidth={strokeW}
        strokeLinecap="round"
        strokeDasharray={showPull && !showFlow && !animatePullMotion ? DATA_PULL_STYLE.dasharray : undefined}
        className="pointer-events-none"
      />

      {showPull && !showFlow && (
        <>
          <path
            d={pathData}
            fill="none"
            stroke={DATA_PULL_STYLE.flowStroke}
            strokeWidth={thickness}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              strokeDasharray: DATA_PULL_STYLE.dasharray,
              animation: animatePullMotion ? DATA_PULL_STYLE.animation : undefined,
            }}
          />
          <path
            d={pathData}
            fill="none"
            stroke={DATA_PULL_STYLE.glow}
            strokeWidth={thickness + 5}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              filter: "blur(4px)",
              animation: DATA_PULL_STYLE.glowAnimation,
            }}
          />
        </>
      )}

      {animateFlow && (
        <>
          <path
            d={pathData}
            fill="none"
            stroke={isError ? "#fca5a5" : flow.flowStroke}
            strokeWidth={edgeKind === "exec" ? thickness + 3 : thickness + 2}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              strokeDasharray: isError ? "6 4" : flow.flowDasharray,
              animation: isError ? "edgeFlowData 1.2s linear infinite" : flow.flowAnimation,
            }}
          />
          <path
            d={pathData}
            fill="none"
            stroke={isError ? "rgba(239, 68, 68, 0.5)" : flow.glow}
            strokeWidth={edgeKind === "exec" ? thickness + 10 : thickness + 8}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              filter: "blur(5px)",
              animation: isError ? "edgeGlowData 1.6s ease-in-out infinite" : flow.glowAnimation,
            }}
          />
        </>
      )}

      {!isRunning && showFlow && !isError && (
        <path
          d={pathData}
          fill="none"
          stroke={flow.idleGlow}
          strokeWidth={edgeKind === "exec" ? thickness + 8 : thickness + 6}
          strokeLinecap="round"
          className="pointer-events-none"
          style={{ filter: "blur(4px)" }}
        />
      )}

      {!animateFlow && isError && (
        <path
          d={pathData}
          fill="none"
          stroke="rgba(239, 68, 68, 0.25)"
          strokeWidth={thickness + 6}
          strokeLinecap="round"
          strokeDasharray="6 4"
          className="pointer-events-none"
          style={{ filter: "blur(4px)" }}
        />
      )}
    </g>
  );
});
