import { shallow } from "zustand/shallow";
import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { isCurrentGraphRun, isGraphRunResultPending } from "@/features/core/graph/read";
import { useExecutionStore } from "@/features/core/execution";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { GraphResultState } from "@/shared/types/domain/result";
import {
  projectGraphPresentation,
  projectGraphResultCache,
  type GraphResultPresentation,
} from "./graphPresentation";

export function useGraphResultPresentation<T>(
  graphPath: string | undefined,
  selector: (presentation: DeepReadonly<GraphResultPresentation>) => T,
): T {
  return useReadProjection(graphPresentations, (snapshot) =>
    selector((graphPath ? snapshot.graphs[graphPath] : undefined) ?? EMPTY_GRAPH_PRESENTATION),
  );
}

const EMPTY_GRAPH_PRESENTATION = projectGraphPresentation(
  projectGraphResultCache(undefined),
  [],
  null,
);
const presentationInputs = new Map<
  string,
  {
    outputs: GraphResultState["outputs"] | undefined;
    connections: GraphResultState["connections"] | undefined;
    running: string[];
    pending: string[];
    failure: GraphResultPresentation["failure"];
    value: GraphResultPresentation;
  }
>();
let previousPresentations: Record<string, GraphResultPresentation> = {};
let previousPresentationSources: readonly unknown[] = [];
const graphPresentations = createReadProjection(() => {
  const { sessions, resultStates: caches } = useResourceStore.getState();
  const executions = useExecutionStore.getState().graphs;
  const sources = [caches, sessions, executions];
  if (shallow(previousPresentationSources, sources)) return { graphs: previousPresentations };
  previousPresentationSources = sources;
  const runningByGraph: Record<string, string[]> = {};
  const pendingByGraph: Record<string, string[]> = {};
  for (const execution of Object.values(executions))
    for (const run of Object.values(execution.outputRuns)) {
      if (!sessions[run.output.graphPath] || !isCurrentGraphRun(run.run)) continue;
      if (run.active) (runningByGraph[run.output.graphPath] ??= []).push(run.output.port.nodeId);
      if (isGraphRunResultPending(run.run, run.resultRevision))
        (pendingByGraph[run.output.graphPath] ??= []).push(portAddressKey(run.output.port));
    }
  const paths = new Set([
    ...Object.keys(caches),
    ...Object.keys(sessions),
    ...Object.keys(executions),
    ...Object.keys(runningByGraph),
  ]);
  const next: Record<string, GraphResultPresentation> = {};
  for (const path of paths) {
    const cached = caches[path];
    const cache =
      cached?.semanticInputHash === sessions[path]?.semanticInputHash ? cached : undefined;
    const running = runningByGraph[path] ?? [];
    const pending = pendingByGraph[path] ?? [];
    const recordedFailure = executions[path]?.runFailure;
    const failure =
      recordedFailure && isCurrentGraphRun(recordedFailure.run) ? recordedFailure : null;
    const previous = presentationInputs.get(path);
    const sameResults =
      previous &&
      previous.outputs === cache?.outputs &&
      previous.connections === cache?.connections &&
      shallow(previous.pending, pending);
    if (
      previous &&
      sameResults &&
      previous.failure === failure &&
      shallow(previous.running, running)
    ) {
      next[path] = previous.value;
    } else {
      const value = projectGraphPresentation(
        sameResults
          ? previous.value
          : projectGraphResultCache(cache, new Set(pending), previous?.value),
        running,
        failure,
        previous?.value,
      );
      presentationInputs.set(path, {
        outputs: cache?.outputs,
        connections: cache?.connections,
        running,
        pending,
        failure,
        value,
      });
      next[path] = value;
    }
  }
  for (const path of presentationInputs.keys())
    if (!paths.has(path)) presentationInputs.delete(path);
  if (!shallow(previousPresentations, next)) previousPresentations = next;
  return { graphs: previousPresentations };
}, [useResourceStore, useExecutionStore]);
