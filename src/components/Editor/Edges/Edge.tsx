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
}) => {
  // ... (keep the same implementation)
  const dx = Math.abs(x1 - x2);
  const curvature = Math.max(dx * 0.5, 40);

  const dir = startIsInput ? -1 : 1;
  
  const c1x = x1 + curvature * dir;
  const c1y = y1;
  const c2x = x2 - curvature * dir;
  const c2y = y2;
  
  const pathData = `M ${x1},${y1} C ${c1x},${c1y} ${c2x},${c2y} ${x2},${y2}`;

  return (
    <path
      d={pathData}
      fill="none"
      stroke={color}
      strokeWidth={thickness}
      strokeLinecap="round"
      className="pointer-events-none"
    />
  );
});
