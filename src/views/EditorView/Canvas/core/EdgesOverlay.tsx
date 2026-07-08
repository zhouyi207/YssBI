import React, { useMemo, useRef, useSyncExternalStore } from "react";
import { useShallow } from "zustand/react/shallow";
import { Edge } from "./Edge";
import { useExecutionStore, connectionKey, getExecutionVisual, subscribeExecutionVisual } from "@/features/core/execution";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useEdgeDragPreview } from "@/features/core/canvas/useEdgeDragPreview";
import { pinThemeTypeKey } from '@/shared/types/domain/pinSemantics';
import type { ConnectionData, PinData } from "@/shared/types";

interface EdgesOverlayProps {
  graphPath: string;
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
  dimmed?: boolean;
}

export interface EdgeData {
  id: string;
  fromPinId: string;
  toPinId: string;
  sourceNodeId: string;
  targetNodeId?: string;
  pinType: string;
  pinColor?: string;
}

export function buildEdgeData(
  graphNodeIds: string[],
  connections: ConnectionData[],
  getPin: (pinId: string) => PinData | undefined,
): EdgeData[] {
  const result: EdgeData[] = [];
  const nodeIdSet = new Set(graphNodeIds);
  for (const conn of connections) {
    const fromPin = getPin(conn.from);
    if (!fromPin || !nodeIdSet.has(fromPin.nodeId)) continue;
    const toPin = getPin(conn.to);
    result.push({
      id: conn.id,
      fromPinId: conn.from,
      toPinId: conn.to,
      sourceNodeId: fromPin.nodeId,
      targetNodeId: toPin?.nodeId,
      pinType: pinThemeTypeKey(fromPin),
      pinColor: fromPin.ui?.color,
    });
  }
  return result;
}

export const EdgesOverlay = React.memo<EdgesOverlayProps>(({ graphPath, getPinWorldPos, dimmed }) => {
  const { theme } = useTheme();
  const visual = useSyncExternalStore(subscribeExecutionVisual, getExecutionVisual, getExecutionVisual);
  const graphState = useExecutionStore((s) => s.graphs[graphPath]);
  const isReplay = useExecutionStore((s) => s.isPlaying && s.playbackGraphPath === graphPath);

  const useVisual = (visual.active && visual.graphPath === graphPath) || isReplay;
  const status = useVisual ? visual.status : (graphState?.status ?? "idle");
  const completedConnections = useVisual ? visual.completedConnections : graphState?.completedConnections;
  const flowingConnections = useVisual ? visual.flowingConnections : graphState?.flowingConnections;
  const nodeStates = useVisual ? undefined : graphState?.nodeStates;
  const isRunning = status === "running";

  const graphNodeIds = useGraphDataStore(
    useShallow((s) => s.getGraphNodeIds(graphPath)),
  );
  const connections = useGraphDataStore(
    useShallow((s) => s.getGraphConnections(graphPath)),
  );
  const sourcePins = useGraphDataStore(
    useShallow((s) => s.getGraphConnections(graphPath).map((conn) => s.getGraphPin(graphPath, conn.from))),
  );
  const targetPins = useGraphDataStore(
    useShallow((s) => s.getGraphConnections(graphPath).map((conn) => s.getGraphPin(graphPath, conn.to))),
  );

  const edges = useMemo<EdgeData[]>(() => {
    const pinsById = new Map<string, PinData>();
    for (const pin of [...sourcePins, ...targetPins]) {
      if (pin) pinsById.set(pin.id, pin);
    }
    return buildEdgeData(graphNodeIds, connections, (pinId) => pinsById.get(pinId));
  }, [connections, graphNodeIds, sourcePins, targetPins]);

  const svgRef = useRef<SVGSVGElement>(null);
  useEdgeDragPreview(svgRef, edges, getPinWorldPos);

  return (
    <svg
      ref={svgRef}
      className="absolute pointer-events-none"
      style={{ overflow: "visible", left: 0, top: 0, width: 1, height: 1, zIndex: 0 }}
    >
      <style>{`
        @keyframes edgeFlowData { to { stroke-dashoffset: -40; } }
        @keyframes edgePullData { to { stroke-dashoffset: 40; } }
        @keyframes edgeFlowExec { to { stroke-dashoffset: -16; } }
        @keyframes edgeGlowData { 0%,100% { opacity: .3; } 50% { opacity: .7; } }
        @keyframes edgePullGlow { 0%,100% { opacity: .2; } 50% { opacity: .5; } }
        @keyframes edgeGlowExec { 0%,100% { opacity: .5; } 50% { opacity: 1; } }
      `}</style>
      {edges.map((edge) => {
        const start = getPinWorldPos(edge.fromPinId);
        const end = getPinWorldPos(edge.toPinId);
        if (!start || !end) return null;

        const connKey = connectionKey(edge.fromPinId, edge.toPinId);
        const isError = useVisual
          ? visual.errorNodeIds.has(edge.sourceNodeId)
          : nodeStates?.get(edge.sourceNodeId)?.status === "error";
        const hasPull = completedConnections?.has(connKey) ?? false;
        const hasFlow = flowingConnections?.has(connKey) ?? false;
        // data：先取数、后流动；ConnectionFlow 仅在 pin 已有值时由后端发出
        const isPullActive = edge.pinType !== "exec" && hasPull && !hasFlow && !isError;
        const isFlowActive = edge.pinType === "exec" ? hasPull : hasFlow;
        const color = edge.pinColor ?? getPinTypeColor(edge.pinType, theme);
        const edgeKind = edge.pinType === "exec" ? "exec" : "data";

        return (
          <Edge
            key={edge.id}
            edgeId={edge.id}
            fromPinId={edge.fromPinId}
            toPinId={edge.toPinId}
            x1={start.x}
            y1={start.y}
            x2={end.x}
            y2={end.y}
            color={color}
            thickness={2}
            edgeKind={edgeKind}
            isPullActive={isPullActive}
            isFlowActive={isFlowActive}
            isError={isError}
            isRunning={isRunning}
            dimmed={dimmed}
          />
        );
      })}
    </svg>
  );
});
