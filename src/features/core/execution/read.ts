import { useSyncExternalStore } from 'react';

import {
  freezeProjectionSnapshot,
  type DeepReadonly,
} from '@/shared/types/deepReadonly';
import { useExecutionStore } from './useExecutionStore';
import type {
  ExecutionStatus,
  GraphExecutionState,
  NodeExecutionState,
  PinHistoryProjection,
  PinPreviewState,
  RunOutputProjection,
} from '@/shared/types/ui/execution';

export interface GraphExecutionProjection {
  readonly status: ExecutionStatus;
  readonly runId: string | null;
  readonly nodeStates: ReadonlyMap<string, NodeExecutionState>;
  readonly completedConnections: ReadonlySet<string>;
  readonly flowingConnections: ReadonlySet<string>;
  readonly recording: readonly import('@/shared/types/ui/execution').RecordedEvent[];
  readonly graphDirty: boolean;
  readonly runOutput: DeepReadonly<RunOutputProjection>;
  readonly pinHistories: ReadonlyMap<string, DeepReadonly<PinHistoryProjection>>;
  readonly pinPreviews: ReadonlyMap<string, DeepReadonly<PinPreviewState>>;
}

export interface ExecutionReadSnapshot {
  readonly graphs: DeepReadonly<Record<string, GraphExecutionProjection>>;
  readonly isPlaying: boolean;
  readonly playbackGraphPath: string | null;
}

export interface ExecutionReadCapability {
  readonly getSnapshot: () => DeepReadonly<ExecutionReadSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function projectGraph(graph: GraphExecutionState): GraphExecutionProjection {
  return {
    status: graph.status,
    runId: graph.runId,
    nodeStates: graph.nodeStates,
    completedConnections: graph.completedConnections,
    flowingConnections: graph.flowingConnections,
    recording: graph.recording,
    graphDirty: graph.graphDirty,
    runOutput: graph.runOutput,
    pinHistories: graph.pinHistories,
    pinPreviews: graph.pinPreviews,
  };
}

function buildSnapshot(): DeepReadonly<ExecutionReadSnapshot> {
  const { graphs, isPlaying, playbackGraphPath } = useExecutionStore.getState();
  return freezeProjectionSnapshot({
    graphs: Object.fromEntries(
      Object.entries(graphs).map(([graphPath, graph]) => [graphPath, projectGraph(graph)]),
    ),
    isPlaying,
    playbackGraphPath,
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useExecutionStore.subscribe(refreshSnapshot);

export function getExecutionSnapshot(): DeepReadonly<ExecutionReadSnapshot> {
  return currentSnapshot;
}

export function subscribeExecutionRead(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useExecutionRead<T>(
  selector: (state: DeepReadonly<ExecutionReadSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeExecutionRead,
    getExecutionSnapshot,
    getExecutionSnapshot,
  );
  return selector(snapshot);
}

export const executionRead: ExecutionReadCapability = {
  getSnapshot: getExecutionSnapshot,
  subscribe: subscribeExecutionRead,
};
