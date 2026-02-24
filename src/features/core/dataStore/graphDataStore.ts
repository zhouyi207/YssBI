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

interface GraphDataStore {
  // ======================
  // 实体表
  // ======================
  nodes: Record<NodeId, NodeData>;
  pins: Record<PinId, PinData>;
  connections: Record<ConnectionId, ConnectionData>;

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
  updateNode(nodeId: NodeId, patch: Partial<NodeData>): void;
  /** 批量更新节点位置（拖拽时使用，避免 replaceGraphNodes 的 O(n) 清空+重建） */
  batchUpdateNodePositions(updates: Array<{ nodeId: NodeId; x: number; y: number }>): void;
  deleteNode(nodeId: NodeId): void;
  /** 批量添加节点和它们的 pin（单次 set，避免 N 次 re-render） */
  batchAddNodesAndPins(graphId: GraphId, items: Array<{ node: NodeData; pins: PinData[] }>): void;
  /** 批量删除节点（单次 set） */
  batchDeleteNodes(nodeIds: NodeId[]): void;

  // ======================
  // Pin
  // ======================
  addPin(nodeId: NodeId, pin: PinData): void;
  updatePin(pinId: PinId, patch: Partial<PinData>): void;
  deletePin(pinId: PinId): void;
  /** 批量更新 pin（断连 + 删 pin + 加 pin，单次 set） */
  batchUpdatePins(params: {
    disconnectIds: ConnectionId[];
    removePinIds: PinId[];
    addPins: Array<{ nodeId: NodeId; pin: PinData }>;
  }): void;
  /** 按后端提供的顺序重排节点的 pin 列表 */
  reorderNodePins(nodeId: NodeId, pinOrder: PinId[]): void;

  // ======================
  // Connection
  // ======================
  connect(from: PinId, to: PinId): void;
  disconnect(connectionId: ConnectionId): void;
  /** 批量断开连接（单次 set） */
  batchDisconnect(connectionIds: ConnectionId[]): void;

  // ======================
  // Graph
  // ======================
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

  graphNodes: {},
  nodePins: {},
  pinConnections: {},

