import { create } from 'zustand';
import {
  NodeId,
  PinId,
  GraphPath,
  ConnectionId,
  GraphDataLike,
  NodeData,
  PinData,
  ConnectionData,
} from '@/shared/types';
import { normalizeGraphDataLike } from '@/shared/types/dto/graphModel';
import { isExecPin } from '@/shared/types/domain/pinSemantics';
import { resolveNodeViewMeta } from '@/features/domain/nodeViewMeta';
import { logger } from '@/utils/appLogger';
import {
  type GraphEntityBucket,
  getGraphConnection,
  getGraphNodeIds,
  hasGraphData,
} from './graphEntityAccess';

export type { GraphEntityBucket } from './graphEntityAccess';

type PinDataInput = PinData & { links?: string[] };

function emptyGraphBucket(): GraphEntityBucket {
  return {
    nodes: {},
    pins: {},
    connections: {},
    graphNodes: [],
    nodePins: {},
    pinConnections: {},
  };
}

function cloneGraphBucket(bucket: GraphEntityBucket): GraphEntityBucket {
  return {
    nodes: { ...bucket.nodes },
    pins: { ...bucket.pins },
    connections: { ...bucket.connections },
    graphNodes: [...bucket.graphNodes],
    nodePins: { ...bucket.nodePins },
    pinConnections: { ...bucket.pinConnections },
  };
}

function commitGraphBucket(
  state: { graphEntities: Record<GraphPath, GraphEntityBucket> },
  graphPath: GraphPath,
  bucket: GraphEntityBucket,
) {
  return {
    graphEntities: { ...state.graphEntities, [graphPath]: bucket },
  };
}

function withGraphBucket(
  state: { graphEntities: Record<GraphPath, GraphEntityBucket> },
  graphPath: GraphPath,
  mutate: (bucket: GraphEntityBucket) => GraphEntityBucket | null,
  notFoundLabel?: string,
) {
  const current = state.graphEntities[graphPath];
  if (!current) {
    if (notFoundLabel) {
      logger.graph.warn(`${notFoundLabel}: graph "${graphPath}" not loaded`, 'GraphDataStore');
    }
    return state;
  }
  const bucket = cloneGraphBucket(current);
  const next = mutate(bucket);
  if (!next) return state;
  return commitGraphBucket(state, graphPath, next);
}

function disconnectBucketConnection(bucket: GraphEntityBucket, connectionId: ConnectionId): void {
  const conn = bucket.connections[connectionId];
  if (!conn) return;
  bucket.pinConnections[conn.from] = (bucket.pinConnections[conn.from] ?? []).filter(
    (id) => id !== connectionId,
  );
  bucket.pinConnections[conn.to] = (bucket.pinConnections[conn.to] ?? []).filter(
    (id) => id !== connectionId,
  );
  delete bucket.connections[connectionId];
}

function deleteBucketNode(bucket: GraphEntityBucket, nodeId: NodeId): void {
  const pinIds = bucket.nodePins[nodeId] ?? [];
  for (const pinId of pinIds) {
    for (const connId of bucket.pinConnections[pinId] ?? []) {
      disconnectBucketConnection(bucket, connId);
    }
    delete bucket.pinConnections[pinId];
    delete bucket.pins[pinId];
  }
  delete bucket.nodePins[nodeId];
  delete bucket.nodes[nodeId];
  bucket.graphNodes = bucket.graphNodes.filter((id) => id !== nodeId);
}

function connectBucketPins(bucket: GraphEntityBucket, from: PinId, to: PinId): void {
  const id: ConnectionId = `${from}->${to}`;
  if (bucket.connections[id]) return;
  bucket.connections[id] = { id, from, to };
  bucket.pinConnections[from] = [...(bucket.pinConnections[from] ?? []), id];
  bucket.pinConnections[to] = [...(bucket.pinConnections[to] ?? []), id];
}

function toStoredPin(pin: PinDataInput): PinData {
  const { links: _links, ...stored } = pin;
  return stored;
}

/** 注册表 enrich：title / category 以 catalog 为权威（uiStyle 在视图层推导）。 */
function enrichNodeData(node: NodeData): NodeData {
  const meta = resolveNodeViewMeta(node);
  return {
    ...node,
    category: meta.category,
    title: meta.title,
    description: meta.description ?? node.description,
  };
}

