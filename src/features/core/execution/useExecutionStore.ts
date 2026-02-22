import { create } from 'zustand';
import type { ExecutionState, ExecutionEvent, RecordedEvent } from '@/shared/types/ui';

interface ExecutionStore extends ExecutionState {
  startExecution: () => void;
  completeExecution: () => void;
  setCurrentNode: (nodeId: string | null) => void;
  markNodeExecuting: (nodeId: string) => void;
  markNodeCompleted: (nodeId: string) => void;
  markNodeError: (nodeId: string, error?: string) => void;
  markConnectionCompleted: (fromPinId: string, toPinId: string) => void;
  markConnectionError: (fromPinId: string, toPinId: string) => void;
  setRecording: (recording: RecordedEvent[]) => void;
  setPlaying: (playing: boolean) => void;
  resetVisuals: () => void;
  reset: () => void;
  applyEvent: (event: ExecutionEvent) => void;
}

const initialState: ExecutionState = {
  status: "idle",
  currentNodeId: null,
  executedNodes: new Set(),
  nodeStates: new Map(),
  completedConnections: new Set(),
  errorConnections: new Set(),
  recording: [],
  isPlaying: false,
};

export const useExecutionStore = create<ExecutionStore>((set, get) => ({
  ...initialState,

  startExecution: () => set({
    status: "running",
    currentNodeId: null,
    executedNodes: new Set(),
    nodeStates: new Map(),
    completedConnections: new Set(),
    errorConnections: new Set(),
    recording: [],
  }),

  completeExecution: () => set({
    status: "completed",
    currentNodeId: null,
  }),

  setCurrentNode: (nodeId) => set({ currentNodeId: nodeId }),

  markNodeExecuting: (nodeId) => set((state) => {
    const newNodeStates = new Map(state.nodeStates);
    newNodeStates.set(nodeId, {
      nodeId,
      status: "executing",
      timestamp: Date.now(),
    });
    return {
      currentNodeId: nodeId,
      nodeStates: newNodeStates,
    };
  }),

  markNodeCompleted: (nodeId) => set((state) => {
    const newNodeStates = new Map(state.nodeStates);
    newNodeStates.set(nodeId, {
      nodeId,
      status: "completed",
      timestamp: Date.now(),
    });
    const newExecutedNodes = new Set(state.executedNodes);
    newExecutedNodes.add(nodeId);
    return {
      nodeStates: newNodeStates,
      executedNodes: newExecutedNodes,
    };
  }),

  markNodeError: (nodeId, _error) => set((state) => {
    const newNodeStates = new Map(state.nodeStates);
    newNodeStates.set(nodeId, {
      nodeId,
      status: "error",
      timestamp: Date.now(),
    });
    return {
      status: "error",
      currentNodeId: null,
      nodeStates: newNodeStates,
    };
  }),

  markConnectionCompleted: (fromPinId, toPinId) => set((state) => {
    const next = new Set(state.completedConnections);
    next.add(`${fromPinId}->${toPinId}`);
    return { completedConnections: next };
  }),

  markConnectionError: (fromPinId, toPinId) => set((state) => {
    const next = new Set(state.errorConnections);
    next.add(`${fromPinId}->${toPinId}`);
    return { errorConnections: next };
  }),

  setRecording: (recording) => set({ recording }),

  setPlaying: (playing) => set({ isPlaying: playing }),

  resetVisuals: () => set({
    status: "idle",
    currentNodeId: null,
    executedNodes: new Set(),
    nodeStates: new Map(),
    completedConnections: new Set(),
    errorConnections: new Set(),
    isPlaying: false,
  }),

  reset: () => set({ ...initialState }),

  applyEvent: (event: ExecutionEvent) => {
    const store = get();
    switch (event.event) {
      case 'executionStart':
        store.startExecution();
        break;
      case 'executionComplete':
        store.completeExecution();
        break;
      case 'nodeStart':
        store.markNodeExecuting(event.data.nodeId);
        break;
      case 'nodeComplete':
        store.markNodeCompleted(event.data.nodeId);
        break;
      case 'nodeError':
        store.markNodeError(event.data.nodeId, event.data.error);
        break;
      case 'connectionActive':
        store.markConnectionCompleted(event.data.fromPinId, event.data.toPinId);
        break;
    }
  },
}));
