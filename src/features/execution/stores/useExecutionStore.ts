import { create } from 'zustand';
import { ExecutionStatus, NodeExecutionState, ExecutionState } from '@/shared/types/editor';

interface ExecutionStore extends ExecutionState {
  // Actions
  startExecution: () => void;
  completeExecution: () => void;
  setCurrentNode: (nodeId: string | null) => void;
  markNodeExecuting: (nodeId: string) => void;
  markNodeCompleted: (nodeId: string) => void;
  markNodeError: (nodeId: string, error?: string) => void;
  addActiveConnection: (fromPinId: string, toPinId: string) => void;
  removeActiveConnection: (fromPinId: string, toPinId: string) => void;
  markConnectionCompleted: (fromPinId: string, toPinId: string) => void;
  reset: () => void;
}

const initialState: ExecutionState = {
  status: "idle",
  currentNodeId: null,
  executedNodes: new Set(),
  nodeStates: new Map(),
  activeConnections: new Set(),
  completedConnections: new Set(),
};

export const useExecutionStore = create<ExecutionStore>((set) => ({
  ...initialState,

  startExecution: () => set({
    status: "running",
    currentNodeId: null,
    executedNodes: new Set(),
    nodeStates: new Map(),
    activeConnections: new Set(),
    completedConnections: new Set(),
  }),

  completeExecution: () => set((state) => ({
    status: "completed",
    currentNodeId: null,
    activeConnections: new Set(),
  })),

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

  markNodeError: (nodeId, error) => set((state) => {
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

  addActiveConnection: (fromPinId, toPinId) => set((state) => {
    const newActiveConnections = new Set(state.activeConnections);
    newActiveConnections.add(`${fromPinId}->${toPinId}`);
    return { activeConnections: newActiveConnections };
  }),

  removeActiveConnection: (fromPinId, toPinId) => set((state) => {
    const newActiveConnections = new Set(state.activeConnections);
    newActiveConnections.delete(`${fromPinId}->${toPinId}`);
    return { activeConnections: newActiveConnections };
  }),

  markConnectionCompleted: (fromPinId, toPinId) => set((state) => {
    const newCompletedConnections = new Set(state.completedConnections);
    newCompletedConnections.add(`${fromPinId}->${toPinId}`);
    return { completedConnections: newCompletedConnections };
  }),

  reset: () => set(initialState),
}));
