import React, { useMemo } from "react";
import { Edge } from "./Edge";
import { useExecutionStore } from "@/features/core/execution";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import type { Pin } from "@/shared/types/domain";

interface EdgesOverlayProps {
  graphId: string;
  nodes: Array<{
    id: string;
    outputs: Pin[];
  }>;
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
  dimmed?: boolean;
}

interface EdgeData {
  id: string;
  fromPinId: string;
  toPinId: string;
  sourceNodeId: string;
  pinType: string;
  pinColor?: string;
}

export const EdgesOverlay = React.memo<EdgesOverlayProps>(({ graphId, nodes, getPinWorldPos, dimmed }) => {
  const { theme } = useTheme();
  const graphState = useExecutionStore((s) => s.graphs[graphId]);
  const status = graphState?.status ?? "idle";
  const completedConnections = graphState?.completedConnections;
  const nodeStates = graphState?.nodeStates;
  const isRunning = status === "running";

  const edges = useMemo<EdgeData[]>(() => {
    const result: EdgeData[] = [];
    for (const node of nodes) {
      if (!node?.outputs) continue;
      for (const pin of node.outputs) {
        if (!pin.links) continue;
        for (const targetId of pin.links) {
          result.push({
            id: `${pin.id}->${targetId}`,
            fromPinId: pin.id,
            toPinId: targetId,
            sourceNodeId: node.id,
            pinType: pin.type ?? "any",
            pinColor: pin.ui?.color,
          });
        }
      }
    }
    return result;
  }, [nodes]);

  return (
    <svg
      className="absolute pointer-events-none"
      style={{ overflow: "visible", left: 0, top: 0, width: 1, height: 1, zIndex: 0 }}
    >
      <style>{`
        @keyframes edgeFlow { to { stroke-dashoffset: -40; } }
        @keyframes edgeGlow { 0%,100% { opacity: .3; } 50% { opacity: .7; } }
      `}</style>
      {edges.map((edge) => {
        const start = getPinWorldPos(edge.fromPinId);
        const end = getPinWorldPos(edge.toPinId);
        if (!start || !end) return null;

        const isCompleted = completedConnections?.has(edge.id) ?? false;
        const sourceState = nodeStates?.get(edge.sourceNodeId);
        const isError = sourceState?.status === "error";
        const color = edge.pinColor ?? getPinTypeColor(edge.pinType, theme);

        return (
          <Edge
            key={edge.id}
            x1={start.x}
            y1={start.y}
            x2={end.x}
            y2={end.y}
            color={color}
            thickness={2}
            isCompleted={isCompleted}
            isError={isError}
            isRunning={isRunning}
            dimmed={dimmed}
          />
        );
      })}
    </svg>
  );
});
