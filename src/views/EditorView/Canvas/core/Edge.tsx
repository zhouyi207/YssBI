import React from "react";
import { computeEdgePath } from "@/features/core/canvas/edgePath";

export type EdgeKind = "exec" | "data";

/** 执行中高亮/流动样式：exec 控制流 vs data 数据流 */
const ACTIVE_EDGE_STYLE: Record<
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
  /** exec 控制流连线 vs data 数据流连线（执行动画/高亮区分） */
  edgeKind?: EdgeKind;
  /** 起点是否为输入针脚 (默认为 false，即起点为输出) */
  startIsInput?: boolean;
  /** 是否已完成（绿色高亮） */
  isCompleted?: boolean;
  /** 是否为错误下游（红色虚线 + 发光） */
  isError?: boolean;
  /** 执行仍在进行中（启用流动动画） */
  isRunning?: boolean;
  /** 拖拽连接时整体半透明 */
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
  fromPinId,
  toPinId,
  x1,
  y1,
  x2,
  y2,
  color = "#999",
  thickness = 2,
  edgeKind = "data",
  startIsInput = false,
  isCompleted = false,
  isError = false,
  isRunning = false,
  dimmed = false,
}) => {
  const pathData = computeEdgePath(x1, y1, x2, y2, startIsInput);
  const active = ACTIVE_EDGE_STYLE[edgeKind];
  const strokeColor = isError ? "#ef4444" : isCompleted ? active.stroke : color;
  const strokeW = (isError || isCompleted) ? thickness + 1 : thickness;
  const animate = isRunning && (isCompleted || isError);

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
        strokeDasharray={isError ? "6 4" : isCompleted && edgeKind === "exec" && !animate ? "5 4" : undefined}
        className="pointer-events-none"
      />

      {animate && (
        <>
          {/* 流动虚线：data 长段缓流，exec 短段快流 */}
          <path
            d={pathData}
            fill="none"
            stroke={isError ? "#fca5a5" : active.flowStroke}
            strokeWidth={edgeKind === "exec" ? thickness + 3 : thickness + 2}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              strokeDasharray: isError ? "6 4" : active.flowDasharray,
              animation: isError ? "edgeFlowData 1.2s linear infinite" : active.flowAnimation,
            }}
          />
          {/* 脉动发光 */}
          <path
            d={pathData}
            fill="none"
            stroke={isError ? "rgba(239, 68, 68, 0.5)" : active.glow}
            strokeWidth={edgeKind === "exec" ? thickness + 10 : thickness + 8}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              filter: "blur(5px)",
              animation: isError ? "edgeGlowData 1.6s ease-in-out infinite" : active.glowAnimation,
            }}
          />
        </>
      )}

      {!animate && isCompleted && !isError && (
        <path
          d={pathData}
          fill="none"
          stroke={active.idleGlow}
          strokeWidth={edgeKind === "exec" ? thickness + 8 : thickness + 6}
          strokeLinecap="round"
          className="pointer-events-none"
          style={{ filter: "blur(4px)" }}
        />
      )}

      {!animate && isError && (
        <path
          d={pathData}
          fill="none"
          stroke="rgba(239, 68, 68, 0.25)"
          strokeWidth={thickness + 6}
          strokeLinecap="round"
          className="pointer-events-none"
          style={{ filter: "blur(4px)" }}
        />
      )}
    </g>
  );
});
