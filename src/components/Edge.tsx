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

export const Edge: React.FC<EdgeProps> = ({
  x1,
  y1,
  x2,
  y2,
  color = "#999",
  thickness = 2,
  startIsInput = false,
}) => {
  // 曲线曲率：基于水平距离，最小 40
  const dx = Math.abs(x1 - x2);
  const curvature = Math.max(dx * 0.5, 40);

  // 如果起点是输出 (Standard)，则起点切线向右 (+)，终点切线向左 (-)
  // 如果起点是输入 (Flipped)，则起点切线向左 (-)，终点切线向右 (+)
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
};
