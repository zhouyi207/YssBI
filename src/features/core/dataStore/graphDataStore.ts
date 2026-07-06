import { create } from 'zustand';
import {
  NodeId,
  PinId,
  GraphId,
  ConnectionId,
  GraphDataLike,
  NodeData,
  PinData,
  ConnectionData,
  RuntimeNodeInput,
} from '@/shared/types';
import { logger } from '@/utils/appLogger';
import { getViewport } from '@/features/core/viewport';
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
  state: { graphEntities: Record<GraphId, GraphEntityBucket> },
  graphId: GraphId,
  bucket: GraphEntityBucket,
) {
  return {
    graphEntities: { ...state.graphEntities, [graphId]: bucket },
  };
}

function withGraphBucket(
  state: { graphEntities: Record<GraphId, GraphEntityBucket> },
  graphId: GraphId,
  mutate: (bucket: GraphEntityBucket) => GraphEntityBucket | null,
  notFoundLabel?: string,
) {
  const current = state.graphEntities[graphId];
  if (!current) {
    if (notFoundLabel) {
      logger.graph.warn(`${notFoundLabel}: graph "${graphId}" not loaded`, 'GraphDataStore');
    }
    return state;
  }
  const bucket = cloneGraphBucket(current);
  const next = mutate(bucket);
  if (!next) return state;
  return commitGraphBucket(state, graphId, next);
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

function toPinIds(arr: unknown): string[] {
  if (!Array.isArray(arr)) return [];
  return arr.map((p) => (typeof p === 'string' ? p : (p as { id?: string })?.id ?? '')).filter(Boolean);
}

function buildGraphBucket(graphId: GraphId, graph: GraphDataLike): GraphEntityBucket {
  const bucket = emptyGraphBucket();
  const conns = Array.isArray(graph.connections)
    ? graph.connections.map((c: { from: string; to: string }) => ({ fromPin: c.from, toPin: c.to }))
    : graph.connections.connections;

  (graph.nodes || []).forEach((node) => {
    const inputIds = toPinIds(node.inputs);
    const outputIds = toPinIds(node.outputs);
    bucket.nodes[node.id] = {
      ...node,
      graphId,
      inputs: inputIds,
      outputs: outputIds,
      nodeType: (node as NodeData).nodeType ?? (node as { nodeType?: string }).nodeType ?? '',
      category: (node as NodeData).category ?? [],
      title: (node as NodeData).title ?? '',
      uiStyle: (node as NodeData).uiStyle ?? 'default',
      position: (node as NodeData).position ?? { x: 0, y: 0 },
    };
    bucket.graphNodes.push(node.id);
    const pinIds = [...inputIds, ...outputIds];
    bucket.nodePins[node.id] = pinIds;
    pinIds.forEach((pinId) => {
      bucket.pinConnections[pinId] = bucket.pinConnections[pinId] ?? [];
    });
  });

  (graph.pins || []).forEach((pin: PinDataInput) => {
    bucket.pins[pin.id] = toStoredPin(pin);
  });

  conns.forEach((connection: { fromPin: string; toPin: string }) => {
    const from = connection.fromPin;
    const to = connection.toPin;
    const id = `${from}->${to}`;
    bucket.connections[id] = { id, from, to };
    bucket.pinConnections[from] = bucket.pinConnections[from] ?? [];
    bucket.pinConnections[from].push(id);
    bucket.pinConnections[to] = bucket.pinConnections[to] ?? [];
    bucket.pinConnections[to].push(id);
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
  graphEntities: Record<GraphId, GraphEntityBucket>;

  addNode(graphId: GraphId, node: NodeData): void;
  updateNode(nodeId: NodeId, patch: Partial<NodeData>, graphId: GraphId): void;
  batchUpdateNodePositions(updates: Array<{ nodeId: NodeId; x: number; y: number }>, graphId: GraphId): void;
  deleteNode(nodeId: NodeId, graphId: GraphId): void;
  batchAddNodesAndPins(graphId: GraphId, items: Array<{ node: NodeData; pins: PinData[] }>): void;
  batchDeleteNodes(nodeIds: NodeId[], graphId: GraphId): void;
  applyNodeDraft(graphId: GraphId, node: NodeData, pins: PinData[]): void;
  revertNodeDraft(nodeId: NodeId, graphId: GraphId): void;
  reconcileNode(graphId: GraphId, node: NodeData, pins: PinData[]): void;

  addPin(nodeId: NodeId, pin: PinData, graphId: GraphId): void;
  updatePin(pinId: PinId, patch: Partial<PinData>, graphId: GraphId): void;
  batchUpdatePinFields(updates: Array<{ pinId: PinId; patch: Partial<PinData> }>, graphId: GraphId): void;
  deletePin(pinId: PinId, graphId: GraphId): void;
  batchUpdatePins(params: {
    disconnectIds: ConnectionId[];
    removePinIds: PinId[];
    updatePins?: Array<{ pinId: PinId; patch: Partial<PinData> }>;
    addPins: Array<{ nodeId: NodeId; pin: PinData }>;
    graphId: GraphId;
  }): void;
  reorderNodePins(nodeId: NodeId, pinOrder: PinId[], graphId: GraphId): void;

  connect(from: PinId, to: PinId, graphId: GraphId): void;
  disconnect(connectionId: ConnectionId, graphId: GraphId): void;
  batchDisconnect(connectionIds: ConnectionId[], graphId: GraphId): void;
  applyConnectionDraft(pinA: PinId, pinB: PinId, graphId: GraphId): ConnectionDraft | null;
  revertConnectionDraft(draft: ConnectionDraft, graphId: GraphId): void;
  batchConnect(pairs: Array<{ from: PinId; to: PinId }>, graphId: GraphId): void;

  getGraphNode(graphId: GraphId, nodeId: NodeId): NodeData | undefined;
  getGraphPin(graphId: GraphId, pinId: PinId): PinData | undefined;
  getGraphNodeIds(graphId: GraphId): NodeId[];
  getGraphNodePins(graphId: GraphId, nodeId: NodeId): PinId[];
  getGraphPinConnections(graphId: GraphId, pinId: PinId): ConnectionId[];
  getGraphConnection(graphId: GraphId, connectionId: ConnectionId): ConnectionData | undefined;
  getGraphConnections(graphId: GraphId): ConnectionData[];
  hasGraph(graphId: GraphId): boolean;
  clearGraph(graphId: GraphId): void;
  hydrateGraphs(graphs: Record<GraphId, GraphDataLike>): void;
  addGraphFromData(graphId: GraphId, graph: GraphDataLike): void;
  replaceGraphNodes(graphId: GraphId, nodes: RuntimeNodeInput[]): void;
}

export const useGraphDataStore = create<GraphDataStore>((set, get) => ({
  graphEntities: {},

  addNode: (graphId, node) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
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

  updateNode: (nodeId, patch, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
        const prev = bucket.nodes[nodeId];
        if (!prev) {
          logger.graph.warn(`updateNode: Node "${nodeId}" not found`, 'GraphDataStore');
          return null;
        }
        bucket.nodes[nodeId] = { ...prev, ...patch };
        return bucket;
      }, 'updateNode'),
    ),

  batchUpdateNodePositions: (updates, graphId) =>
    set((state) => {
      if (updates.length === 0) return state;
      return withGraphBucket(state, graphId, (bucket) => {
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

  deleteNode: (nodeId, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
        if (!bucket.nodes[nodeId]) {
          logger.graph.warn(`deleteNode: Node "${nodeId}" not found`, 'GraphDataStore');
          return null;
        }
        deleteBucketNode(bucket, nodeId);
        return bucket;
      }),
    ),

  batchAddNodesAndPins: (graphId, items) =>
    set((state) => {
      if (items.length === 0) return state;
      return withGraphBucket(state, graphId, (bucket) => {
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

  applyNodeDraft: (graphId, node, pins) => get().batchAddNodesAndPins(graphId, [{ node, pins }]),

  revertNodeDraft: (nodeId, graphId) => get().deleteNode(nodeId, graphId),

  reconcileNode: (graphId, node, pins) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
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

  batchDeleteNodes: (nodeIds, graphId) =>
    set((state) => {
      if (nodeIds.length === 0) return state;
      return withGraphBucket(state, graphId, (bucket) => {
        for (const nodeId of nodeIds) {
          if (bucket.nodes[nodeId]) deleteBucketNode(bucket, nodeId);
        }
        return bucket;
      });
    }),

  addPin: (nodeId, pin, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
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

  updatePin: (pinId, patch, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
        const prev = bucket.pins[pinId];
        if (!prev) {
          logger.graph.warn(`updatePin: Pin "${pinId}" not found`, 'GraphDataStore');
          return null;
        }
        bucket.pins[pinId] = { ...prev, ...patch };
        return bucket;
      }, 'updatePin'),
    ),

  batchUpdatePinFields: (updates, graphId) =>
    set((state) => {
      if (updates.length === 0) return state;
      return withGraphBucket(state, graphId, (bucket) => {
        for (const { pinId, patch } of updates) {
          const prev = bucket.pins[pinId];
          if (!prev) continue;
          bucket.pins[pinId] = { ...prev, ...patch };
        }
        return bucket;
      });
    }),

  deletePin: (pinId, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
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

  batchUpdatePins: ({ disconnectIds, removePinIds, updatePins, addPins, graphId }) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
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

  reorderNodePins: (nodeId, pinOrder, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
        const current = bucket.nodePins[nodeId];
        if (!current) return null;
        const currentSet = new Set(current);
        const ordered = pinOrder.filter((pid) => currentSet.has(pid));
        if (ordered.length !== current.length) return null;
        bucket.nodePins[nodeId] = ordered;
        return bucket;
      }),
    ),

  connect: (from, to, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
        connectBucketPins(bucket, from, to);
        return bucket;
      }),
    ),

  disconnect: (connectionId, graphId) =>
    set((state) =>
      withGraphBucket(state, graphId, (bucket) => {
        if (!bucket.connections[connectionId]) {
          logger.graph.warn(`disconnect: Connection "${connectionId}" not found`, 'GraphDataStore');
          return null;
        }
        disconnectBucketConnection(bucket, connectionId);
        return bucket;
      }),
    ),

  batchDisconnect: (connectionIds, graphId) =>
    set((state) => {
      if (connectionIds.length === 0) return state;
      return withGraphBucket(state, graphId, (bucket) => {
        for (const connectionId of connectionIds) disconnectBucketConnection(bucket, connectionId);
        return bucket;
      });
    }),

  applyConnectionDraft: (pinA, pinB, graphId) => {
    const state = get();
    const readPin = (pinId: PinId) => state.getGraphPin(graphId, pinId);
    const readPinConnections = (pinId: PinId) => state.getGraphPinConnections(graphId, pinId);
    const readConnection = (connectionId: ConnectionId) => state.getGraphConnection(graphId, connectionId);

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
    if (fromPin.direction === 'output' && fromPin.type === 'exec') {
      for (const cid of readPinConnections(from)) {
        const conn = readConnection(cid);
        if (conn?.from === from && !disconnectedIds.includes(cid)) disconnectedIds.push(cid);
      }
    }

    set((s) =>
      withGraphBucket(s, graphId, (bucket) => {
        for (const cid of disconnectedIds) disconnectBucketConnection(bucket, cid);
        connectBucketPins(bucket, from, to);
        return bucket;
      }),
    );

    return { from, to, connectionId, disconnectedIds };
  },

  revertConnectionDraft: (draft, graphId) =>
    set((s) =>
      withGraphBucket(s, graphId, (bucket) => {
        disconnectBucketConnection(bucket, draft.connectionId);
        for (const cid of draft.disconnectedIds) {
          const parts = cid.split('->');
          if (parts.length !== 2 || bucket.connections[cid]) continue;
          connectBucketPins(bucket, parts[0], parts[1]);
        }
        return bucket;
      }),
    ),

  batchConnect: (pairs, graphId) =>
    set((s) => {
      if (pairs.length === 0) return s;
      return withGraphBucket(s, graphId, (bucket) => {
        for (const { from, to } of pairs) connectBucketPins(bucket, from, to);
        return bucket;
      });
    }),

  getGraphNode: (graphId, nodeId) => get().graphEntities[graphId]?.nodes[nodeId],

  getGraphPin: (graphId, pinId) => get().graphEntities[graphId]?.pins[pinId],

  getGraphNodeIds: (graphId) => getGraphNodeIds(get(), graphId),

  getGraphNodePins: (graphId, nodeId) => get().graphEntities[graphId]?.nodePins[nodeId] ?? [],

  getGraphPinConnections: (graphId, pinId) => get().graphEntities[graphId]?.pinConnections[pinId] ?? [],

  getGraphConnection: (graphId, connectionId) => getGraphConnection(get(), graphId, connectionId),

  getGraphConnections: (graphId) => {
    const bucket = get().graphEntities[graphId];
    return bucket ? Object.values(bucket.connections) : [];
  },

  hasGraph: (graphId) => hasGraphData(get(), graphId),

  clearGraph: (graphId) =>
    set((state) => {
      if (!state.graphEntities[graphId]) return state;
      const graphEntities = { ...state.graphEntities };
      delete graphEntities[graphId];
      return { graphEntities };
    }),

  hydrateGraphs: (graphs) => {
    const graphEntities: Record<GraphId, GraphEntityBucket> = {};
    Object.values(graphs).forEach((graph) => {
      graphEntities[graph.id] = buildGraphBucket(graph.id, graph);
    });
    set({ graphEntities });
  },

  addGraphFromData: (graphId, graph) => {
    set((state) => {
      const bucket = buildGraphBucket(graphId, graph);
      return commitGraphBucket(state, graphId, bucket);
    });
  },

  replaceGraphNodes: (graphId, nodes) => {
    const state = get();
    const pins: PinData[] = [];
    const connectionItems: { fromPin: string; toPin: string }[] = [];
    const toPinId = (p: string | PinDataInput): string => (typeof p === 'string' ? p : (p?.id ?? ''));

    const nodeData = nodes.map((n: RuntimeNodeInput) => {
      const inputIds = (n.inputs || []).map(toPinId).filter(Boolean);
      const outputIds = (n.outputs || []).map(toPinId).filter(Boolean);

      [...(n.inputs || []), ...(n.outputs || [])].forEach((p) => {
        const pin = typeof p === 'object' && p?.id ? p : state.getGraphPin(graphId, toPinId(p));
        if (pin && !pins.some((x) => x.id === (pin.id ?? toPinId(p)))) {
          pins.push(pin);
        }
      });

      (n.outputs || []).forEach((p) => {
        const pinId = toPinId(p);
        const links = state.getGraphPinConnections(graphId, pinId).map((cid) => {
          const conn = state.getGraphConnection(graphId, cid);
          return conn?.from === pinId ? conn?.to : conn?.from;
        }).filter(Boolean);
        links.forEach((toId) => connectionItems.push({ fromPin: pinId, toPin: toId }));
      });

      return { ...n, inputs: inputIds, outputs: outputIds };
    });

    get().clearGraph(graphId);
    get().addGraphFromData(graphId, {
      id: graphId,
      name: '',
      type: 'event',
      canvas: getViewport(graphId),
      nodes: nodeData,
      pins,
      connections: { connections: connectionItems },
    });
  },
}));
