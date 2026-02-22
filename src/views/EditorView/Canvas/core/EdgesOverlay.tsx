import React, { useMemo } from "react";
import { Edge } from "./Edge";
import { useExecutionStore } from "@/features/core/execution";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import type { Pin } from "@/shared/types/domain";

interface EdgesOverlayProps {
  nodes: Array<{
    id: string;
    outputs: Pin[];
  }>;
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
}

interface EdgeData {
  id: string;
  fromPinId: string;
  toPinId: string;
  sourceNodeId: string;
  pinType: string;
  pinColor?: string;
}

export const EdgesOverlay = React.memo<EdgesOverlayProps>(({ nodes, getPinWorldPos }) => {
  const { theme } = useTheme();
  const completedConnections = useExecutionStore((s) => s.completedConnections);
  const nodeStates = useExecutionStore((s) => s.nodeStates);

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
      {edges.map((edge) => {
        const start = getPinWorldPos(edge.fromPinId);
        const end = getPinWorldPos(edge.toPinId);
        if (!start || !end) return null;

        const isCompleted = completedConnections.has(edge.id);
        const sourceState = nodeStates.get(edge.sourceNodeId);
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
          />
        );
      })}
    </svg>
  );
});
