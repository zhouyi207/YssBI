import {
  createReadProjection,
  useReadProjection,
  shareProjection,
} from "@/features/core/state/readProjection";

import { type DeepReadonly } from "@/shared/types/deepReadonly";
import { useExecutionStore } from "./useExecutionStore";
import type {
  ExecutionStatus,
  GraphExecutionState,
  RunFailureProjection,
} from "@/features/core/execution/executionTypes";

export interface GraphExecutionProjection {
  readonly status: ExecutionStatus;
  readonly runId: string | null;
  readonly runFailure: DeepReadonly<RunFailureProjection> | null;
}

export interface ExecutionReadSnapshot {
  readonly graphs: DeepReadonly<Record<string, GraphExecutionProjection>>;
}

const graphViews = new WeakMap<GraphExecutionState, GraphExecutionProjection>();
function projectGraph(graph: GraphExecutionState): GraphExecutionProjection {
  const existing = graphViews.get(graph);
  if (existing) return existing;
  const projection: GraphExecutionProjection = {
    status: graph.status,
    runId: graph.runId,
    runFailure: graph.runFailure,
  };
  graphViews.set(graph, projection);
  return projection;
}

let previousGraphs: ExecutionReadSnapshot["graphs"] = {};
function buildSnapshot(): DeepReadonly<ExecutionReadSnapshot> {
  const { graphs } = useExecutionStore.getState();
  previousGraphs = shareProjection(
    previousGraphs,
    Object.fromEntries(Object.entries(graphs).map(([path, graph]) => [path, projectGraph(graph)])),
  );
  return { graphs: previousGraphs };
}

const projection = createReadProjection(buildSnapshot, [useExecutionStore]);
export function useExecutionRead<T>(
  selector: (snapshot: DeepReadonly<ExecutionReadSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}
