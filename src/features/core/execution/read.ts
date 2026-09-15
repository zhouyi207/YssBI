import { useSyncExternalStore } from "react";

import { freezeProjectionSnapshot, type DeepReadonly } from "@/shared/types/deepReadonly";
import { useExecutionStore } from "./useExecutionStore";
import type {
  ExecutionStatus,
  GraphExecutionState,
  PinPreviewState,
  RunFailureProjection,
} from "@/features/core/execution/executionTypes";

export interface GraphExecutionProjection {
  readonly status: ExecutionStatus;
  readonly runId: string | null;
  readonly runFailure: DeepReadonly<RunFailureProjection> | null;
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
    runFailure: graph.runFailure,
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
