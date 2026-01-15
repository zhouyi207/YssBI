import React from "react";
import { type NodeProps } from "./Node";

export interface Connection {
  id: string;
  from: string;
  to: string;
  selected?: boolean;
}

export interface EdgeProps {
  connection: Connection;
  components: NodeProps[];
  onSelect?: (id: string) => void;
}

export const Edge: React.FC<EdgeProps> = ({
  connection,
  components,
  onSelect,
}) => {
  const fromComp = components.find(c => c.id === connection.from);
  const toComp = components.find(c => c.id === connection.to);

  if (!fromComp || !toComp) return null;

  const fromX = fromComp.x + (fromComp.width ?? 0);
  const fromY = fromComp.y + (fromComp.height ?? 0) / 2;
  const toX = toComp.x;
  const toY = toComp.y + (toComp.height ?? 0) / 2;

  return (
    <g>
      {/* 选中轮廓（最底层） */}
      {connection.selected && (
        <line
          x1={fromX}
          y1={fromY}
          x2={toX}
          y2={toY}
          stroke="#60a5fa"
          strokeWidth={6}
          strokeLinecap="round"
        />
      )}

      {/* 主线 */}
      <line
        x1={fromX}
        y1={fromY}
        x2={toX}
        y2={toY}
        stroke="#333"
        strokeWidth={2}
        strokeLinecap="round"
        markerEnd="url(#arrowhead)"
      />

      {/* 点击命中层（最上层） */}
      <line
        x1={fromX}
        y1={fromY}
        x2={toX}
        y2={toY}
        stroke="transparent"
        strokeWidth={40}
        pointerEvents="stroke"
        onClick={(e) => {
          e.stopPropagation();
          onSelect?.(connection.id);
        }}
      />
    </g>
  );
};

/* ================= Marker ================= */

export const ArrowheadMarker: React.FC = () => (
  <defs>
    <marker
      id="arrowhead"
      markerWidth="10"
      markerHeight="7"
      refX="9"
      refY="3.5"
      orient="auto"
      markerUnits="strokeWidth"
    >
      <polygon points="0 0, 10 3.5, 0 7" fill="#333" />
    </marker>
  </defs>
);
