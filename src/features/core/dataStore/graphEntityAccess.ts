import type {
  ConnectionData,
  ConnectionId,
  GraphPath,
  NodeData,
  NodeId,
  PinData,
  PinId,
} from "@/shared/types";
import type {
  CompilationOutcomeDto,
  DiagnosticDto,
  ProjectionBasisDto,
} from "@/shared/types/domain/editorProjection";

/** Per-graph normalized entity bucket — sole authority for graph topology in the store. */
export interface GraphEntityBucket {
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;
  graphNodes: NodeId[];
  nodePins: Record<NodeId, PinId[]>;
  pinConnections: Record<PinId, ConnectionId[]>;
  basis: ProjectionBasisDto;
  sourceRevision: number;
  requestGeneration: number;
  diagnostics: DiagnosticDto[];
  outcome: CompilationOutcomeDto;
  hasBlockingDiagnostics: boolean;
}

export interface GraphEntitiesState {
  graphEntities: Record<GraphPath, GraphEntityBucket>;
}

export function getGraphBucket(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): GraphEntityBucket | undefined {
  return state.graphEntities[graphPath];
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
  return state.graphEntities[graphPath]?.nodePins[nodeId] ?? [];
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

export function getGraphProjectionBasis(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): ProjectionBasisDto | undefined {
  return state.graphEntities[graphPath]?.basis;
}

export function getGraphSourceRevision(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): number | undefined {
  return state.graphEntities[graphPath]?.sourceRevision;
}

export function getGraphRequestGeneration(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): number | undefined {
  return state.graphEntities[graphPath]?.requestGeneration;
}

export function getGraphDiagnostics(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): DiagnosticDto[] | undefined {
  return state.graphEntities[graphPath]?.diagnostics;
}

export function hasGraphBlockingDiagnostics(
  state: GraphEntitiesState,
  graphPath: GraphPath,
): boolean | undefined {
  return state.graphEntities[graphPath]?.hasBlockingDiagnostics;
}
