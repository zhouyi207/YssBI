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

type PinDataInput = PinData & { links?: string[] };

interface GraphEntityBucket {
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;
  graphNodes: NodeId[];
  nodePins: Record<NodeId, PinId[]>;
  pinConnections: Record<PinId, ConnectionId[]>;
}

interface GraphEntityMirror {
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;
  graphNodes: Record<GraphId, NodeId[]>;
  nodePins: Record<NodeId, PinId[]>;
  pinConnections: Record<PinId, ConnectionId[]>;
}

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

function flattenGraphBuckets(graphEntities: Record<GraphId, GraphEntityBucket>) {
  const nodes: Record<NodeId, NodeData> = {};
  const pins: Record<PinId, PinData> = {};
  const connections: Record<ConnectionId, ConnectionData> = {};
  const graphNodes: Record<GraphId, NodeId[]> = {};
  const nodePins: Record<NodeId, PinId[]> = {};
  const pinConnections: Record<PinId, ConnectionId[]> = {};

  for (const [graphId, bucket] of Object.entries(graphEntities)) {
    graphNodes[graphId] = bucket.graphNodes;
    Object.assign(nodes, bucket.nodes);
    Object.assign(pins, bucket.pins);
    Object.assign(connections, bucket.connections);
    Object.assign(nodePins, bucket.nodePins);
    Object.assign(pinConnections, bucket.pinConnections);
  }

  return { nodes, pins, connections, graphNodes, nodePins, pinConnections };
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
  state: GraphEntityMirror & { graphEntities: Record<GraphId, GraphEntityBucket> },
  graphId: GraphId,
  bucket: GraphEntityBucket,
) {
  const previousBucket = state.graphEntities[graphId];
  const graphEntities = { ...state.graphEntities, [graphId]: bucket };
  return {
    graphEntities,
    ...patchFlatMirrorForGraphBucket(state, graphEntities, graphId, previousBucket, bucket),
  };
}

function patchFlatMirrorForGraphBucket(
  state: GraphEntityMirror,
  graphEntities: Record<GraphId, GraphEntityBucket>,
  graphId: GraphId,
  previousBucket: GraphEntityBucket | undefined,
  nextBucket: GraphEntityBucket | undefined,
): GraphEntityMirror {
  const nextNodes = { ...state.nodes };
  const nextPins = { ...state.pins };
  const nextConnections = { ...state.connections };
  const nextGraphNodes = { ...state.graphNodes };
  const nextNodePins = { ...state.nodePins };
  const nextPinConnections = { ...state.pinConnections };

  if (nextBucket) nextGraphNodes[graphId] = nextBucket.graphNodes;
  else delete nextGraphNodes[graphId];

  patchRecordMirror(nextNodes, previousBucket?.nodes, nextBucket?.nodes, graphEntities, (bucket) => bucket.nodes);
  patchRecordMirror(nextPins, previousBucket?.pins, nextBucket?.pins, graphEntities, (bucket) => bucket.pins);
  patchRecordMirror(
    nextConnections,
    previousBucket?.connections,
    nextBucket?.connections,
    graphEntities,
    (bucket) => bucket.connections,
  );
  patchRecordMirror(nextNodePins, previousBucket?.nodePins, nextBucket?.nodePins, graphEntities, (bucket) => bucket.nodePins);
  patchRecordMirror(
    nextPinConnections,
    previousBucket?.pinConnections,
    nextBucket?.pinConnections,
    graphEntities,
    (bucket) => bucket.pinConnections,
  );

  return {
    nodes: nextNodes,
    pins: nextPins,
    connections: nextConnections,
    graphNodes: nextGraphNodes,
    nodePins: nextNodePins,
    pinConnections: nextPinConnections,
  };
}

function patchRecordMirror<T>(
  target: Record<string, T>,
  previousRecord: Record<string, T> | undefined,
  nextRecord: Record<string, T> | undefined,
  graphEntities: Record<GraphId, GraphEntityBucket>,
  selectRecord: (bucket: GraphEntityBucket) => Record<string, T>,
): void {
  const affectedKeys = new Set<string>([
    ...Object.keys(previousRecord ?? {}),
    ...Object.keys(nextRecord ?? {}),
  ]);

  for (const key of affectedKeys) {
    if (nextRecord && key in nextRecord) {
      target[key] = nextRecord[key];
      continue;
    }

    const replacement = findMirrorReplacement(key, graphEntities, selectRecord);
    if (replacement) target[key] = replacement.value;
    else delete target[key];
  }
}

