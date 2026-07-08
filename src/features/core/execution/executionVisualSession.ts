import type { ExecutionEvent } from '@/shared/types/ui/execution';

export type ExecutionVisualSnapshot = {
  active: boolean;
  graphPath: string | null;
  status: 'idle' | 'running' | 'completed' | 'error';
  executingNodeId: string | null;
  executedNodeIds: Set<string>;
  errorNodeIds: Set<string>;
  nodeDurations: Map<string, number>;
  /** data 取数（ConnectionActive） */
  completedConnections: Set<string>;
  /** data 流动（ConnectionFlow） */
  flowingConnections: Set<string>;
};

export function connectionKey(fromPinId: string, toPinId: string): string {
  return `${fromPinId}->${toPinId}`;
}

function idleSnapshot(): ExecutionVisualSnapshot {
  return {
    active: false,
    graphPath: null,
    status: 'idle',
    executingNodeId: null,
    executedNodeIds: new Set(),
    errorNodeIds: new Set(),
    nodeDurations: new Map(),
    completedConnections: new Set(),
    flowingConnections: new Set(),
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

export function resetExecutionVisual(graphPath: string): void {
  snapshot = {
    active: true,
    graphPath,
    status: 'running',
    executingNodeId: null,
    executedNodeIds: new Set(),
    errorNodeIds: new Set(),
    nodeDurations: new Map(),
    completedConnections: new Set(),
    flowingConnections: new Set(),
  };
  publish();
}

export function clearExecutionVisual(): void {
  snapshot = idleSnapshot();
  publish();
}

/** Apply one channel event to the live visual snapshot (no React store). */
export function applyExecutionVisualEvent(graphPath: string, event: ExecutionEvent): void {
  if (!snapshot.active || snapshot.graphPath !== graphPath) {
    if (event.event === 'executionStart') {
      resetExecutionVisual(graphPath);
      return;
    }
    return;
  }

  switch (event.event) {
    case 'executionStart':
      resetExecutionVisual(graphPath);
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
        // 保持 running，直到 executionComplete；避免后续连线动画被提前关掉
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
    case 'connectionFlow': {
      const flowingConnections = new Set(snapshot.flowingConnections);
      flowingConnections.add(connectionKey(event.data.fromPinId, event.data.toPinId));
      snapshot = { ...snapshot, flowingConnections };
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
  flowingConnections: Set<string>;
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
    flowingConnections: new Set(snap.flowingConnections),
  };
}