function buildGraphBucket(graphPath: GraphPath, graph: GraphDataLike): GraphEntityBucket {
  const normalized = normalizeGraphDataLike(graphPath, graph);
  const bucket = emptyGraphBucket();

  normalized.nodes.forEach((node) => {
    const enriched = enrichNodeData(node);
    bucket.nodes[enriched.id] = enriched;
    bucket.graphNodes.push(enriched.id);
    const pinIds = [...enriched.inputs, ...enriched.outputs];
    bucket.nodePins[node.id] = pinIds;
    pinIds.forEach((pinId) => {
      bucket.pinConnections[pinId] = bucket.pinConnections[pinId] ?? [];
    });
  });

  normalized.pins.forEach((pin: PinDataInput) => {
    bucket.pins[pin.id] = toStoredPin(pin);
  });

  normalized.connections.forEach((connection) => {
    connectBucketPins(bucket, connection.from, connection.to);
  });

  return bucket;
}

/** applyConnectionDraft 的结果：用于后端失败时回滚乐观连接 */
export interface ConnectionDraft {
  from: PinId;
  to: PinId;
  connectionId: ConnectionId;
  disconnectedIds: ConnectionId[];
}

interface GraphDataStore {
  graphEntities: Record<GraphPath, GraphEntityBucket>;

  addNode(graphPath: GraphPath, node: NodeData): void;
  updateNode(nodeId: NodeId, patch: Partial<NodeData> , graphPath: GraphPath): void;
  batchUpdateNodePositions(updates: Array<{ nodeId: NodeId; x: number; y: number }> , graphPath: GraphPath): void;
  deleteNode(nodeId: NodeId , graphPath: GraphPath): void;
  batchAddNodesAndPins(graphPath: GraphPath, items: Array<{ node: NodeData; pins: PinData[] }>): void;
  batchDeleteNodes(nodeIds: NodeId[] , graphPath: GraphPath): void;
  applyNodeDraft(graphPath: GraphPath, node: NodeData, pins: PinData[]): void;
  revertNodeDraft(nodeId: NodeId , graphPath: GraphPath): void;
  reconcileNode(graphPath: GraphPath, node: NodeData, pins: PinData[]): void;

  addPin(nodeId: NodeId, pin: PinData , graphPath: GraphPath): void;
  updatePin(pinId: PinId, patch: Partial<PinData> , graphPath: GraphPath): void;
  batchUpdatePinFields(updates: Array<{ pinId: PinId; patch: Partial<PinData> }> , graphPath: GraphPath): void;
  deletePin(pinId: PinId , graphPath: GraphPath): void;
  batchUpdatePins(params: {
    disconnectIds: ConnectionId[];
    removePinIds: PinId[];
    updatePins?: Array<{ pinId: PinId; patch: Partial<PinData> }>;
    addPins: Array<{ nodeId: NodeId; pin: PinData }>;
    graphPath: GraphPath;
  }): void;
  reorderNodePins(nodeId: NodeId, pinOrder: PinId[] , graphPath: GraphPath): void;

  connect(from: PinId, to: PinId , graphPath: GraphPath): void;
  disconnect(connectionId: ConnectionId , graphPath: GraphPath): void;
  batchDisconnect(connectionIds: ConnectionId[] , graphPath: GraphPath): void;
  applyConnectionDraft(pinA: PinId, pinB: PinId , graphPath: GraphPath): ConnectionDraft | null;
  revertConnectionDraft(draft: ConnectionDraft , graphPath: GraphPath): void;
  batchConnect(pairs: Array<{ from: PinId; to: PinId }> , graphPath: GraphPath): void;

  getGraphNode(graphPath: GraphPath, nodeId: NodeId): NodeData | undefined;
  getGraphPin(graphPath: GraphPath, pinId: PinId): PinData | undefined;
  getGraphNodeIds(graphPath: GraphPath): NodeId[];
  getGraphNodePins(graphPath: GraphPath, nodeId: NodeId): PinId[];
  getGraphPinConnections(graphPath: GraphPath, pinId: PinId): ConnectionId[];
  getGraphConnection(graphPath: GraphPath, connectionId: ConnectionId): ConnectionData | undefined;
  getGraphConnections(graphPath: GraphPath): ConnectionData[];
  hasGraph(graphPath: GraphPath): boolean;
  clearGraph(graphPath: GraphPath): void;
  hydrateGraphs(graphs: Record<GraphPath, GraphDataLike>): void;
  addGraphFromData(graphPath: GraphPath, graph: GraphDataLike): void;
}

