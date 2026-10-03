import { castDraft, produce } from "immer";
import { shallow } from "zustand/shallow";
import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";

import { type DeepReadonly } from "@/shared/types/deepReadonly";
import { useExecutionStore } from "./useExecutionStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { isCurrentGraphRun } from "@/features/core/graph/read";
import type {
  ExecutionStatus,
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

let previousGraphs: ExecutionReadSnapshot["graphs"] = {};
let previousSources: readonly unknown[] = [];
function buildSnapshot(): DeepReadonly<ExecutionReadSnapshot> {
  const { graphs } = useExecutionStore.getState();
  const { sessions, resultStates } = useResourceStore.getState();
  const sources = [graphs, sessions, resultStates];
  if (shallow(previousSources, sources)) return { graphs: previousGraphs };
  previousGraphs = produce(previousGraphs, (draft) => {
    for (const path in graphs) {
      const graph = graphs[path];
      const runFailure =
        graph.runFailure && isCurrentGraphRun(graph.runFailure.run) ? graph.runFailure : null;
      const runId = graph.run?.runId ?? null;
      const previous = previousGraphs[path];
      if (
        previous?.status === graph.status &&
        previous.runId === runId &&
        previous.runFailure === runFailure
      )
        continue;
      draft[path] = { status: graph.status, runId, runFailure: castDraft(runFailure) };
    }
    for (const path in draft) if (!graphs[path]) delete draft[path];
  });
  previousSources = sources;
  return { graphs: previousGraphs };
}

const projection = createReadProjection(buildSnapshot, [useExecutionStore, useResourceStore]);
export function useExecutionRead<T>(
  selector: (snapshot: DeepReadonly<ExecutionReadSnapshot>) => T,
): T {
  return useReadProjection(projection, selector);
}
