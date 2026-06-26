import React, { useMemo } from "react";
import { Edge } from "./Edge";
import { useExecutionStore } from "@/features/core/execution";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";

interface EdgesOverlayProps {
  graphId: string;
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

export const EdgesOverlay = React.memo<EdgesOverlayProps>(({ graphId, getPinWorldPos, dimmed }) => {
  const { theme } = useTheme();
  const graphState = useExecutionStore((s) => s.graphs[graphId]);
  const status = graphState?.status ?? "idle";
  const completedConnections = graphState?.completedConnections;
  const nodeStates = graphState?.nodeStates;
  const isRunning = status === "running";

  const connections = useGraphDataStore((s) => s.connections);
  const pins = useGraphDataStore((s) => s.pins);
  const graphNodeIds = useGraphDataStore((s) => s.graphNodes[graphId]);

  const edges = useMemo<EdgeData[]>(() => {
    const result: EdgeData[] = [];
    const nodeIdSet = new Set(graphNodeIds ?? []);
    for (const conn of Object.values(connections)) {
      const fromPin = pins[conn.from];
      if (!fromPin || !nodeIdSet.has(fromPin.nodeId)) continue;
      result.push({
        id: conn.id,
        fromPinId: conn.from,
        toPinId: conn.to,
        sourceNodeId: fromPin.nodeId,
        pinType: fromPin.type ?? "any",
        pinColor: fromPin.ui?.color,
      });
    }
    return result;
  }, [connections, pins, graphNodeIds]);

  return (
    <svg
      className="absolute pointer-events-none"
      style={{ overflow: "visible", left: 0, top: 0, width: 1, height: 1, zIndex: 0 }}
    >
      <style>{`
        @keyframes edgeFlowData { to { stroke-dashoffset: -40; } }
        @keyframes edgeFlowExec { to { stroke-dashoffset: -16; } }
        @keyframes edgeGlowData { 0%,100% { opacity: .3; } 50% { opacity: .7; } }
        @keyframes edgeGlowExec { 0%,100% { opacity: .5; } 50% { opacity: 1; } }
      `}</style>
      {edges.map((edge) => {
        const start = getPinWorldPos(edge.fromPinId);
        const end = getPinWorldPos(edge.toPinId);
        if (!start || !end) return null;

        const isCompleted = completedConnections?.has(edge.id) ?? false;
        const sourceState = nodeStates?.get(edge.sourceNodeId);
        const isError = sourceState?.status === "error";
        const color = edge.pinColor ?? getPinTypeColor(edge.pinType, theme);
        const edgeKind = edge.pinType === "exec" ? "exec" : "data";

        return (
          <Edge
            key={edge.id}
            x1={start.x}
            y1={start.y}
            x2={end.x}
            y2={end.y}
            color={color}
            thickness={2}
            edgeKind={edgeKind}
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
