import { create } from 'zustand';
import { NodeId, PinId, GraphId, ConnectionId, GraphData } from '@/shared/types';
import { NodeData, PinData, ConnectionData } from '@/shared/types';

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
  deleteNode(nodeId: NodeId): void;

  // ======================
  // Pin
  // ======================
  addPin(nodeId: NodeId, pin: PinData): void;
  updatePin(pinId: PinId, patch: Partial<PinData>): void;
  deletePin(pinId: PinId): void;

  // ======================
  // Connection
  // ======================
  connect(from: PinId, to: PinId): void;
  disconnect(connectionId: ConnectionId): void;

  // ======================
  // Graph
  // ======================
  clearGraph(graphId: GraphId): void;
  hydrateGraphs(graphs: Record<GraphId, GraphData>): void;
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
        console.warn(`[GraphDataStore] Node "${node.id}" already exists`);
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
        console.warn(`[GraphDataStore] updateNode: Node "${nodeId}" not found`);
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

  deleteNode: (nodeId) =>
    set((state) => {
      const node = state.nodes[nodeId];
      if (!node) {
        console.warn(`[GraphDataStore] deleteNode: Node "${nodeId}" not found`);
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

  // ======================================================
  // Pin
  // ======================================================
  addPin: (nodeId, pin) =>
    set((state) => {
      if (state.pins[pin.id]) {
        console.warn(`[GraphDataStore] Pin "${pin.id}" already exists`);
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
        console.warn(`[GraphDataStore] updatePin: Pin "${pinId}" not found`);
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
        console.warn(`[GraphDataStore] deletePin: Pin "${pinId}" not found`);
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
        console.warn(
          `[GraphDataStore] disconnect: Connection "${connectionId}" not found`
        );
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

    Object.values(graphs).forEach((graph) => {
      const nodeIds: NodeId[] = [];

      // -----------------------------
      // 处理节点
      // -----------------------------
      graph.nodes.forEach((node: NodeData) => {
        nodes[node.id] = node;
        nodeIds.push(node.id);

        // nodePins 索引
        const pinIds = [...(node.inputs || []), ...(node.outputs || [])];
        nodePins[node.id] = pinIds;

        // 初始化 pinConnections
        pinIds.forEach((pinId) => {
          pinConnections[pinId] = [];
        });
      });

      graphNodes[graph.id] = nodeIds;

      // -----------------------------
      // 处理 pins
      // -----------------------------
      graph.pins.forEach((pin: PinData) => {
        pins[pin.id] = pin;
      });

      // -----------------------------
      // 处理 connections
      // -----------------------------
      graph.connections.forEach((c: ConnectionData) => {
        connections[c.id] = c;

        // 更新 pinConnections 索引
        pinConnections[c.from] = pinConnections[c.from] || [];
        pinConnections[c.from].push(c.id);

        pinConnections[c.to] = pinConnections[c.to] || [];
        pinConnections[c.to].push(c.id);
      });
    });

    set({ nodes, pins, connections, graphNodes, nodePins, pinConnections });
  },
}));
