import type {
  ConnectionData,
  ConnectionId,
  GraphPath,
  NodeData,
  NodeId,
  PinData,
  PinId,
} from '@/shared/types';

/** Per-graph normalized entity bucket — sole authority for graph topology in the store. */
export interface GraphEntityBucket {
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;
  graphNodes: NodeId[];
  nodePins: Record<NodeId, PinId[]>;
  pinConnections: Record<PinId, ConnectionId[]>;
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

export function hasGraphData(state: GraphEntitiesState , graphPath: GraphPath): boolean {
  return graphPath in state.graphEntities;
}

export function getGraphNodeIds(state: GraphEntitiesState , graphPath: GraphPath): NodeId[] {
  return state.graphEntities[graphPath]?.graphNodes ?? [];
}

export function getGraphConnection(
  state: GraphEntitiesState,
  graphPath: GraphPath,
  connectionId: ConnectionId,
): ConnectionData | undefined {
  return state.graphEntities[graphPath]?.connections[connectionId];
}
