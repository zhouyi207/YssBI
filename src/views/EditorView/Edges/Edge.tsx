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
  /** 🆕 是否正在执行（显示流动动画） */
  isActive?: boolean;
  /** 🆕 连接的唯一标识符 */
  connectionId?: string;
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
  isActive = false,
  connectionId,
}) => {
  const dx = Math.abs(x1 - x2);
  const curvature = Math.max(dx * 0.5, 40);

  const dir = startIsInput ? -1 : 1;
  
  const c1x = x1 + curvature * dir;
  const c1y = y1;
  const c2x = x2 - curvature * dir;
  const c2y = y2;
  
  const pathData = `M ${x1},${y1} C ${c1x},${c1y} ${c2x},${c2y} ${x2},${y2}`;

  return (
    <g>
      {/* 基础连接线 */}
      <path
        d={pathData}
        fill="none"
        stroke={isActive ? "#facc15" : color}
        strokeWidth={isActive ? thickness + 1 : thickness}
        strokeLinecap="round"
        className="pointer-events-none transition-all duration-200"
      />
      
      {/* 🆕 流动动画 */}
      {isActive && (
        <>
          <path
            d={pathData}
            fill="none"
            stroke="rgba(250, 204, 21, 0.6)"
            strokeWidth={thickness + 2}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              strokeDasharray: "10 10",
              strokeDashoffset: "0",
              animation: "dash 0.5s linear infinite",
            }}
          />
          {/* 添加发光效果 */}
          <path
            d={pathData}
            fill="none"
            stroke="rgba(250, 204, 21, 0.3)"
            strokeWidth={thickness + 6}
            strokeLinecap="round"
            className="pointer-events-none"
            style={{
              filter: "blur(4px)",
            }}
          />
        </>
      )}
    </g>
  );
});
