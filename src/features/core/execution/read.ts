import { useSyncExternalStore } from 'react';

import {
  freezeProjectionSnapshot,
  type DeepReadonly,
} from '@/features/core/projection/deepReadonly';
import { useExecutionStore } from './useExecutionStore';
import type {
  ExecutionStatus,
  GraphExecutionState,
  PinHistoryProjection,
  PinPreviewState,
  RunOutputProjection,
} from '@/shared/types/ui/execution';

export interface GraphExecutionProjection {
  readonly status: ExecutionStatus;
  readonly runId: string | null;
  readonly runOutput: DeepReadonly<RunOutputProjection>;
  readonly pinHistories: ReadonlyMap<string, DeepReadonly<PinHistoryProjection>>;
  readonly pinPreviews: ReadonlyMap<string, DeepReadonly<PinPreviewState>>;
}

export interface ExecutionReadSnapshot {
  readonly graphs: DeepReadonly<Record<string, GraphExecutionProjection>>;
}

export interface ExecutionReadCapability {
  readonly getSnapshot: () => DeepReadonly<ExecutionReadSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
}

function projectGraph(graph: GraphExecutionState): GraphExecutionProjection {
  return {
    status: graph.status,
    runId: graph.runId,
    runOutput: graph.runOutput,
    pinHistories: graph.pinHistories,
    pinPreviews: graph.pinPreviews,
  };
}

function buildSnapshot(): DeepReadonly<ExecutionReadSnapshot> {
  const { graphs } = useExecutionStore.getState();
  return freezeProjectionSnapshot({
    graphs: Object.fromEntries(
      Object.entries(graphs).map(([graphPath, graph]) => [graphPath, projectGraph(graph)]),
    ),
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
