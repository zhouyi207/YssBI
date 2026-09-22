import type {
  ConnectionData,
  ConnectionId,
  GraphPath,
  NodeData,
  NodeId,
  PinData,
  PinId,
} from "@/features/domain/editorProjection/graphRuntimeTypes";
import type {
  ResolutionOutcomeDto,
  DiagnosticDto,
  ProjectionBasisDto,
} from "@/shared/types/domain/editorProjection";

/** Per-graph normalized entity bucket — sole authority for graph topology in the store. */
export interface GraphEntityBucket {
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;
  graphNodes: NodeId[];
  pinConnections: Record<PinId, ConnectionId[]>;
  basis: ProjectionBasisDto;
  /** Complete canonical problem set copied from the same projection as the entities. */
  diagnostics: DiagnosticDto[];
  outcome: ResolutionOutcomeDto;
  hasBlockingDiagnostics: boolean;
}

export interface GraphEntitiesState {
  graphEntities: Record<GraphPath, GraphEntityBucket>;
}

export function hasGraphData(state: GraphEntitiesState, graphPath: GraphPath): boolean {
  return graphPath in state.graphEntities;
}

export function getGraphNodeIds(state: GraphEntitiesState, graphPath: GraphPath): NodeId[] {
  return state.graphEntities[graphPath]?.graphNodes ?? [];
}

export function getGraphNode(
  state: GraphEntitiesState,
  graphPath: GraphPath,
  nodeId: NodeId,
): NodeData | undefined {
  return state.graphEntities[graphPath]?.nodes[nodeId];
}

export function getGraphPin(
  state: GraphEntitiesState,
  graphPath: GraphPath,
  pinId: PinId,
): PinData | undefined {
  return state.graphEntities[graphPath]?.pins[pinId];
}

export function getGraphNodePins(
  state: GraphEntitiesState,
  graphPath: GraphPath,
  nodeId: NodeId,
): PinId[] {
  return state.graphEntities[graphPath]?.nodes[nodeId]?.pinIds ?? [];
}

export function getGraphPinConnections(
  state: GraphEntitiesState,
  graphPath: GraphPath,
  pinId: PinId,
): ConnectionId[] {
  return state.graphEntities[graphPath]?.pinConnections[pinId] ?? [];
}

export function getGraphConnection(
  state: GraphEntitiesState,
  graphPath: GraphPath,
  connectionId: ConnectionId,
): ConnectionData | undefined {
  return state.graphEntities[graphPath]?.connections[connectionId];
}

export function getGraphConnections(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): ConnectionData[] {
  const bucket = state.graphEntities[graphPath];
  return bucket ? Object.values(bucket.connections) : [];
}

export function isGraphProjectionExecutable(
  projection: Pick<GraphEntityBucket, "outcome" | "hasBlockingDiagnostics"> | undefined,
): boolean {
  return (
    projection !== undefined &&
    projection.outcome.type === "success" &&
    !projection.hasBlockingDiagnostics
  );
}
