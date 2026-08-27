import React, { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { useShallow } from "zustand/react/shallow";
import { Edge } from "./Edge";
import { useExecutionStore, connectionKey, getExecutionVisual, subscribeExecutionVisual } from "@/features/core/execution";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useTheme } from "@/features/core/theme/useTheme";
import { getPinTypeColor } from "@/features/core/theme/pinTypeTheme";
import { useEdgeDragPreview } from "@/features/core/canvas/useEdgeDragPreview";
import { getConnectPreview, subscribeConnectPreview } from '@/features/core/canvas/connectPreview';
import { resolvePinVisualSpec } from '@/shared/types/domain/pinVisual';
import type { ConnectionData, PinData } from "@/shared/types";
import type { EdgeData } from '@/features/domain/canvas/edgeData';
import { ConnectionContextMenu } from '@/views/EditorView/ContextMenu';

export function replacementEdgeAttributes(
  connectionId: string,
  highlightedConnectionIds: ReadonlySet<string>,
) {
  return highlightedConnectionIds.has(connectionId)
    ? { 'data-replacement-preview': true as const }
    : {};
}


export interface EdgeDoubleClickSelectionSnapshot {
  before: { nodeIds: Set<string>; connectionIds: Set<string> };
  temporary: { nodeIds: Set<string>; connectionIds: Set<string> };
}

interface EdgesOverlayBaseProps {
  graphPath: string;
  groupId: string;
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
  getCanvasLocalPoint: (clientX: number, clientY: number) => { x: number; y: number };
  dimmed?: boolean;
}

type EdgesOverlayProps = EdgesOverlayBaseProps & (
  | { mode: 'preview' }
  | {
      mode: 'interactive';
      selectedNodeIds: readonly string[];
      selectedConnectionIds: readonly string[];
      onSelectedConnectionIdsChange: (
        connectionIds: string[],
        graphPath: string,
        groupId: string,
      ) => void;
      onBreakConnections: (
        connectionIds: string[],
        graphPath: string,
        groupId: string,
      ) => boolean | void | Promise<boolean | void>;
      onEdgeDoubleClick: (
        connectionId: string,
        position: Readonly<{ x: number; y: number }>,
        graphPath: string,
        groupId: string,
        selection: EdgeDoubleClickSelectionSnapshot,
      ) => void;
    }
);

interface EdgeContextMenuDescriptor {
  position: { x: number; y: number };
  connectionIds: string[];
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
    const visual = resolvePinVisualSpec(fromPin);
    result.push({
      id: conn.id,
      fromPinId: conn.from,
      toPinId: conn.to,
      sourceNodeId: fromPin.nodeId,
      targetNodeId: toPin?.nodeId,
      colorKey: visual.colorKey,
      edgeKind: visual.edgeKind,
    });
  }
  return result;
}

const EMPTY_SELECTED_IDS: readonly string[] = [];

