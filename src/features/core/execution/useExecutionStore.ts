import { create } from 'zustand';
import type { ExecutionState, GraphExecutionState, ExecutionEvent, RecordedEvent } from '@/shared/types/ui';

const emptyGraphState = (): GraphExecutionState => ({
  status: "idle",
  currentNodeId: null,
  executedNodes: new Set(),
  nodeStates: new Map(),
  completedConnections: new Set(),
  errorConnections: new Set(),
  recording: [],
  graphDirty: false,
  nodeDurations: new Map(),
});

interface ExecutionStore extends ExecutionState {
  /** 获取指定图的执行状态（不存在时返回空状态） */
  getGraph: (graphId: string) => GraphExecutionState;

  startExecution: (graphId: string) => void;
  completeExecution: (graphId: string) => void;
  failExecution: (graphId: string) => void;
  markNodeExecuting: (graphId: string, nodeId: string) => void;
  markNodeCompleted: (graphId: string, nodeId: string, durationMs?: number) => void;
  markNodeError: (graphId: string, nodeId: string, durationMs?: number) => void;
  markConnectionCompleted: (graphId: string, fromPinId: string, toPinId: string) => void;
  setRecording: (graphId: string, recording: RecordedEvent[]) => void;
  setPlaying: (playing: boolean, graphId?: string) => void;
  markGraphDirty: (graphId: string) => void;
  resetGraphVisuals: (graphId: string) => void;
  applyEvent: (graphId: string, event: ExecutionEvent) => void;
}

function updateGraph(
  state: ExecutionState,
  graphId: string,
  patch: Partial<GraphExecutionState>,
): { graphs: Record<string, GraphExecutionState> } {
  const prev = state.graphs[graphId] ?? emptyGraphState();
  return {
    graphs: {
      ...state.graphs,
      [graphId]: { ...prev, ...patch },
    },
  };
}

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  graphs: {},
  playbackGraphId: null,
  isPlaying: false,

  getGraph: (graphId) => get().graphs[graphId] ?? emptyGraphState(),

  startExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "running",
    currentNodeId: null,
    executedNodes: new Set(),
    nodeStates: new Map(),
    completedConnections: new Set(),
    errorConnections: new Set(),
    graphDirty: false,
    nodeDurations: new Map(),
  })),

  completeExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "completed",
    currentNodeId: null,
  })),

  failExecution: (graphId) => set((state) => updateGraph(state, graphId, {
    status: "error",
    currentNodeId: null,
  })),

  markNodeExecuting: (graphId, nodeId) => set((state) => {
    const g = state.graphs[graphId] ?? emptyGraphState();
    const newNodeStates = new Map(g.nodeStates);
    newNodeStates.set(nodeId, { nodeId, status: "executing", timestamp: Date.now() });
    return updateGraph(state, graphId, { currentNodeId: nodeId, nodeStates: newNodeStates });
  }),

  markNodeCompleted: (graphId, nodeId, durationMs?: number) => set((state) => {
    const g = state.graphs[graphId] ?? emptyGraphState();
    const newNodeStates = new Map(g.nodeStates);
    newNodeStates.set(nodeId, { nodeId, status: "completed", timestamp: Date.now(), durationMs });
    const newExecutedNodes = new Set(g.executedNodes);
    newExecutedNodes.add(nodeId);
    const newNodeDurations = durationMs != null ? new Map(g.nodeDurations).set(nodeId, durationMs) : g.nodeDurations;
    return updateGraph(state, graphId, { nodeStates: newNodeStates, executedNodes: newExecutedNodes, nodeDurations: newNodeDurations });
  }),

  markNodeError: (graphId, nodeId, durationMs?: number) => set((state) => {
    const g = state.graphs[graphId] ?? emptyGraphState();
    const newNodeStates = new Map(g.nodeStates);
    newNodeStates.set(nodeId, { nodeId, status: "error", timestamp: Date.now(), durationMs });
    const newNodeDurations = durationMs != null ? new Map(g.nodeDurations).set(nodeId, durationMs) : g.nodeDurations;
    return updateGraph(state, graphId, { status: "error", currentNodeId: null, nodeStates: newNodeStates, nodeDurations: newNodeDurations });
  }),

  markConnectionCompleted: (graphId, fromPinId, toPinId) => set((state) => {
    const g = state.graphs[graphId] ?? emptyGraphState();
    const next = new Set(g.completedConnections);
    next.add(`${fromPinId}->${toPinId}`);
    return updateGraph(state, graphId, { completedConnections: next });
  }),

  setRecording: (graphId, recording) => set((state) => updateGraph(state, graphId, { recording })),

  setPlaying: (playing, graphId) => set({
    isPlaying: playing,
    playbackGraphId: playing ? (graphId ?? get().playbackGraphId) : null,
  }),

  markGraphDirty: (graphId) => set((state) => {
    const g = state.graphs[graphId];
    if (!g || (g.status === "idle" && !(state.isPlaying && state.playbackGraphId === graphId))) return state;
    const stop = state.playbackGraphId === graphId;
    return {
      ...updateGraph(state, graphId, {
        graphDirty: true,
        status: "idle",
        currentNodeId: null,
        executedNodes: new Set(),
        nodeStates: new Map(),
        completedConnections: new Set(),
        errorConnections: new Set(),
        recording: [],
        nodeDurations: new Map(),
      }),
      isPlaying: stop ? false : state.isPlaying,
      playbackGraphId: stop ? null : state.playbackGraphId,
    };
  }),

  resetGraphVisuals: (graphId) => set((state) => {
    const stop = state.playbackGraphId === graphId;
    return {
      ...updateGraph(state, graphId, {
        status: "idle",
        currentNodeId: null,
        executedNodes: new Set(),
        nodeStates: new Map(),
        completedConnections: new Set(),
        errorConnections: new Set(),
        nodeDurations: new Map(),
      }),
      isPlaying: stop ? false : state.isPlaying,
      playbackGraphId: stop ? null : state.playbackGraphId,
    };
  }),

  applyEvent: (graphId, event) => {
    const store = get();
    switch (event.event) {
      case 'executionStart':
        store.startExecution(graphId);
        break;
      case 'executionComplete':
        store.completeExecution(graphId);
        break;
      case 'nodeStart':
        store.markNodeExecuting(graphId, event.data.nodeId);
        break;
      case 'nodeComplete':
        store.markNodeCompleted(graphId, event.data.nodeId, event.data.durationMs);
        break;
      case 'nodeError':
        store.markNodeError(graphId, event.data.nodeId, event.data.durationMs);
        break;
      case 'connectionActive':
        store.markConnectionCompleted(graphId, event.data.fromPinId, event.data.toPinId);
        break;
    }
  },
}));
