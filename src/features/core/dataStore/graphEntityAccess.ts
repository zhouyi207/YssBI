import type {
  ConnectionData,
  ConnectionId,
  GraphId,
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
  graphEntities: Record<GraphId, GraphEntityBucket>;
}

export function getGraphBucket(
  state: GraphEntitiesState,
  graphId: GraphId,
): GraphEntityBucket | undefined {
  return state.graphEntities[graphId];
}

export function hasGraphData(state: GraphEntitiesState, graphId: GraphId): boolean {
  return graphId in state.graphEntities;
}

export function getGraphNodeIds(state: GraphEntitiesState, graphId: GraphId): NodeId[] {
  return state.graphEntities[graphId]?.graphNodes ?? [];
}

export function getGraphConnection(
  state: GraphEntitiesState,
  graphId: GraphId,
  connectionId: ConnectionId,
): ConnectionData | undefined {
  return state.graphEntities[graphId]?.connections[connectionId];
}
