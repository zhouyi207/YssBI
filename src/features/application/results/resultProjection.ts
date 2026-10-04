import type { Draft } from "immer";
import { shallow } from "zustand/shallow";
import { createBoundApplicationStore } from "@/features/core/state/applicationStore";
import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { type GraphProjectionData } from "@/features/core/dataStore/graphProjection";
import { graphOutputKey } from "@/features/domain/editorProjection";
import type { ErrorReference } from "@/features/application/errorReference";
import {
  resultReferenceKey,
  type GraphResultState,
  type ResultReference,
} from "@/shared/types/domain/result";
import {
  inspectableRefsFromPinView,
  type ResolvePinViewTargetParams,
} from "@/features/core/execution/pinViewTarget";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ResultAnalysis } from "@/shared/types/domain/resultReport";
import type { ResultDescriptor, ResultPage, ResultValue } from "./types";
import { createPinResultSearchProjector, type PinResultSearchEntry } from "./pinResultSearch";
import { useExecutionStore } from "@/features/core/execution/useExecutionStore";
import type { ExecutionState } from "@/features/core/execution/executionTypes";
import { isCurrentGraphRun } from "@/features/core/graph/read";
import {
  resultPageKey,
  resultAnalysisKey,
  type ResultPinRequest,
  type ResultQueryReadCapability,
  type ResultQueryScope,
} from "./resultQueryCoordinator";

export interface ResultProjectionState {
  readonly descriptors: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly values: Record<string, DeepReadonly<ResultValue | null>>;
  readonly pages: Record<string, DeepReadonly<ResultPage | null>>;
  readonly analyses: Record<string, DeepReadonly<ResultAnalysis | null>>;
  readonly pinResults: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly failures: Record<string, DeepReadonly<ErrorReference>>;
}

export const emptyState: ResultProjectionState = {
  descriptors: {},
  values: {},
  pages: {},
  analyses: {},
  pinResults: {},
  failures: {},
};

export const resultProjection = createBoundApplicationStore<ResultProjectionState>(
  () => emptyState,
);

export function pinResultKey(request: ResultPinRequest): string {
  return graphOutputKey({ graphPath: request.graphPath, port: request.output });
}

export function scopeKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "graphState":
      return `graphState:${scope.graphPath}`;
    case "descriptor":
    case "value":
      return `${scope.kind}:${resultReferenceKey(scope)}`;
    case "page":
      return `page:${resultPageKey(scope)}`;
    case "analysis":
      return `analysis:${resultAnalysisKey(scope)}`;
    case "pinResult":
      return `pinResult:${pinResultKey(scope)}`;
  }
}

const outputIndexes = new WeakMap<
  GraphResultState["outputs"],
  ReadonlyMap<string, GraphResultState["outputs"][number]>
>();

/** Only published, immutable graph summaries reach this index. */
function indexResultOutputs(outputs: GraphResultState["outputs"]) {
  let index = outputIndexes.get(outputs);
  if (!index) {
    index = new Map(outputs.map((entry) => [graphOutputKey(entry.output), entry]));
    outputIndexes.set(outputs, index);
  }
  return index;
}

export function matchesResultOutput(
  summary: GraphResultState | null | undefined,
  key: string,
  result: DeepReadonly<ResultDescriptor>,
): boolean {
  if (!summary || summary.executionSessionId !== result.executionSessionId) return false;
  const output = indexResultOutputs(summary.outputs).get(key);
  return output?.state === "valid" && output.resultId === result.resultId;
}

type GraphResultRead = Pick<GraphProjectionData, "sessions" | "resultStates" | "graphEntities">;

/** Explicit previous-value intent; these references never become current Pin bindings. */
export function stalePinResultReferences(
  params: ResolvePinViewTargetParams,
  snapshot: GraphResultRead,
): ResultReference[] {
  const summary = snapshot.resultStates[params.graphPath];
  const session = snapshot.sessions[params.graphPath];
  if (!session || !summary || summary.semanticInputHash !== session.semanticInputHash) return [];
  const outputs = indexResultOutputs(summary.outputs);
  return inspectableRefsFromPinView(params, snapshot.graphEntities[params.graphPath]).flatMap(
    (ref) => {
      if (ref.kind !== "outputPin") return [];
      const output = outputs.get(graphOutputKey({ graphPath: ref.graphPath, port: ref.output }));
      return output?.state === "stale" && output.resultId
        ? [{ executionSessionId: summary.executionSessionId, resultId: output.resultId }]
        : [];
    },
  );
}

export function isCurrentPinResult(
  graphPath: string,
  key: string,
  result: DeepReadonly<ResultDescriptor>,
): boolean {
  const owner = useExecutionStore.getState().graphs[graphPath]?.outputRuns[key];
  if (
    owner &&
    isCurrentGraphRun(owner.run) &&
    (owner.run.executionSessionId !== result.executionSessionId ||
      BigInt(owner.run.runId) > BigInt(result.provenance.runId))
  )
    return false;
  const { sessions, resultStates } = useResourceStore.getState();
  const session = sessions[graphPath];
  // Current output queries also support graphs without a mounted editor session.
  if (!session) return true;
  const summary = resultStates[graphPath];
  return (
    summary?.semanticInputHash === session.semanticInputHash &&
    matchesResultOutput(summary, key, result)
  );
}

