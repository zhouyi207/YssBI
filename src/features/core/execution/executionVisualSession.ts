import type { ExecutionEvent } from '@/shared/types/ui/execution';

export type ExecutionVisualSnapshot = {
  active: boolean;
  graphId: string | null;
  status: 'idle' | 'running' | 'completed' | 'error';
  executingNodeId: string | null;
  executedNodeIds: Set<string>;
  errorNodeIds: Set<string>;
  nodeDurations: Map<string, number>;
  completedConnections: Set<string>;
};

export function connectionKey(fromPinId: string, toPinId: string): string {
  return `${fromPinId}->${toPinId}`;
}

function idleSnapshot(): ExecutionVisualSnapshot {
  return {
    active: false,
    graphId: null,
    status: 'idle',
    executingNodeId: null,
    executedNodeIds: new Set(),
    errorNodeIds: new Set(),
    nodeDurations: new Map(),
    completedConnections: new Set(),
  };
}

let snapshot: ExecutionVisualSnapshot = idleSnapshot();
const listeners = new Set<() => void>();

function publish(): void {
  listeners.forEach((listener) => listener());
}

export function getExecutionVisual(): ExecutionVisualSnapshot {
  return snapshot;
}

export function subscribeExecutionVisual(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function resetExecutionVisual(graphId: string): void {
  snapshot = {
    active: true,
    graphId,
    status: 'running',
    executingNodeId: null,
    executedNodeIds: new Set(),
    errorNodeIds: new Set(),
    nodeDurations: new Map(),
    completedConnections: new Set(),
  };
  publish();
}

export function clearExecutionVisual(): void {
  snapshot = idleSnapshot();
  publish();
}

/** Apply one channel event to the live visual snapshot (no React store). */
export function applyExecutionVisualEvent(graphId: string, event: ExecutionEvent): void {
  if (!snapshot.active || snapshot.graphId !== graphId) {
    if (event.event === 'executionStart') {
      resetExecutionVisual(graphId);
      return;
    }
    return;
  }

  switch (event.event) {
    case 'executionStart':
      resetExecutionVisual(graphId);
      break;
    case 'executionComplete':
      snapshot = {
        ...snapshot,
        status: event.data.hasError ? 'error' : 'completed',
        executingNodeId: null,
      };
      break;
    case 'nodeStart':
      snapshot = { ...snapshot, executingNodeId: event.data.nodeId };
      break;
    case 'nodeComplete': {
      const executedNodeIds = new Set(snapshot.executedNodeIds);
      executedNodeIds.add(event.data.nodeId);
      const nodeDurations = new Map(snapshot.nodeDurations);
      if (event.data.durationMs != null) {
        nodeDurations.set(event.data.nodeId, event.data.durationMs);
      }
      snapshot = {
        ...snapshot,
        executingNodeId: snapshot.executingNodeId === event.data.nodeId ? null : snapshot.executingNodeId,
        executedNodeIds,
        nodeDurations,
      };
      break;
    }
    case 'nodeError': {
      const errorNodeIds = new Set(snapshot.errorNodeIds);
      errorNodeIds.add(event.data.nodeId);
      const nodeDurations = new Map(snapshot.nodeDurations);
      if (event.data.durationMs != null) {
        nodeDurations.set(event.data.nodeId, event.data.durationMs);
      }
      snapshot = {
        ...snapshot,
        status: 'error',
        executingNodeId: snapshot.executingNodeId === event.data.nodeId ? null : snapshot.executingNodeId,
        errorNodeIds,
        nodeDurations,
      };
      break;
    }
    case 'connectionActive': {
      const completedConnections = new Set(snapshot.completedConnections);
      completedConnections.add(connectionKey(event.data.fromPinId, event.data.toPinId));
      snapshot = { ...snapshot, completedConnections };
      break;
    }
    default:
      break;
  }
  publish();
}

export function snapshotToGraphPatch(snap: ExecutionVisualSnapshot): {
  status: ExecutionVisualSnapshot['status'];
  nodeStates: Map<string, import('@/shared/types/ui/execution').NodeExecutionState>;
  completedConnections: Set<string>;
} {
  const nodeStates = new Map<string, import('@/shared/types/ui/execution').NodeExecutionState>();
  const now = Date.now();

  for (const nodeId of snap.executedNodeIds) {
    nodeStates.set(nodeId, {
      nodeId,
      status: 'completed',
      timestamp: now,
      durationMs: snap.nodeDurations.get(nodeId),
    });
  }
  for (const nodeId of snap.errorNodeIds) {
    nodeStates.set(nodeId, {
      nodeId,
      status: 'error',
      timestamp: now,
      durationMs: snap.nodeDurations.get(nodeId),
    });
  }

  return {
    status: snap.status === 'idle' ? 'completed' : snap.status,
    nodeStates,
    completedConnections: new Set(snap.completedConnections),
  };
}
