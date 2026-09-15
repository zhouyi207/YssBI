import React from "react";
import { computeEdgePath } from "@/features/core/canvas";
import type { GraphElementState, GraphCacheAppearance } from "@/features/application/results";

interface EdgeProps {
  interactive?: boolean;
  edgeId?: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color?: string;
  thickness?: number;
  startIsInput?: boolean;
  state?: GraphElementState;
  cacheState?: GraphCacheAppearance;
  title?: string;
  dimmed?: boolean;
  replacementPreview?: boolean;
  selected?: boolean;
  onPointerDown?: (event: React.PointerEvent<SVGPathElement>) => void;
  onClick?: (event: React.MouseEvent<SVGPathElement>) => void;
  onContextMenu?: (event: React.MouseEvent<SVGPathElement>) => void;
  onDoubleClick?: (event: React.MouseEvent<SVGPathElement>) => void;
}

export const Edge = React.memo<EdgeProps>(
  ({
    edgeId,
    interactive = false,
    x1,
    y1,
    x2,
    y2,
    color = "var(--muted-foreground)",
    thickness = 2,
    startIsInput = false,
    state,
    cacheState,
    title,
    dimmed = false,
    replacementPreview = false,
    selected = false,
    onPointerDown,
    onClick,
    onContextMenu,
    onDoubleClick,
  }) => {
    const [hovered, setHovered] = React.useState(false);
    const pathData = computeEdgePath(x1, y1, x2, y2, startIsInput);
    const stroke = replacementPreview
      ? "var(--status-warning)"
      : state === "error"
        ? "var(--status-danger)"
        : color;
    return (
      <g
        className="graph-edge"
        data-edge-id={edgeId}
        data-selected={selected}
        data-hovered={hovered}
        data-graph-state={state}
        data-cache-state={cacheState}
        style={{ opacity: dimmed ? 0.25 : undefined, transition: "opacity 150ms" }}
      >
        {title && <title>{title}</title>}
        {(selected || hovered) && (
          <path
            data-edge-selection-visual={selected || undefined}
            data-edge-hover-visual={(!selected && hovered) || undefined}
            d={pathData}
            fill="none"
            stroke="var(--ring)"
            strokeOpacity={selected ? 0.55 : 0.35}
            strokeWidth={selected ? thickness + 14 : thickness + 5}
            strokeLinecap="round"
            className="pointer-events-none"
          />
        )}
        <path
          className="graph-edge-line pointer-events-none"
          d={pathData}
          fill="none"
          stroke={stroke}
          strokeWidth={state === "valid" || replacementPreview ? thickness + 1 : thickness}
          strokeLinecap="round"
        />
        {(state === "compiling" || state === "running") && (
          <path
            className="graph-edge-activity pointer-events-none"
            d={pathData}
            fill="none"
            stroke={stroke}
            strokeWidth={thickness + 1}
            strokeLinecap="round"
          />
        )}
        {(interactive || onPointerDown || onClick || onContextMenu || onDoubleClick) && (
          <path
            data-edge-hit-target={edgeId}
            d={pathData}
            fill="none"
            stroke="transparent"
            strokeWidth={12}
            pointerEvents="stroke"
            onPointerEnter={() => setHovered(true)}
            onPointerLeave={() => setHovered(false)}
            onPointerDown={onPointerDown}
            onClick={onClick}
            onContextMenu={onContextMenu}
            onDoubleClick={onDoubleClick}
          />
        )}
      </g>
    );
  },
);
