import React from "react";
import { computeEdgePath } from "@/features/core/canvas";
import type { GraphElementState, GraphCacheAppearance } from "@/features/application/results";

interface EdgeAppearanceProps {
  interactive?: boolean;
  edgeId?: string;
  color?: string;
  thickness?: number;
  state?: GraphElementState;
  cacheState?: GraphCacheAppearance;
  title?: string;
  replacementPreview?: boolean;
  selected?: boolean;
}

type EdgeProps = EdgeAppearanceProps &
  (
    | { pathRef: React.RefCallback<SVGPathElement> }
    | { x1: number; y1: number; x2: number; y2: number; startIsInput?: boolean }
  );

export const Edge = React.memo<EdgeProps>(function Edge(props) {
  const {
    edgeId,
    interactive = false,
    color = "var(--muted-foreground)",
    thickness = 2,
    state,
    cacheState,
    title,
    replacementPreview = false,
    selected = false,
  } = props;
  const [hovered, setHovered] = React.useState(false);
  const geometry =
    "pathRef" in props
      ? { ref: props.pathRef }
      : { d: computeEdgePath(props.x1, props.y1, props.x2, props.y2, props.startIsInput) };
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
      style={{ transition: "opacity 150ms" }}
    >
      {title && <title>{title}</title>}
      {(selected || hovered) && (
        <path
          data-edge-selection-visual={selected || undefined}
          data-edge-hover-visual={(!selected && hovered) || undefined}
          {...geometry}
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
        {...geometry}
        fill="none"
        stroke={stroke}
        strokeWidth={state === "valid" || replacementPreview ? thickness + 1 : thickness}
        strokeLinecap="round"
      />
      {state === "running" && (
        <path
          className="graph-edge-activity pointer-events-none"
          {...geometry}
          fill="none"
          stroke={stroke}
          strokeWidth={thickness + 1}
          strokeLinecap="round"
        />
      )}
      {interactive && (
        <path
          data-edge-hit-target={edgeId}
          {...geometry}
          fill="none"
          stroke="transparent"
          strokeWidth={12}
          pointerEvents="stroke"
          onPointerEnter={() => setHovered(true)}
          onPointerLeave={() => setHovered(false)}
        />
      )}
    </g>
  );
});