let previousBindings: ResultProjectionState["pinResults"] | undefined;
let previousSummaries: GraphProjectionData["resultStates"] | undefined;
let previousExecutions: ExecutionState["graphs"] | undefined;
let currentBindings: ResultProjectionState["pinResults"] = {};
let currentResultsByGraph: Record<string, DeepReadonly<ResultDescriptor>[]> = {};
let searchEntriesByGraph: Record<string, PinResultSearchEntry[]> = {};
let previousSearchResults: typeof currentResultsByGraph | undefined;
let previousSearchEntities: GraphProjectionData["graphEntities"] | undefined;
const searchProjectors = new Map<string, ReturnType<typeof createPinResultSearchProjector>>();
const emptySearchEntries: readonly PinResultSearchEntry[] = [];
const readProjection = createReadProjection(() => {
  const state = resultProjection.getState();
  const summaries = useResourceStore.getState().resultStates;
  const executions = useExecutionStore.getState().graphs;
  const outputOwnersChanged =
    previousExecutions !== executions &&
    (!previousExecutions ||
      Object.keys(previousExecutions).length !== Object.keys(executions).length ||
      Object.entries(executions).some(
        ([path, graph]) => graph.outputRuns !== previousExecutions?.[path]?.outputRuns,
      ));
  // The graph owner installs/removes session and result identities together. Saving flags
  // and unrelated payload updates do not change current-output eligibility.
  if (
    previousBindings !== state.pinResults ||
    previousSummaries !== summaries ||
    outputOwnersChanged
  ) {
    const next: ResultProjectionState["pinResults"] = {};
    for (const [key, result] of Object.entries(state.pinResults))
      if (result && isCurrentPinResult(result.provenance.graphPath, key, result))
        next[key] = result;
    if (!shallow(currentBindings, next)) {
      currentBindings = next;
      const byGraph: typeof currentResultsByGraph = {};
      for (const result of Object.values(next)) {
        const path = result?.provenance.output?.graphPath;
        if (result && path) (byGraph[path] ??= []).push(result);
      }
      for (const path in byGraph) {
        if (shallow(byGraph[path], currentResultsByGraph[path]))
          byGraph[path] = currentResultsByGraph[path];
      }
      currentResultsByGraph = byGraph;
    }
    previousBindings = state.pinResults;
    previousSummaries = summaries;
  }
  previousExecutions = executions;
  const entities = useResourceStore.getState().graphEntities;
  if (previousSearchResults !== currentResultsByGraph || previousSearchEntities !== entities) {
    const nextEntries: typeof searchEntriesByGraph = {};
    for (const path in currentResultsByGraph) {
      let projectEntries = searchProjectors.get(path);
      if (!projectEntries) {
        projectEntries = createPinResultSearchProjector();
        searchProjectors.set(path, projectEntries);
      }
      nextEntries[path] = projectEntries(currentResultsByGraph[path], entities[path]);
    }
    for (const path of searchProjectors.keys()) {
      if (!currentResultsByGraph[path]) searchProjectors.delete(path);
    }
    if (!shallow(searchEntriesByGraph, nextEntries)) searchEntriesByGraph = nextEntries;
    previousSearchResults = currentResultsByGraph;
    previousSearchEntities = entities;
  }
  return { ...state, pinResults: currentBindings, searchEntriesByGraph };
}, [resultProjection, useResourceStore, useExecutionStore]);

export function usePinResultSearchEntries(graphPath: string) {
  return useReadProjection(
    readProjection,
    (state) => state.searchEntriesByGraph[graphPath] ?? emptySearchEntries,
  );
}

export function useCurrentPinResult(request: ResultPinRequest) {
  const key = pinResultKey(request);
  return useReadProjection(readProjection, (state) => state.pinResults[key] ?? null);
}

export const resultQueryRead: ResultQueryReadCapability = {
  subscribe: readProjection.subscribe,
  getDescriptor: (reference) =>
    resultProjection.getState().descriptors[resultReferenceKey(reference)] ?? null,
  getValue: (reference) =>
    resultProjection.getState().values[resultReferenceKey(reference)] ?? null,
  getPage: (request) => resultProjection.getState().pages[resultPageKey(request)] ?? null,
  getAnalysis: (request) =>
    resultProjection.getState().analyses[resultAnalysisKey(request)] ?? null,
  getPinResult: (request) => {
    const key = pinResultKey(request);
    const result = resultProjection.getState().pinResults[key];
    // Recheck the source for synchronous graph listeners that run before read projection refresh.
    return result && isCurrentPinResult(request.graphPath, key, result) ? result : null;
  },
  getFailure: (scope) => resultProjection.getState().failures[scopeKey(scope)] ?? null,
};

export function removeResultPayload(state: Draft<ResultProjectionState>, key: string): void {
  delete state.values[key];
  for (const id of Object.keys(state.pages)) if (id.startsWith(`${key}:`)) delete state.pages[id];
  for (const id of Object.keys(state.analyses))
    if (id.startsWith(`${key}:`)) delete state.analyses[id];
  delete state.failures[`value:${key}`];
  for (const id of Object.keys(state.failures)) {
    if (id.startsWith(`page:${key}:`) || id.startsWith(`analysis:${key}:`))
      delete state.failures[id];
  }
}

export function removeResultProjection(state: Draft<ResultProjectionState>, key: string): void {
  removeResultPayload(state, key);
  delete state.descriptors[key];
  delete state.failures[`descriptor:${key}`];
  for (const [id, value] of Object.entries(state.pinResults)) {
    if (value && resultReferenceKey(value) === key) state.pinResults[id] = null;
  }
}