function findMirrorReplacement<T>(
  key: string,
  graphEntities: Record<GraphId, GraphEntityBucket>,
  selectRecord: (bucket: GraphEntityBucket) => Record<string, T>,
): { value: T } | null {
  for (const bucket of Object.values(graphEntities)) {
    const record = selectRecord(bucket);
    if (key in record) return { value: record[key] };
  }
  return null;
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
  // ======================
  // 实体表
  // ======================
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;
  graphEntities: Record<GraphId, GraphEntityBucket>;

  // ======================
  // 索引表
  // ======================
  graphNodes: Record<GraphId, NodeId[]>;
  nodePins: Record<NodeId, PinId[]>;
  pinConnections: Record<PinId, ConnectionId[]>;

  // ======================
  // Node
  // ======================
  addNode(graphId: GraphId, node: NodeData): void;
  updateNode(nodeId: NodeId, patch: Partial<NodeData>, graphId: GraphId): void;
  /** 批量更新节点位置（拖拽时使用，避免 replaceGraphNodes 的 O(n) 清空+重建） */
  batchUpdateNodePositions(updates: Array<{ nodeId: NodeId; x: number; y: number }>, graphId: GraphId): void;
  deleteNode(nodeId: NodeId, graphId: GraphId): void;
  /** 批量添加节点和它们的 pin（单次 set，避免 N 次 re-render） */
  batchAddNodesAndPins(graphId: GraphId, items: Array<{ node: NodeData; pins: PinData[] }>): void;
  /** 批量删除节点（单次 set） */
  batchDeleteNodes(nodeIds: NodeId[], graphId: GraphId): void;
  /**
   * 乐观节点草稿：用客户端生成的 id 立即插入节点及其初始 pin，先于后端往返渲染。
   * 后端权威数据通过 NodeCreated 事件回传后由 handler 对齐（id 一致，无重复）。
   */
  applyNodeDraft(graphId: GraphId, node: NodeData, pins: PinData[]): void;
  /** 回滚 applyNodeDraft（后端创建失败时） */
  revertNodeDraft(nodeId: NodeId, graphId: GraphId): void;
  /**
   * 用后端权威数据覆盖已乐观插入的节点（id 一致）：更新节点字段、按 id 更新/补齐
   * pin、并按权威顺序重排，使乐观渲染最终与后端一致。
   */
  reconcileNode(graphId: GraphId, node: NodeData, pins: PinData[]): void;

  // ======================
  // Pin
  // ======================
  addPin(nodeId: NodeId, pin: PinData, graphId: GraphId): void;
  updatePin(pinId: PinId, patch: Partial<PinData>, graphId: GraphId): void;
  /** 批量更新 pin 字段（单次 set，避免 N 次 re-render） */
  batchUpdatePinFields(updates: Array<{ pinId: PinId; patch: Partial<PinData> }>, graphId: GraphId): void;
  deletePin(pinId: PinId, graphId: GraphId): void;
  /** 批量更新 pin（断连 + 删 pin + 更新 pin + 加 pin，单次 set） */
  batchUpdatePins(params: {
    disconnectIds: ConnectionId[];
    removePinIds: PinId[];
    updatePins?: Array<{ pinId: PinId; patch: Partial<PinData> }>;
    addPins: Array<{ nodeId: NodeId; pin: PinData }>;
    graphId: GraphId;
  }): void;
  /** 按后端提供的顺序重排节点的 pin 列表 */
  reorderNodePins(nodeId: NodeId, pinOrder: PinId[], graphId: GraphId): void;

  // ======================
  // Connection
  // ======================
  connect(from: PinId, to: PinId, graphId: GraphId): void;
  disconnect(connectionId: ConnectionId, graphId: GraphId): void;
  /** 批量断开连接（单次 set） */
  batchDisconnect(connectionIds: ConnectionId[], graphId: GraphId): void;
  /**
   * 乐观连接草稿：单次 set 内解析方向、断开冲突连接（input 单入、exec output 单出）
   * 并建立新连接。仅用于本地即时预览，后端仍是权威；返回值供失败回滚。
   */
  applyConnectionDraft(pinA: PinId, pinB: PinId, graphId: GraphId): ConnectionDraft | null;
  /** 回滚 applyConnectionDraft（后端连接失败时） */
  revertConnectionDraft(draft: ConnectionDraft, graphId: GraphId): void;
  /** 批量建立连接（粘贴/恢复，单次 set，避免逐条 re-render） */
  batchConnect(pairs: Array<{ from: PinId; to: PinId }>, graphId: GraphId): void;

  // ======================
  // Graph
  // ======================
  getGraphNode(graphId: GraphId, nodeId: NodeId): NodeData | undefined;
  getGraphPin(graphId: GraphId, pinId: PinId): PinData | undefined;
  getGraphNodePins(graphId: GraphId, nodeId: NodeId): PinId[];
  getGraphPinConnections(graphId: GraphId, pinId: PinId): ConnectionId[];
  getGraphConnections(graphId: GraphId): ConnectionData[];
  clearGraph(graphId: GraphId): void;
  hydrateGraphs(graphs: Record<GraphId, GraphDataLike>): void;
  addGraphFromData(graphId: GraphId, graph: GraphDataLike): void;
  /** 用新的 nodes 替换图中节点（用于 setNodes 等批量更新） */
  replaceGraphNodes(graphId: GraphId, nodes: RuntimeNodeInput[]): void;
}