  // ======================================================
  // Node
  // ======================================================
  addNode: (graphId, node) =>
    set((state) => {
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

  updateNode: (nodeId, patch) =>
    set((state) => {
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

  batchUpdateNodePositions: (updates) =>
    set((state) => {
      if (updates.length === 0) return state;
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

  deleteNode: (nodeId) =>
    set((state) => {
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
      const graphId = node.graphId;
      const graphNodeIds = state.graphNodes[graphId] ?? [];
      const nextGraphNodes = {
        ...state.graphNodes,
        [graphId]: graphNodeIds.filter((id) => id !== nodeId),
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
            nextPins[pin.id] = pin;
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

  batchDeleteNodes: (nodeIds) =>
    set((state) => {
      if (nodeIds.length === 0) return state;

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
  addPin: (nodeId, pin) =>
    set((state) => {
      if (state.pins[pin.id]) {
        logger.graph.warn(`Pin "${pin.id}" already exists`, 'GraphDataStore');
        return state;
      }

      return {
        pins: {
          ...state.pins,
          [pin.id]: pin,
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

  updatePin: (pinId, patch) =>
    set((state) => {
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

  deletePin: (pinId) =>
    set((state) => {
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

  batchUpdatePins: ({ disconnectIds, removePinIds, addPins }) =>
    set((state) => {
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

      // 3. 添加新 pin
      for (const { nodeId, pin } of addPins) {
        if (!nextPins[pin.id]) {
          nextPins[pin.id] = pin;
          nextNodePins[nodeId] = [...(nextNodePins[nodeId] ?? []), pin.id];
          nextPinConnections[pin.id] = [];
        }
      }

      return {
        pins: nextPins,
        connections: nextConnections,
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
      };
    }),

  reorderNodePins: (nodeId, pinOrder) =>
    set((state) => {
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
  connect: (from, to) =>
    set((state) => {
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

  disconnect: (connectionId) =>
    set((state) => {
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

  batchDisconnect: (connectionIds) =>
    set((state) => {
      if (connectionIds.length === 0) return state;

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

  // ======================================================
  // Graph
  // ======================================================
  clearGraph: (graphId) => {
    const nodeIds = get().graphNodes[graphId] ?? [];
    for (const nodeId of nodeIds) {
      get().deleteNode(nodeId);
    }

    set((state) => {
      const nextGraphNodes = { ...state.graphNodes };
      delete nextGraphNodes[graphId];
      return { graphNodes: nextGraphNodes };
    });
  },

  hydrateGraphs: (graphs) => {
    const nodes: Record<NodeId, NodeData> = {};
    const pins: Record<PinId, PinData> = {};
    const connections: Record<ConnectionId, ConnectionData> = {};
    const graphNodes: Record<GraphId, NodeId[]> = {};
    const nodePins: Record<NodeId, PinId[]> = {};
    const pinConnections: Record<PinId, ConnectionId[]> = {};

    const toPinIds = (arr: unknown): string[] => {
      if (!Array.isArray(arr)) return [];
      return arr.map((p) => (typeof p === 'string' ? p : (p as { id?: string })?.id ?? '')).filter(Boolean);
    };

    Object.values(graphs).forEach((graph) => {
      const nodeIds: NodeId[] = [];
      const conns = Array.isArray(graph.connections)
        ? graph.connections.map((c: { from: string; to: string }) => ({ fromPin: c.from, toPin: c.to }))
        : graph.connections.connections;

      (graph.nodes || []).forEach((node) => {
        const inputIds = toPinIds(node.inputs);
        const outputIds = toPinIds(node.outputs);
        nodes[node.id] = {
          ...node,
          graphId: graph.id,
          inputs: inputIds,
          outputs: outputIds,
          nodeType: (node as NodeData).nodeType ?? (node as { nodeType?: string }).nodeType ?? '',
          category: (node as NodeData).category ?? [],
          title: (node as NodeData).title ?? '',
          uiStyle: (node as NodeData).uiStyle ?? 'default',
          position: (node as NodeData).position ?? { x: 0, y: 0 },
        };
        nodeIds.push(node.id);
        const pinIds = [...inputIds, ...outputIds];
        nodePins[node.id] = pinIds;
        pinIds.forEach((pid) => { pinConnections[pid] = []; });
      });

      graphNodes[graph.id] = nodeIds;

      (graph.pins || []).forEach((pin: PinData) => {
        pins[pin.id] = pin;
      });

      conns.forEach((c: { fromPin: string; toPin: string }) => {
        const from = c.fromPin;
        const to = c.toPin;
        const id = `${from}->${to}`;
        connections[id] = { id, from, to };
        pinConnections[from] = pinConnections[from] ?? [];
        pinConnections[from].push(id);
        pinConnections[to] = pinConnections[to] ?? [];
        pinConnections[to].push(id);
      });
    });

    set({ nodes, pins, connections, graphNodes, nodePins, pinConnections });
  },

  addGraphFromData: (graphId, graph) => {
    const conns = Array.isArray(graph.connections)
      ? graph.connections.map((c: { from: string; to: string }) => ({ fromPin: c.from, toPin: c.to }))
      : graph.connections.connections;
    set((state) => {
      const nodeIds: NodeId[] = [];
      const nextNodes = { ...state.nodes };
      const nextPins = { ...state.pins };
      const nextConnections = { ...state.connections };
      const nextNodePins = { ...state.nodePins };
      const nextPinConnections = { ...state.pinConnections };

      const toPinId = (p: string | PinData): string =>
        typeof p === 'object' && p?.id ? p.id : String(p);

      (graph.nodes || []).forEach((node: { id: string; nodeType?: string; uiStyle?: string; inputs?: (string | PinData)[]; outputs?: (string | PinData)[]; category?: string[]; title?: string; position?: { x: number; y: number }; description?: string; isInternal?: boolean; paramsKind?: string; variableId?: string; variableName?: string; variableType?: string; subGraphId?: string; dataframeId?: string; dataframeName?: string }) => {
        const inputIds = (node.inputs ?? []).map(toPinId).filter(Boolean);
        const outputIds = (node.outputs ?? []).map(toPinId).filter(Boolean);
        const allPinIds = [...inputIds, ...outputIds];
        const nodeType = node.nodeType ?? '';
        const uiStyle = node.uiStyle ?? 'default';
        nextNodes[node.id] = {
          id: node.id,
          graphId,
          nodeType,
          category: node.category ?? [],
          title: node.title ?? '',
          inputs: inputIds,
          outputs: outputIds,
          uiStyle,
          description: node.description,
          position: node.position ?? { x: 0, y: 0 },
          isInternal: node.isInternal,
          paramsKind: (node.paramsKind ?? 'none') as NodeData['paramsKind'],
          variableId: node.variableId,
          variableName: node.variableName,
          variableType: node.variableType,
          subGraphId: node.subGraphId,
          dataframeId: node.dataframeId,
          dataframeName: node.dataframeName,
        };
        nodeIds.push(node.id);
        nextNodePins[node.id] = allPinIds;
        allPinIds.forEach((pid) => {
          if (!nextPinConnections[pid]) nextPinConnections[pid] = [];
        });
      });

      (graph.pins || []).forEach((pin: PinData) => {
        nextPins[pin.id] = pin;
      });

      conns.forEach((c: { fromPin: string; toPin: string }) => {
        const from = c.fromPin;
        const to = c.toPin;
        const id = `${from}->${to}`;
        nextConnections[id] = { id, from, to };
        nextPinConnections[from] = nextPinConnections[from] || [];
        nextPinConnections[from].push(id);
        nextPinConnections[to] = nextPinConnections[to] || [];
        nextPinConnections[to].push(id);
      });

      return {
        nodes: nextNodes,
        pins: nextPins,
        connections: nextConnections,
        graphNodes: { ...state.graphNodes, [graphId]: nodeIds },
        nodePins: nextNodePins,
        pinConnections: nextPinConnections,
      };
    });
  },

  replaceGraphNodes: (graphId, nodes) => {
    const state = get();
    const pinsRecord = state.pins;

    const pins: PinData[] = [];
    const connectionItems: { fromPin: string; toPin: string }[] = [];
    const toPinId = (p: string | PinData): string =>
      typeof p === 'string' ? p : (p?.id ?? '');

    const nodeData = nodes.map((n: RuntimeNodeInput) => {
      const inputIds = (n.inputs || []).map(toPinId).filter(Boolean);
      const outputIds = (n.outputs || []).map(toPinId).filter(Boolean);

      // 收集完整 Pin 对象：节点含 Pin 对象则用其展开，否则从 Store 查找
      [...(n.inputs || []), ...(n.outputs || [])].forEach((p) => {
        const pin = typeof p === 'object' && p?.id ? p : pinsRecord[toPinId(p)];
        if (pin && !pins.some((x) => x.id === (pin.id ?? toPinId(p)))) {
          pins.push(pin);
        }
      });

      // 提取连接：Pin 对象有 links 则用 links，否则从 pinConnections 推导
      (n.outputs || []).forEach((p) => {
        const pinId = toPinId(p);
        const pinObj = typeof p === 'object' ? p : null;
        const links = pinObj && Array.isArray(pinObj.links)
          ? pinObj.links
          : (state.pinConnections[pinId] ?? []).map((cid) => {
              const conn = state.connections[cid];
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
      canvas: { x: 0, y: 0, scale: 1 },
      nodes: nodeData,
      pins,
      connections: { connections: connectionItems },
    });
  },
}));