export const EdgesOverlay = React.memo(function EdgesOverlay(props: EdgesOverlayProps) {
  const {
    graphPath,
    groupId,
    getPinWorldPos,
    getCanvasLocalPoint,
    dimmed,
  } = props;
  const interaction = props.mode === 'interactive' ? props : null;
  const selectedNodeIds = interaction?.selectedNodeIds ?? EMPTY_SELECTED_IDS;
  const selectedConnectionIds = interaction?.selectedConnectionIds ?? EMPTY_SELECTED_IDS;
  const { tokens } = useTheme();
  const visual = useSyncExternalStore(subscribeExecutionVisual, getExecutionVisual, getExecutionVisual);
  const getScopedConnectPreview = () => getConnectPreview({ graphPath, groupId });
  const connectPreview = useSyncExternalStore(
    subscribeConnectPreview,
    getScopedConnectPreview,
    getScopedConnectPreview,
  );
  const highlightedConnectionIds = new Set(connectPreview.highlightedConnectionIds);
  const selectedConnectionIdSet = useMemo(
    () => new Set(selectedConnectionIds),
    [selectedConnectionIds],
  );
  const [edgeContextMenu, setEdgeContextMenu] = useState<EdgeContextMenuDescriptor | null>(null);
  const pendingDoubleClickRef = useRef<{
    connectionId: string;
    snapshot: EdgeDoubleClickSelectionSnapshot;
  } | null>(null);
  useEffect(() => {
    if (props.mode === 'preview') setEdgeContextMenu(null);
  }, [props.mode]);

  const handleEdgePointerDown = useCallback((event: React.PointerEvent<SVGPathElement>) => {
    event.preventDefault();
    event.stopPropagation();
  }, []);

  const handleEdgeClick = useCallback((connectionId: string, event: React.MouseEvent<SVGPathElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (event.button !== 0 || event.detail !== 1 || !interaction) return;
    const toggle = event.ctrlKey || event.metaKey || event.shiftKey;
    const next = toggle
      ? (selectedConnectionIdSet.has(connectionId)
        ? selectedConnectionIds.filter((id) => id !== connectionId)
        : [...selectedConnectionIds, connectionId])
      : [connectionId];
    pendingDoubleClickRef.current = {
      connectionId,
      snapshot: {
        before: {
          nodeIds: new Set(selectedNodeIds),
          connectionIds: new Set(selectedConnectionIds),
        },
        temporary: {
          nodeIds: new Set(),
          connectionIds: new Set(next),
        },
      },
    };
    interaction.onSelectedConnectionIdsChange([...next], graphPath, groupId);
    setEdgeContextMenu(null);
  }, [graphPath, groupId, interaction, selectedConnectionIdSet, selectedConnectionIds, selectedNodeIds]);

  const handleEdgeDoubleClick = useCallback((connectionId: string, event: React.MouseEvent<SVGPathElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (!interaction) return;
    const position = getCanvasLocalPoint(event.clientX, event.clientY);
    const pending = pendingDoubleClickRef.current;
    const snapshot = pending?.connectionId === connectionId
      ? pending.snapshot
      : {
          before: {
            nodeIds: new Set(selectedNodeIds),
            connectionIds: new Set(selectedConnectionIds),
          },
          temporary: {
            nodeIds: new Set(selectedNodeIds),
            connectionIds: new Set(selectedConnectionIds),
          },
        };
    pendingDoubleClickRef.current = null;
    interaction.onEdgeDoubleClick(connectionId, position, graphPath, groupId, snapshot);
  }, [getCanvasLocalPoint, graphPath, groupId, interaction, selectedConnectionIds, selectedNodeIds]);

  const handleEdgeContextMenu = useCallback((connectionId: string, event: React.MouseEvent<SVGPathElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (!interaction) return;
    const connectionIds = selectedConnectionIdSet.has(connectionId)
      ? [...selectedConnectionIds]
      : [connectionId];
    if (!selectedConnectionIdSet.has(connectionId)) {
      interaction.onSelectedConnectionIdsChange(connectionIds, graphPath, groupId);
    }
    setEdgeContextMenu({
      position: { x: event.clientX, y: event.clientY },
      connectionIds,
    });
  }, [graphPath, groupId, interaction, selectedConnectionIdSet, selectedConnectionIds]);
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
  useEdgeDragPreview(svgRef, edges, getPinWorldPos, { graphPath, groupId });

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
        const isPullActive = edge.edgeKind === 'data' && hasPull && !hasFlow && !isError;
        const isFlowActive = edge.edgeKind === 'exec' ? hasPull : hasFlow;
        const color = getPinTypeColor(edge.colorKey, tokens);
        const edgeKind = edge.edgeKind;

        return (
          <g key={edge.id} {...replacementEdgeAttributes(edge.id, highlightedConnectionIds)}>
          <Edge
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
            replacementPreview={highlightedConnectionIds.has(edge.id)}
            selected={selectedConnectionIdSet.has(edge.id)}
            onPointerDown={interaction ? handleEdgePointerDown : undefined}
            onClick={interaction ? (event) => handleEdgeClick(edge.id, event) : undefined}
            onContextMenu={interaction
              ? (event) => handleEdgeContextMenu(edge.id, event)
              : undefined}
            onDoubleClick={interaction
              ? (event) => handleEdgeDoubleClick(edge.id, event)
              : undefined}
          />
          </g>
        );
      })}
      {interaction && edgeContextMenu ? (
        <ConnectionContextMenu
          position={edgeContextMenu.position}
          selectedCount={edgeContextMenu.connectionIds.length}
          onBreak={() => {
            void interaction.onBreakConnections(
              edgeContextMenu.connectionIds,
              graphPath,
              groupId,
            );
          }}
          onClose={() => setEdgeContextMenu(null)}
        />
      ) : null}
    </svg>
  );
});