export const useGraphDataStore = create<GraphDataStore>((set, get) => ({
  graphEntities: {},

  addNode: (graphPath, node) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        if (bucket.nodes[node.id]) {
          logger.graph.warn(`Node "${node.id}" already exists`, 'GraphDataStore');
          return null;
        }
        bucket.nodes[node.id] = node;
        bucket.graphNodes = [...bucket.graphNodes, node.id];
        bucket.nodePins[node.id] = [];
        return bucket;
      }),
    ),

  updateNode: (nodeId, patch, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        const prev = bucket.nodes[nodeId];
        if (!prev) {
          logger.graph.warn(`updateNode: Node "${nodeId}" not found`, 'GraphDataStore');
          return null;
        }
        bucket.nodes[nodeId] = { ...prev, ...patch };
        return bucket;
      }, 'updateNode'),
    ),

  batchUpdateNodePositions: (updates, graphPath) =>
    set((state) => {
      if (updates.length === 0) return state;
      return withGraphBucket(state, graphPath, (bucket) => {
        let changed = false;
        for (const { nodeId, x, y } of updates) {
          const prev = bucket.nodes[nodeId];
          if (prev?.position && (prev.position.x !== x || prev.position.y !== y)) {
            bucket.nodes[nodeId] = { ...prev, position: { x, y } };
            changed = true;
          }
        }
        return changed ? bucket : null;
      });
    }),

  deleteNode: (nodeId, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        if (!bucket.nodes[nodeId]) {
          logger.graph.warn(`deleteNode: Node "${nodeId}" not found`, 'GraphDataStore');
          return null;
        }
        deleteBucketNode(bucket, nodeId);
        return bucket;
      }),
    ),

  batchAddNodesAndPins: (graphPath, items) =>
    set((state) => {
      if (items.length === 0) return state;
      return withGraphBucket(state, graphPath, (bucket) => {
        for (const { node, pins } of items) {
          if (bucket.nodes[node.id]) continue;
          bucket.nodes[node.id] = node;
          bucket.graphNodes.push(node.id);
          const pinIds: PinId[] = [];
          for (const pin of pins) {
            if (!bucket.pins[pin.id]) {
              bucket.pins[pin.id] = toStoredPin(pin);
              pinIds.push(pin.id);
              bucket.pinConnections[pin.id] = bucket.pinConnections[pin.id] ?? [];
            }
          }
          bucket.nodePins[node.id] = pinIds;
        }
        return bucket;
      });
    }),

  applyNodeDraft: (graphPath, node, pins) => get().batchAddNodesAndPins(graphPath, [{ node, pins }]),

  revertNodeDraft: (nodeId, graphPath) => get().deleteNode(nodeId, graphPath),

  reconcileNode: (graphPath, node, pins) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        const existing = bucket.nodes[node.id];
        bucket.nodes[node.id] = existing ? { ...existing, ...node } : node;
        if (!existing) bucket.graphNodes = [...bucket.graphNodes, node.id];

        const existingPinIds = bucket.nodePins[node.id] ?? [];
        for (const pin of pins) {
          const prev = bucket.pins[pin.id];
          bucket.pins[pin.id] = prev ? { ...prev, ...pin } : toStoredPin(pin);
          bucket.pinConnections[pin.id] = bucket.pinConnections[pin.id] ?? [];
        }
        const authoritativeIds = new Set(pins.map((p) => p.id));
        for (const pid of existingPinIds) {
          if (!authoritativeIds.has(pid)) {
            delete bucket.pins[pid];
            delete bucket.pinConnections[pid];
          }
        }
        bucket.nodePins[node.id] = pins.map((p) => p.id);
        return bucket;
      }, 'reconcileNode'),
    ),

  batchDeleteNodes: (nodeIds, graphPath) =>
    set((state) => {
      if (nodeIds.length === 0) return state;
      return withGraphBucket(state, graphPath, (bucket) => {
        for (const nodeId of nodeIds) {
          if (bucket.nodes[nodeId]) deleteBucketNode(bucket, nodeId);
        }
        return bucket;
      });
    }),

  addPin: (nodeId, pin, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        if (bucket.pins[pin.id]) {
          logger.graph.warn(`Pin "${pin.id}" already exists`, 'GraphDataStore');
          return null;
        }
        bucket.pins[pin.id] = toStoredPin(pin);
        bucket.nodePins[nodeId] = [...(bucket.nodePins[nodeId] ?? []), pin.id];
        bucket.pinConnections[pin.id] = [];
        return bucket;
      }),
    ),

  updatePin: (pinId, patch, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        const prev = bucket.pins[pinId];
        if (!prev) {
          logger.graph.warn(`updatePin: Pin "${pinId}" not found`, 'GraphDataStore');
          return null;
        }
        bucket.pins[pinId] = { ...prev, ...patch };
        return bucket;
      }, 'updatePin'),
    ),

  batchUpdatePinFields: (updates, graphPath) =>
    set((state) => {
      if (updates.length === 0) return state;
      return withGraphBucket(state, graphPath, (bucket) => {
        for (const { pinId, patch } of updates) {
          const prev = bucket.pins[pinId];
          if (!prev) continue;
          bucket.pins[pinId] = { ...prev, ...patch };
        }
        return bucket;
      });
    }),

  deletePin: (pinId, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        const pin = bucket.pins[pinId];
        if (!pin) {
          logger.graph.warn(`deletePin: Pin "${pinId}" not found`, 'GraphDataStore');
          return null;
        }
        for (const connId of bucket.pinConnections[pinId] ?? []) {
          disconnectBucketConnection(bucket, connId);
        }
        delete bucket.pinConnections[pinId];
        delete bucket.pins[pinId];
        bucket.nodePins[pin.nodeId] = (bucket.nodePins[pin.nodeId] ?? []).filter((id) => id !== pinId);
        return bucket;
      }),
    ),

  batchUpdatePins: ({ disconnectIds, removePinIds, updatePins, addPins, graphPath }) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        for (const connId of disconnectIds) disconnectBucketConnection(bucket, connId);
        for (const pinId of removePinIds) {
          const pin = bucket.pins[pinId];
          if (!pin) continue;
          for (const connId of bucket.pinConnections[pinId] ?? []) {
            disconnectBucketConnection(bucket, connId);
          }
          delete bucket.pinConnections[pinId];
          delete bucket.pins[pinId];
          bucket.nodePins[pin.nodeId] = (bucket.nodePins[pin.nodeId] ?? []).filter((id) => id !== pinId);
        }
        for (const { pinId, patch } of updatePins ?? []) {
          const existing = bucket.pins[pinId];
          if (!existing) continue;
          bucket.pins[pinId] = { ...existing, ...patch };
        }
        for (const { nodeId, pin } of addPins) {
          if (!bucket.pins[pin.id]) {
            bucket.pins[pin.id] = toStoredPin(pin);
            bucket.nodePins[nodeId] = [...(bucket.nodePins[nodeId] ?? []), pin.id];
            bucket.pinConnections[pin.id] = bucket.pinConnections[pin.id] ?? [];
          }
        }
        return bucket;
      }),
    ),

  reorderNodePins: (nodeId, pinOrder, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        const current = bucket.nodePins[nodeId];
        if (!current) return null;
        const currentSet = new Set(current);
        const ordered = pinOrder.filter((pid) => currentSet.has(pid));
        if (ordered.length !== current.length) return null;
        bucket.nodePins[nodeId] = ordered;
        return bucket;
      }),
    ),

  connect: (from, to, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        connectBucketPins(bucket, from, to);
        return bucket;
      }),
    ),

  disconnect: (connectionId, graphPath) =>
    set((state) =>
      withGraphBucket(state, graphPath, (bucket) => {
        if (!bucket.connections[connectionId]) {
          logger.graph.warn(`disconnect: Connection "${connectionId}" not found`, 'GraphDataStore');
          return null;
        }
        disconnectBucketConnection(bucket, connectionId);
        return bucket;
      }),
    ),

  batchDisconnect: (connectionIds, graphPath) =>
    set((state) => {
      if (connectionIds.length === 0) return state;
      return withGraphBucket(state, graphPath, (bucket) => {
        for (const connectionId of connectionIds) disconnectBucketConnection(bucket, connectionId);
        return bucket;
      });
    }),

  applyConnectionDraft: (pinA, pinB, graphPath) => {
    const state = get();
    const readPin = (pinId: PinId) => state.getGraphPin(graphPath, pinId);
    const readPinConnections = (pinId: PinId) => state.getGraphPinConnections(graphPath, pinId);
    const readConnection = (connectionId: ConnectionId) => state.getGraphConnection(graphPath, connectionId);

    const dirA = readPin(pinA)?.direction;
    const dirB = readPin(pinB)?.direction;
    if (!dirA || !dirB) return null;

    let from: PinId;
    let to: PinId;
    if (dirA === 'output' && dirB === 'input') {
      from = pinA;
      to = pinB;
    } else if (dirA === 'input' && dirB === 'output') {
      from = pinB;
      to = pinA;
    } else {
      from = pinA;
      to = pinB;
    }

    const fromPin = readPin(from);
    const toPin = readPin(to);
    if (!fromPin || !toPin) return null;

    const connectionId: ConnectionId = `${from}->${to}`;
    const disconnectedIds: ConnectionId[] = [];

    if (toPin.direction === 'input') {
      for (const cid of readPinConnections(to)) disconnectedIds.push(cid);
    }
    if (fromPin.direction === 'output' && isExecPin(fromPin)) {
      for (const cid of readPinConnections(from)) {
        const conn = readConnection(cid);
        if (conn?.from === from && !disconnectedIds.includes(cid)) disconnectedIds.push(cid);
      }
    }

    set((s) =>
      withGraphBucket(s, graphPath, (bucket) => {
        for (const cid of disconnectedIds) disconnectBucketConnection(bucket, cid);
        connectBucketPins(bucket, from, to);
        return bucket;
      }),
    );

    return { from, to, connectionId, disconnectedIds };
  },

  revertConnectionDraft: (draft, graphPath) =>
    set((s) =>
      withGraphBucket(s, graphPath, (bucket) => {
        disconnectBucketConnection(bucket, draft.connectionId);
        for (const cid of draft.disconnectedIds) {
          const parts = cid.split('->');
          if (parts.length !== 2 || bucket.connections[cid]) continue;
          connectBucketPins(bucket, parts[0], parts[1]);
        }
        return bucket;
      }),
    ),

  batchConnect: (pairs, graphPath) =>
    set((s) => {
      if (pairs.length === 0) return s;
      return withGraphBucket(s, graphPath, (bucket) => {
        for (const { from, to } of pairs) connectBucketPins(bucket, from, to);
        return bucket;
      });
    }),

  getGraphNode: (graphPath, nodeId) => get().graphEntities[graphPath]?.nodes[nodeId],

  getGraphPin: (graphPath, pinId) => get().graphEntities[graphPath]?.pins[pinId],

  getGraphNodeIds: (graphPath) => getGraphNodeIds(get(), graphPath),

  getGraphNodePins: (graphPath, nodeId) => get().graphEntities[graphPath]?.nodePins[nodeId] ?? [],

  getGraphPinConnections: (graphPath, pinId) => get().graphEntities[graphPath]?.pinConnections[pinId] ?? [],

  getGraphConnection: (graphPath, connectionId) => getGraphConnection(get(), graphPath, connectionId),

  getGraphConnections: (graphPath) => {
    const bucket = get().graphEntities[graphPath];
    return bucket ? Object.values(bucket.connections) : [];
  },

  hasGraph: (graphPath) => hasGraphData(get(), graphPath),

  clearGraph: (graphPath) =>
    set((state) => {
      if (!state.graphEntities[graphPath]) return state;
      const graphEntities = { ...state.graphEntities };
      delete graphEntities[graphPath];
      return { graphEntities };
    }),

  hydrateGraphs: (graphs) => {
    const graphEntities: Record<GraphPath, GraphEntityBucket> = {};
    Object.entries(graphs).forEach(([graphPath, graph]) => {
      graphEntities[graphPath] = buildGraphBucket(graphPath, graph);
    });
    set({ graphEntities });
  },

  addGraphFromData: (graphPath, graph) => {
    set((state) => {
      const bucket = buildGraphBucket(graphPath, graph);
      return commitGraphBucket(state, graphPath, bucket);
    });
  },
}));
