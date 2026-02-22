import React from "react";

interface EdgeProps {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color?: string;
  thickness?: number;
  /** 起点是否为输入针脚 (默认为 false，即起点为输出) */
  startIsInput?: boolean;
  /** 是否已完成（绿色高亮） */
  isCompleted?: boolean;
  /** 是否为错误下游（红色虚线 + 发光） */
  isError?: boolean;
  /** 执行仍在进行中（启用流动动画） */
  isRunning?: boolean;
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
  x1,
  y1,
  x2,
  y2,
  color = "#999",
  thickness = 2,
  startIsInput = false,
  isCompleted = false,
  isError = false,
  isRunning = false,
}) => {
  const dx = Math.abs(x1 - x2);
  const curvature = Math.max(dx * 0.5, 40);

  const dir = startIsInput ? -1 : 1;
  
  const c1x = x1 + curvature * dir;
  const c1y = y1;
  const c2x = x2 - curvature * dir;
  const c2y = y2;
  
  const pathData = `M ${x1},${y1} C ${c1x},${c1y} ${c2x},${c2y} ${x2},${y2}`;

  const strokeColor = isError ? "#ef4444" : isCompleted ? "#10b981" : color;
  const strokeW = (isError || isCompleted) ? thickness + 1 : thickness;
  const animate = isRunning && (isCompleted || isError);

  return (
    <g>
      <path
        d={pathData}
        fill="none"
        stroke={strokeColor}
        strokeWidth={strokeW}
        strokeLinecap="round"
        strokeDasharray={isError ? "6 4" : undefined}
        className="pointer-events-none"
      />

      {animate && (
        <>
          {/* 流动虚线 */}
          <path
            d={pathData}
            fill="none"
            stroke={isError ? "#fca5a5" : "#6ee7b7"}
            strokeWidth={thickness + 2}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              strokeDasharray: "14 26",
              animation: "edgeFlow 1.2s linear infinite",
            }}
          />
          {/* 脉动发光 */}
          <path
            d={pathData}
            fill="none"
            stroke={isError ? "rgba(239, 68, 68, 0.5)" : "rgba(16, 185, 129, 0.5)"}
            strokeWidth={thickness + 8}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              filter: "blur(5px)",
              animation: "edgeGlow 1.6s ease-in-out infinite",
            }}
          />
        </>
      )}

      {!animate && isCompleted && !isError && (
        <path
          d={pathData}
          fill="none"
          stroke="rgba(16, 185, 129, 0.25)"
          strokeWidth={thickness + 6}
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