export const useGraphDataStore = create<GraphDataStore>((set, get) => ({
  // ======================================================
  // State
  // ======================================================
  nodes: {},
  pins: {},
  connections: {},
  graphEntities: {},

  graphNodes: {},
  nodePins: {},
  pinConnections: {},

  // ======================================================
  // Node
  // ======================================================
  addNode: (graphId, node) =>
    set((state) => {
      if (state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        if (bucket.nodes[node.id]) {
          logger.graph.warn(`Node "${node.id}" already exists`, 'GraphDataStore');
          return state;
        }
        bucket.nodes[node.id] = node;
        bucket.graphNodes = [...bucket.graphNodes, node.id];
        bucket.nodePins[node.id] = [];
        return commitGraphBucket(state, graphId, bucket);
      }

      if (state.nodes[node.id]) {
        logger.graph.warn(`Node "${node.id}" already exists`, 'GraphDataStore');
        return state;
      }

      return {
        nodes: {
          ...state.nodes,
          [node.id]: node,
        },
        graphNodes: {
          ...state.graphNodes,
          [graphId]: [...(state.graphNodes[graphId] ?? []), node.id],
        },
        nodePins: {
          ...state.nodePins,
          [node.id]: [],
        },
      };
    }),

  updateNode: (nodeId, patch, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        const prev = bucket.nodes[nodeId];
        if (!prev) {
          logger.graph.warn(`updateNode: Node "${nodeId}" not found`, 'GraphDataStore');
          return state;
        }
        bucket.nodes[nodeId] = { ...prev, ...patch };
        return commitGraphBucket(state, graphId, bucket);
      }

      const prev = state.nodes[nodeId];
      if (!prev) {
        logger.graph.warn(`updateNode: Node "${nodeId}" not found`, 'GraphDataStore');
        return state;
      }

      return {
        nodes: {
          ...state.nodes,
          [nodeId]: {
            ...prev,
            ...patch,
          },
        },
      };
    }),

  batchUpdateNodePositions: (updates, graphId) =>
    set((state) => {
      if (updates.length === 0) return state;
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        let changed = false;
        for (const { nodeId, x, y } of updates) {
          const prev = bucket.nodes[nodeId];
          if (prev?.position && (prev.position.x !== x || prev.position.y !== y)) {
            bucket.nodes[nodeId] = { ...prev, position: { x, y } };
            changed = true;
          }
        }
        return changed ? commitGraphBucket(state, graphId, bucket) : state;
      }

      const nextNodes = { ...state.nodes };
      let changed = false;
      for (const { nodeId, x, y } of updates) {
        const prev = nextNodes[nodeId];
        if (prev?.position && (prev.position.x !== x || prev.position.y !== y)) {
          nextNodes[nodeId] = { ...prev, position: { x, y } };
          changed = true;
        }
      }
      return changed ? { nodes: nextNodes } : state;
    }),

  deleteNode: (nodeId, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        if (!bucket.nodes[nodeId]) return state;
        deleteBucketNode(bucket, nodeId);
        return commitGraphBucket(state, graphId, bucket);
      }

      const node = state.nodes[nodeId];
      if (!node) {
        logger.graph.warn(`deleteNode: Node "${nodeId}" not found`, 'GraphDataStore');
        return state;
      }

      const nextNodes = { ...state.nodes };
      const nextNodePins = { ...state.nodePins };
      const nextPins = { ...state.pins };
      const nextPinConnections = { ...state.pinConnections };
      const nextConnections = { ...state.connections };

      // 1️⃣ 删除 node 下的所有 pin（以及 pin 的 connection）
      const pinIds = state.nodePins[nodeId] ?? [];
      for (const pinId of pinIds) {
        const connIds = state.pinConnections[pinId] ?? [];
        for (const connId of connIds) {
          const conn = state.connections[connId];
          if (!conn) continue;

          // 从另一端 pin 中移除 connection
          const otherPin =
            conn.from === pinId ? conn.to : conn.from;

          nextPinConnections[otherPin] =
            (nextPinConnections[otherPin] ?? []).filter(
              (id) => id !== connId
            );

          delete nextConnections[connId];
        }

        delete nextPinConnections[pinId];
        delete nextPins[pinId];
      }

      delete nextNodePins[nodeId];
      delete nextNodes[nodeId];

      // 2️⃣ 从 graphNodes 中移除
      const nodeGraphId = node.graphId;
      const graphNodeIds = state.graphNodes[nodeGraphId] ?? [];
      const nextGraphNodes = {
        ...state.graphNodes,
        [nodeGraphId]: graphNodeIds.filter((id) => id !== nodeId),
      };

      return {
        nodes: nextNodes,
        pins: nextPins,
        connections: nextConnections,
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
        graphNodes: nextGraphNodes,
      };
    }),

  batchAddNodesAndPins: (graphId, items) =>
    set((state) => {
      if (items.length === 0) return state;
      if (state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
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
        return commitGraphBucket(state, graphId, bucket);
      }

      const nextNodes = { ...state.nodes };
      const nextPins = { ...state.pins };
      const nextNodePins = { ...state.nodePins };
      const graphNodeIds = [...(state.graphNodes[graphId] ?? [])];

      for (const { node, pins } of items) {
        if (nextNodes[node.id]) continue;
        nextNodes[node.id] = node;
        graphNodeIds.push(node.id);

        const pinIds: PinId[] = [];
        for (const pin of pins) {
          if (!nextPins[pin.id]) {
            nextPins[pin.id] = toStoredPin(pin);
            pinIds.push(pin.id);
          }
        }
        nextNodePins[node.id] = pinIds;
      }

      return {
        nodes: nextNodes,
        pins: nextPins,
        nodePins: nextNodePins,
        graphNodes: { ...state.graphNodes, [graphId]: graphNodeIds },
      };
    }),

  applyNodeDraft: (graphId, node, pins) =>
    get().batchAddNodesAndPins(graphId, [{ node, pins }]),

  revertNodeDraft: (nodeId, graphId) => get().deleteNode(nodeId, graphId),

  reconcileNode: (graphId, node, pins) =>
    set((state) => {
      if (state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
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
        return commitGraphBucket(state, graphId, bucket);
      }

      const existing = state.nodes[node.id];
      // 节点尚未乐观插入（如来自其它来源）：走普通添加路径。
      if (!existing) {
        const nextPins = { ...state.pins };
        const pinIds: PinId[] = [];
        for (const pin of pins) {
          nextPins[pin.id] = toStoredPin(pin);
          pinIds.push(pin.id);
        }
        const nextPinConnections = { ...state.pinConnections };
        for (const pin of pins) {
          if (!nextPinConnections[pin.id]) nextPinConnections[pin.id] = [];
        }
        return {
          nodes: { ...state.nodes, [node.id]: node },
          pins: nextPins,
          nodePins: { ...state.nodePins, [node.id]: pinIds },
          pinConnections: nextPinConnections,
          graphNodes: {
            ...state.graphNodes,
            [graphId]: [...(state.graphNodes[graphId] ?? []), node.id],
          },
        };
      }

      // 已乐观插入：用权威字段覆盖，保留既有连接索引。
      const nextNodes = { ...state.nodes, [node.id]: { ...existing, ...node } };
      const nextPins = { ...state.pins };
      const nextPinConnections = { ...state.pinConnections };
      const existingPinIds = state.nodePins[node.id] ?? [];

      for (const pin of pins) {
        const prev = nextPins[pin.id];
        nextPins[pin.id] = prev ? { ...prev, ...pin } : pin;
        if (!nextPinConnections[pin.id]) nextPinConnections[pin.id] = [];
      }
      // 移除权威集合中不存在的乐观 pin（孤立新建一般不会发生）
      const authoritativeIds = new Set(pins.map((p) => p.id));
      for (const pid of existingPinIds) {
        if (!authoritativeIds.has(pid)) {
          delete nextPins[pid];
          delete nextPinConnections[pid];
        }
      }

      return {
        nodes: nextNodes,
        pins: nextPins,
        nodePins: { ...state.nodePins, [node.id]: pins.map((p) => p.id) },
        pinConnections: nextPinConnections,
      };
    }),

  batchDeleteNodes: (nodeIds, graphId) =>
    set((state) => {
      if (nodeIds.length === 0) return state;
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        for (const nodeId of nodeIds) {
          if (bucket.nodes[nodeId]) deleteBucketNode(bucket, nodeId);
        }
        return commitGraphBucket(state, graphId, bucket);
      }

      const nextNodes = { ...state.nodes };
      const nextNodePins = { ...state.nodePins };
      const nextPins = { ...state.pins };
      const nextPinConnections = { ...state.pinConnections };
      const nextConnections = { ...state.connections };
      const nextGraphNodes = { ...state.graphNodes };

      for (const nodeId of nodeIds) {
        const node = nextNodes[nodeId];
        if (!node) continue;

        const pinIds = nextNodePins[nodeId] ?? [];
        for (const pinId of pinIds) {
          const connIds = nextPinConnections[pinId] ?? [];
          for (const connId of connIds) {
            const conn = nextConnections[connId];
            if (!conn) continue;
            const otherPin = conn.from === pinId ? conn.to : conn.from;
            nextPinConnections[otherPin] =
              (nextPinConnections[otherPin] ?? []).filter((id) => id !== connId);
            delete nextConnections[connId];
          }
          delete nextPinConnections[pinId];
          delete nextPins[pinId];
        }

        delete nextNodePins[nodeId];
        delete nextNodes[nodeId];

        const graphId = node.graphId;
        if (nextGraphNodes[graphId]) {
          nextGraphNodes[graphId] = nextGraphNodes[graphId].filter((id) => id !== nodeId);
        }
      }

      return {
        nodes: nextNodes,
        pins: nextPins,
        connections: nextConnections,
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
        graphNodes: nextGraphNodes,
      };
    }),

  // ======================================================
  // Pin
  // ======================================================
  addPin: (nodeId, pin, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        if (bucket.pins[pin.id]) {
          logger.graph.warn(`Pin "${pin.id}" already exists`, 'GraphDataStore');
          return state;
        }
        bucket.pins[pin.id] = toStoredPin(pin);
        bucket.nodePins[nodeId] = [...(bucket.nodePins[nodeId] ?? []), pin.id];
        bucket.pinConnections[pin.id] = [];
        return commitGraphBucket(state, graphId, bucket);
      }

      if (state.pins[pin.id]) {
        logger.graph.warn(`Pin "${pin.id}" already exists`, 'GraphDataStore');
        return state;
      }

      return {
        pins: {
          ...state.pins,
          [pin.id]: toStoredPin(pin),
        },
        nodePins: {
          ...state.nodePins,
          [nodeId]: [...(state.nodePins[nodeId] ?? []), pin.id],
        },
        pinConnections: {
          ...state.pinConnections,
          [pin.id]: [],
        },
      };
    }),

  updatePin: (pinId, patch, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        const prev = bucket.pins[pinId];
        if (!prev) {
          logger.graph.warn(`updatePin: Pin "${pinId}" not found`, 'GraphDataStore');
          return state;
        }
        bucket.pins[pinId] = { ...prev, ...patch };
        return commitGraphBucket(state, graphId, bucket);
      }

      const prev = state.pins[pinId];
      if (!prev) {
        logger.graph.warn(`updatePin: Pin "${pinId}" not found`, 'GraphDataStore');
        return state;
      }

      return {
        pins: {
          ...state.pins,
          [pinId]: {
            ...prev,
            ...patch,
          },
        },
      };
    }),

  batchUpdatePinFields: (updates, graphId) =>
    set((state) => {
      if (updates.length === 0) return state;
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        for (const { pinId, patch } of updates) {
          const prev = bucket.pins[pinId];
          if (!prev) continue;
          bucket.pins[pinId] = { ...prev, ...patch };
        }
        return commitGraphBucket(state, graphId, bucket);
      }

      const nextPins = { ...state.pins };
      for (const { pinId, patch } of updates) {
        const prev = nextPins[pinId];
        if (!prev) continue;
        nextPins[pinId] = { ...prev, ...patch };
      }
      return { pins: nextPins };
    }),

  deletePin: (pinId, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        const pin = bucket.pins[pinId];
        if (!pin) {
          logger.graph.warn(`deletePin: Pin "${pinId}" not found`, 'GraphDataStore');
          return state;
        }
        for (const connId of bucket.pinConnections[pinId] ?? []) {
          disconnectBucketConnection(bucket, connId);
        }
        delete bucket.pinConnections[pinId];
        delete bucket.pins[pinId];
        bucket.nodePins[pin.nodeId] = (bucket.nodePins[pin.nodeId] ?? []).filter((id) => id !== pinId);
        return commitGraphBucket(state, graphId, bucket);
      }

      const pin = state.pins[pinId];
      if (!pin) {
        logger.graph.warn(`deletePin: Pin "${pinId}" not found`, 'GraphDataStore');
        return state;
      }

      const nextPins = { ...state.pins };
      const nextNodePins = { ...state.nodePins };
      const nextConnections = { ...state.connections };
      const nextPinConnections = { ...state.pinConnections };

      // 删除该 pin 的所有 connection
      const connIds = state.pinConnections[pinId] ?? [];
      for (const connId of connIds) {
        const conn = state.connections[connId];
        if (!conn) continue;

        const otherPin =
          conn.from === pinId ? conn.to : conn.from;

        nextPinConnections[otherPin] =
          (nextPinConnections[otherPin] ?? []).filter(
            (id) => id !== connId
          );

        delete nextConnections[connId];
      }

      delete nextPinConnections[pinId];
      delete nextPins[pinId];

      // 从 nodePins 中移除
      nextNodePins[pin.nodeId] =
        (nextNodePins[pin.nodeId] ?? []).filter((id) => id !== pinId);

      return {
        pins: nextPins,
        connections: nextConnections,
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
      };
    }),

  batchUpdatePins: ({ disconnectIds, removePinIds, updatePins, addPins, graphId }) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
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
            if (!bucket.pinConnections[pin.id]) {
              bucket.pinConnections[pin.id] = [];
            }
          }
        }
        return commitGraphBucket(state, graphId, bucket);
      }

      const nextPins = { ...state.pins };
      const nextConnections = { ...state.connections };
      const nextNodePins = { ...state.nodePins };
      const nextPinConnections = { ...state.pinConnections };

      // 1. 断开连接
      for (const connId of disconnectIds) {
        const conn = nextConnections[connId];
        if (!conn) continue;
        nextPinConnections[conn.from] =
          (nextPinConnections[conn.from] ?? []).filter((id) => id !== connId);
        nextPinConnections[conn.to] =
          (nextPinConnections[conn.to] ?? []).filter((id) => id !== connId);
        delete nextConnections[connId];
      }

      // 2. 删除 pin
      for (const pinId of removePinIds) {
        const pin = nextPins[pinId];
        if (!pin) continue;
        const connIds = nextPinConnections[pinId] ?? [];
        for (const connId of connIds) {
          const conn = nextConnections[connId];
          if (!conn) continue;
          const otherPin = conn.from === pinId ? conn.to : conn.from;
          nextPinConnections[otherPin] =
            (nextPinConnections[otherPin] ?? []).filter((id) => id !== connId);
          delete nextConnections[connId];
        }
        delete nextPinConnections[pinId];
        delete nextPins[pinId];
        nextNodePins[pin.nodeId] =
          (nextNodePins[pin.nodeId] ?? []).filter((id) => id !== pinId);
      }

      // 3. 更新已有 pin（如 repeatable 重索引后的名称）
      for (const { pinId, patch } of updatePins ?? []) {
        const existing = nextPins[pinId];
        if (!existing) continue;
        nextPins[pinId] = { ...existing, ...patch };
      }

      // 4. 添加新 pin
      for (const { nodeId, pin } of addPins) {
        if (!nextPins[pin.id]) {
          nextPins[pin.id] = toStoredPin(pin);
          nextNodePins[nodeId] = [...(nextNodePins[nodeId] ?? []), pin.id];
          if (!nextPinConnections[pin.id]) {
            nextPinConnections[pin.id] = [];
          }
        }
      }

      return {
        pins: nextPins,
        connections: nextConnections,
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
      };
    }),

  reorderNodePins: (nodeId, pinOrder, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        const current = bucket.nodePins[nodeId];
        if (!current) return state;
        const currentSet = new Set(current);
        const ordered = pinOrder.filter((pid) => currentSet.has(pid));
        if (ordered.length !== current.length) return state;
        bucket.nodePins[nodeId] = ordered;
        return commitGraphBucket(state, graphId, bucket);
      }

      const current = state.nodePins[nodeId];
      if (!current) return state;
      const currentSet = new Set(current);
      const ordered = pinOrder.filter((pid) => currentSet.has(pid));
      if (ordered.length !== current.length) return state;
      return {
        nodePins: { ...state.nodePins, [nodeId]: ordered },
      };
    }),

  // ======================================================
  // Connection
  // ======================================================
  connect: (from, to, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        connectBucketPins(bucket, from, to);
        return commitGraphBucket(state, graphId, bucket);
      }

      const id: ConnectionId = `${from}->${to}`;

      if (state.connections[id]) {
        return state;
      }

      const conn: ConnectionData = { id, from, to };

      return {
        connections: {
          ...state.connections,
          [id]: conn,
        },
        pinConnections: {
          ...state.pinConnections,
          [from]: [...(state.pinConnections[from] ?? []), id],
          [to]: [...(state.pinConnections[to] ?? []), id],
        },
      };
    }),

  disconnect: (connectionId, graphId) =>
    set((state) => {
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        if (!bucket.connections[connectionId]) {
          logger.graph.warn(`disconnect: Connection "${connectionId}" not found`, 'GraphDataStore');
          return state;
        }
        disconnectBucketConnection(bucket, connectionId);
        return commitGraphBucket(state, graphId, bucket);
      }

      const conn = state.connections[connectionId];
      if (!conn) {
        logger.graph.warn(`disconnect: Connection "${connectionId}" not found`, 'GraphDataStore');
        return state;
      }

      const nextConnections = { ...state.connections };
      const nextPinConnections = { ...state.pinConnections };

      nextPinConnections[conn.from] =
        (nextPinConnections[conn.from] ?? []).filter(
          (id) => id !== connectionId
        );

      nextPinConnections[conn.to] =
        (nextPinConnections[conn.to] ?? []).filter(
          (id) => id !== connectionId
        );

      delete nextConnections[connectionId];

      return {
        connections: nextConnections,
        pinConnections: nextPinConnections,
      };
    }),

  batchDisconnect: (connectionIds, graphId) =>
    set((state) => {
      if (connectionIds.length === 0) return state;
      if (graphId && state.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(state.graphEntities[graphId]);
        for (const connectionId of connectionIds) disconnectBucketConnection(bucket, connectionId);
        return commitGraphBucket(state, graphId, bucket);
      }

      const nextConnections = { ...state.connections };
      const nextPinConnections = { ...state.pinConnections };

      for (const connectionId of connectionIds) {
        const conn = nextConnections[connectionId];
        if (!conn) continue;

        nextPinConnections[conn.from] =
          (nextPinConnections[conn.from] ?? []).filter((id) => id !== connectionId);
        nextPinConnections[conn.to] =
          (nextPinConnections[conn.to] ?? []).filter((id) => id !== connectionId);

        delete nextConnections[connectionId];
      }

      return {
        connections: nextConnections,
        pinConnections: nextPinConnections,
      };
    }),

  applyConnectionDraft: (pinA, pinB, graphId) => {
    const state = get();
    const readPin = (pinId: PinId) => (graphId ? state.getGraphPin(graphId, pinId) : state.pins[pinId]);
    const readPinConnections = (pinId: PinId) =>
      graphId ? state.getGraphPinConnections(graphId, pinId) : state.pinConnections[pinId] ?? [];
    const readConnection = (connectionId: ConnectionId) =>
      graphId
        ? state.graphEntities[graphId]?.connections[connectionId] ?? state.connections[connectionId]
        : state.connections[connectionId];
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

    // input pin 单入：断开已有上游
    if (toPin.direction === 'input') {
      for (const cid of readPinConnections(to)) disconnectedIds.push(cid);
    }
    // exec output 单出：断开已有下游
    if (fromPin.direction === 'output' && fromPin.type === 'exec') {
      for (const cid of readPinConnections(from)) {
        const conn = readConnection(cid);
        if (conn?.from === from && !disconnectedIds.includes(cid)) disconnectedIds.push(cid);
      }
    }

    set((s) => {
      if (graphId && s.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(s.graphEntities[graphId]);
        for (const cid of disconnectedIds) disconnectBucketConnection(bucket, cid);
        connectBucketPins(bucket, from, to);
        return commitGraphBucket(s, graphId, bucket);
      }

      const nextConnections = { ...s.connections };
      const nextPinConnections = { ...s.pinConnections };

      for (const cid of disconnectedIds) {
        const conn = nextConnections[cid];
        if (!conn) continue;
        nextPinConnections[conn.from] =
          (nextPinConnections[conn.from] ?? []).filter((id) => id !== cid);
        nextPinConnections[conn.to] =
          (nextPinConnections[conn.to] ?? []).filter((id) => id !== cid);
        delete nextConnections[cid];
      }

      if (!nextConnections[connectionId]) {
        nextConnections[connectionId] = { id: connectionId, from, to };
        nextPinConnections[from] = [...(nextPinConnections[from] ?? []), connectionId];
        nextPinConnections[to] = [...(nextPinConnections[to] ?? []), connectionId];
      }

      return { connections: nextConnections, pinConnections: nextPinConnections };
    });

    return { from, to, connectionId, disconnectedIds };
  },

  revertConnectionDraft: (draft, graphId) =>
    set((s) => {
      if (graphId && s.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(s.graphEntities[graphId]);
        disconnectBucketConnection(bucket, draft.connectionId);
        for (const cid of draft.disconnectedIds) {
          const parts = cid.split('->');
          if (parts.length !== 2 || bucket.connections[cid]) continue;
          connectBucketPins(bucket, parts[0], parts[1]);
        }
        return commitGraphBucket(s, graphId, bucket);
      }

      const nextConnections = { ...s.connections };
      const nextPinConnections = { ...s.pinConnections };

      const created = nextConnections[draft.connectionId];
      if (created) {
        nextPinConnections[created.from] =
          (nextPinConnections[created.from] ?? []).filter((id) => id !== draft.connectionId);
        nextPinConnections[created.to] =
          (nextPinConnections[created.to] ?? []).filter((id) => id !== draft.connectionId);
        delete nextConnections[draft.connectionId];
      }

      for (const cid of draft.disconnectedIds) {
        const parts = cid.split('->');
        if (parts.length !== 2 || nextConnections[cid]) continue;
        const [f, t] = parts;
        nextConnections[cid] = { id: cid, from: f, to: t };
        nextPinConnections[f] = [...(nextPinConnections[f] ?? []), cid];
        nextPinConnections[t] = [...(nextPinConnections[t] ?? []), cid];
      }

      return { connections: nextConnections, pinConnections: nextPinConnections };
    }),

  batchConnect: (pairs, graphId) =>
    set((s) => {
      if (pairs.length === 0) return s;
      if (graphId && s.graphEntities[graphId]) {
        const bucket = cloneGraphBucket(s.graphEntities[graphId]);
        for (const { from, to } of pairs) connectBucketPins(bucket, from, to);
        return commitGraphBucket(s, graphId, bucket);
      }

      const nextConnections = { ...s.connections };
      const nextPinConnections = { ...s.pinConnections };
      for (const { from, to } of pairs) {
        const id: ConnectionId = `${from}->${to}`;
        if (nextConnections[id]) continue;
        nextConnections[id] = { id, from, to };
        nextPinConnections[from] = [...(nextPinConnections[from] ?? []), id];
        nextPinConnections[to] = [...(nextPinConnections[to] ?? []), id];
      }
      return { connections: nextConnections, pinConnections: nextPinConnections };
    }),

  // ======================================================
  // Graph
  // ======================================================
  getGraphNode: (graphId, nodeId) => {
    const state = get();
    if (Object.keys(state.graphEntities).length > 0) {
      return state.graphEntities[graphId]?.nodes[nodeId];
    }
    return state.nodes[nodeId];
  },

  getGraphPin: (graphId, pinId) => {
    const state = get();
    if (Object.keys(state.graphEntities).length > 0) {
      return state.graphEntities[graphId]?.pins[pinId];
    }
    return state.pins[pinId];
  },

  getGraphNodePins: (graphId, nodeId) => {
    const state = get();
    if (Object.keys(state.graphEntities).length > 0) {
      return state.graphEntities[graphId]?.nodePins[nodeId] ?? [];
    }
    return state.nodePins[nodeId] ?? [];
  },

  getGraphPinConnections: (graphId, pinId) => {
    const state = get();
    if (Object.keys(state.graphEntities).length > 0) {
      return state.graphEntities[graphId]?.pinConnections[pinId] ?? [];
    }
    return state.pinConnections[pinId] ?? [];
  },

  getGraphConnections: (graphId) => {
    const state = get();
    const bucket = state.graphEntities[graphId];
    if (bucket) return Object.values(bucket.connections);
    if (Object.keys(state.graphEntities).length > 0) return [];

    const nodeIds = state.graphNodes[graphId] ?? [];
    const connIds = new Set<string>();
    for (const nodeId of nodeIds) {
      for (const pinId of state.nodePins[nodeId] ?? []) {
        for (const connId of state.pinConnections[pinId] ?? []) {
          connIds.add(connId);
        }
      }
    }
    return Array.from(connIds).map((connId) => state.connections[connId]).filter(Boolean);
  },

  clearGraph: (graphId) =>
    set((state) => {
      if (state.graphEntities[graphId]) {
        const previousBucket = state.graphEntities[graphId];
        const graphEntities = { ...state.graphEntities };
        delete graphEntities[graphId];
        return {
          graphEntities,
          ...patchFlatMirrorForGraphBucket(state, graphEntities, graphId, previousBucket, undefined),
        };
      }

      const nextGraphNodes = { ...state.graphNodes };
      const nodeIds = state.graphNodes[graphId] ?? [];
      const nextNodes = { ...state.nodes };
      const nextPins = { ...state.pins };
      const nextConnections = { ...state.connections };
      const nextNodePins = { ...state.nodePins };
      const nextPinConnections = { ...state.pinConnections };

      for (const nodeId of nodeIds) {
        for (const pinId of nextNodePins[nodeId] ?? []) {
          for (const connId of nextPinConnections[pinId] ?? []) {
            const conn = nextConnections[connId];
            if (!conn) continue;
            const otherPin = conn.from === pinId ? conn.to : conn.from;
            nextPinConnections[otherPin] = (nextPinConnections[otherPin] ?? []).filter(
              (id) => id !== connId,
            );
            delete nextConnections[connId];
          }
          delete nextPinConnections[pinId];
          delete nextPins[pinId];
        }
        delete nextNodePins[nodeId];
        delete nextNodes[nodeId];
      }
      delete nextGraphNodes[graphId];

      return {
        nodes: nextNodes,
        pins: nextPins,
        connections: nextConnections,
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
        graphNodes: nextGraphNodes,
      };
    }),

  hydrateGraphs: (graphs) => {
    const graphEntities: Record<GraphId, GraphEntityBucket> = {};
    Object.values(graphs).forEach((graph) => {
      graphEntities[graph.id] = buildGraphBucket(graph.id, graph);
    });

    set({ graphEntities, ...flattenGraphBuckets(graphEntities) });
  },

  addGraphFromData: (graphId, graph) => {
    set((state) => {
      const bucket = buildGraphBucket(graphId, graph);
      const graphEntities = {
        ...state.graphEntities,
        [graphId]: bucket,
      };
      return {
        graphEntities,
        ...patchFlatMirrorForGraphBucket(
          state,
          graphEntities,
          graphId,
          state.graphEntities[graphId],
          bucket,
        ),
      };
    });
  },

  replaceGraphNodes: (graphId, nodes) => {
    const state = get();

    const pins: PinData[] = [];
    const connectionItems: { fromPin: string; toPin: string }[] = [];
    const toPinId = (p: string | PinDataInput): string =>
      typeof p === 'string' ? p : (p?.id ?? '');

    const nodeData = nodes.map((n: RuntimeNodeInput) => {
      const inputIds = (n.inputs || []).map(toPinId).filter(Boolean);
      const outputIds = (n.outputs || []).map(toPinId).filter(Boolean);

      // 收集完整 Pin 对象：节点含 Pin 对象则用其展开，否则从 Store 查找
      [...(n.inputs || []), ...(n.outputs || [])].forEach((p) => {
        const pin = typeof p === 'object' && p?.id ? p : state.getGraphPin(graphId, toPinId(p));
        if (pin && !pins.some((x) => x.id === (pin.id ?? toPinId(p)))) {
          pins.push(pin);
        }
      });

      // 连接事实只来自 pinConnections，忽略调用方可能携带的运行时 links。
      (n.outputs || []).forEach((p) => {
        const pinId = toPinId(p);
        const links = state.getGraphPinConnections(graphId, pinId).map((cid) => {
          const conn = state.graphEntities[graphId]?.connections[cid] ?? state.connections[cid];
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
