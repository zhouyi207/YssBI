import type { GraphEditorState } from "@/features/core/dataStore/graphProjectionStore";
import { shallow } from "zustand/shallow";
import { createReadProjection, useReadProjection } from "@/features/core/state/readProjection";
import { publishResultSessionEnd } from "@/services/result/resultSessionChannel";
import { resultReferenceKey, type ResultReference } from "@/shared/types/domain/result";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";

import { useExecutionStore } from "@/features/core/execution";
import { graphOutputKey, portAddressKey } from "@/features/domain/editorProjection";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { createBoundApplicationStore } from "@/features/core/state/applicationStore";

import { ResultService } from "@/services/result/resultService";
import { toErrorReference } from "@/features/application/errorReference";
import { captureProjectLifecycleState } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { ErrorReference } from "@/features/application/errorReference";
import type { ResultDescriptor, ResultPage, ResultValue } from "./types";
import type { ResultAnalysis } from "@/shared/types/domain/resultReport";
import {
  createResultQueryCoordinator,
  resultPageKey,
  resultAnalysisKey,
  type ResultAnalysisQuery,
  type ResultPageRequest,
  type ResultPinRequest,
  type ResultQueryReadCapability,
  type ResultQueryScope,
  type ResultGraphStateRequest,
} from "./resultQueryCoordinator";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { GraphResultState } from "@/shared/types/domain/result";
import { projectGraphPresentation, type GraphResultPresentation } from "./graphPresentation";

interface OutputRun {
  runId: string;
  request: ResultPinRequest;
  active: boolean;
  pendingState: boolean;
  semanticInputHash: string | undefined;
}

interface ResultProjectionState {
  readonly runs: Readonly<Record<string, OutputRun>>;
  readonly descriptors: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly values: Record<string, DeepReadonly<ResultValue | null>>;
  readonly pages: Record<string, DeepReadonly<ResultPage | null>>;
  readonly analyses: Record<string, DeepReadonly<ResultAnalysis | null>>;
  readonly pinResults: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly failures: Record<string, DeepReadonly<ErrorReference>>;
}

const emptyState: ResultProjectionState = {
  runs: {},
  descriptors: {},
  values: {},
  pages: {},
  analyses: {},
  pinResults: {},
  failures: {},
};

const resultProjection = createBoundApplicationStore<ResultProjectionState>(() => emptyState);

export function useCurrentResultDescriptors() {
  return resultProjection((state) => state.pinResults);
}

export function useGraphResultPresentation<T>(
  graphPath: string | undefined,
  selector: (presentation: DeepReadonly<GraphResultPresentation>) => T,
): T {
  return useReadProjection(graphPresentations, (snapshot) =>
    selector((graphPath ? snapshot.graphs[graphPath] : undefined) ?? EMPTY_GRAPH_PRESENTATION),
  );
}

function pinResultKey(request: ResultPinRequest): string {
  return graphOutputKey({ graphPath: request.graphPath, port: request.output });
}

function scopeKey(scope: ResultQueryScope): string {
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

const resultQueryPublication = {
  publishGraphState,
  releasePayload(reference: ResultReference) {
    const key = resultReferenceKey(reference);
    resultProjection.setState((state) => {
      const descriptors = { ...state.descriptors };
      if (
        !Object.values(state.pinResults).some((value) => value && resultReferenceKey(value) === key)
      )
        delete descriptors[key];
      return {
        ...state,
        descriptors,
        values: Object.fromEntries(Object.entries(state.values).filter(([id]) => id !== key)),
        pages: Object.fromEntries(
          Object.entries(state.pages).filter(([id]) => !id.startsWith(`${key}:`)),
        ),
        analyses: Object.fromEntries(
          Object.entries(state.analyses).filter(([id]) => !id.startsWith(`${key}:`)),
        ),
        failures: Object.fromEntries(
          Object.entries(state.failures).filter(
            ([id]) =>
              id !== `value:${key}` &&
              !id.startsWith(`page:${key}:`) &&
              !id.startsWith(`analysis:${key}:`),
          ),
        ),
      };
    });
  },
  publishDescriptor(reference: ResultReference, descriptor: DeepReadonly<ResultDescriptor | null>) {
    if (!descriptor) {
      resetResultQuery(reference);
      return;
    }
    // Reading a retained snapshot must never replace the pin's current result.
    resultProjection.setState((state) => ({
      ...state,
      descriptors: { ...state.descriptors, [resultReferenceKey(reference)]: descriptor },
    }));
  },
  publishValue(reference: ResultReference, value: DeepReadonly<ResultValue | null>) {
    resultProjection.setState((state) => ({
      ...state,
      values: { ...state.values, [resultReferenceKey(reference)]: value },
      failures: Object.fromEntries(
        Object.entries(state.failures).filter(
          ([key]) => key !== `value:${resultReferenceKey(reference)}`,
        ),
      ),
    }));
  },
  publishPage(request: ResultPageRequest, page: DeepReadonly<ResultPage | null>) {
    resultProjection.setState((state) => ({
      ...state,
      pages: { ...state.pages, [resultPageKey(request)]: page },
      failures: Object.fromEntries(
        Object.entries(state.failures).filter(([key]) => key !== `page:${resultPageKey(request)}`),
      ),
    }));
  },
  publishAnalysis(request: ResultAnalysisQuery, value: DeepReadonly<ResultAnalysis | null>) {
    resultProjection.setState((state) => ({
      ...state,
      analyses: {
        ...state.analyses,
        [resultAnalysisKey(request)]: value,
      },
      failures: Object.fromEntries(
        Object.entries(state.failures).filter(
          ([key]) => key !== `analysis:${resultAnalysisKey(request)}`,
        ),
      ),
    }));
  },
  publishPinResult(request: ResultPinRequest, result: DeepReadonly<ResultDescriptor | null>) {
    publishCurrentResult(request, result);
    if (!result) scheduleGraphStateRefresh(request.graphPath);
  },
  publishFailure(scope: ResultQueryScope, issue: ErrorReference) {
    if (scope.kind === "graphState" && !currentGraphStateRequest(scope)) return;
    resultProjection.setState((state) => ({
      ...state,
      failures: { ...state.failures, [scopeKey(scope)]: issue },
    }));
  },
};

export const resultQueryRead: ResultQueryReadCapability = {
  subscribe: (listener) => {
    const unsubscribe = resultProjection.subscribe(() => listener());
    return unsubscribe;
  },
  getDescriptor: (reference) =>
    resultProjection.getState().descriptors[resultReferenceKey(reference)] ?? null,
  getValue: (reference) =>
    resultProjection.getState().values[resultReferenceKey(reference)] ?? null,
  getPage: (request) => {
    const page = resultProjection.getState().pages[resultPageKey(request)];
    return page?.offset === request.offset && page.requestedLimit === request.limit ? page : null;
  },
  getAnalysis: (request) =>
    resultProjection.getState().analyses[resultAnalysisKey(request)] ?? null,
  getPinResult: (request) => resultProjection.getState().pinResults[pinResultKey(request)] ?? null,
  getFailure: (scope) => resultProjection.getState().failures[scopeKey(scope)] ?? null,
};

export const resultQueryCoordinator = createResultQueryCoordinator({
  readCurrentProjectInstanceId: () => captureProjectLifecycleState().projectInstanceId,
  service: {
    getGraphState: (graphPath, hash) => ResultService.getGraphState(graphPath, hash),
    getDescriptor: (reference) => ResultService.getDescriptor(reference),
    getValue: (reference) => ResultService.getValue(reference),
    getPage: (reference, offset, limit, table) =>
      ResultService.getPage(reference, offset, limit, table),
    analyze: (reference, analysis) => ResultService.analyze(reference, analysis),
    getPinResult: (graphPath, output) => ResultService.getPinResult(graphPath, output),
  },
  publication: resultQueryPublication,
  toErrorReference,
});

export function resetResultQueryProject(): void {
  publishResultSessionEnd(resultExecutionSessionId);
  resultQueryCoordinator.resetProject();
  pendingGraphRefreshes.clear();
  resultExecutionSessionId = null;
  resultProjection.setState(emptyState);
}

export function resetResultQuery(reference: ResultReference): void {
  const key = resultReferenceKey(reference);
  resultQueryCoordinator.resetResult(reference);
  resultProjection.setState((state) => ({
    ...state,
    descriptors: Object.fromEntries(Object.entries(state.descriptors).filter(([id]) => id !== key)),
    values: Object.fromEntries(Object.entries(state.values).filter(([id]) => id !== key)),
    pages: Object.fromEntries(
      Object.entries(state.pages).filter(([id]) => !id.startsWith(`${key}:`)),
    ),
    analyses: Object.fromEntries(
      Object.entries(state.analyses).filter(([id]) => !id.startsWith(`${key}:`)),
    ),
    failures: Object.fromEntries(
      Object.entries(state.failures).filter(
        ([id]) =>
          id !== `descriptor:${key}` &&
          id !== `value:${key}` &&
          !id.startsWith(`page:${key}:`) &&
          !id.startsWith(`analysis:${key}:`),
      ),
    ),
    pinResults: Object.fromEntries(
      Object.entries(state.pinResults).map(([id, value]) => [
        id,
        value && resultReferenceKey(value) === key ? null : value,
      ]),
    ),
  }));
}

let resultExecutionSessionId: string | null = null;

const pendingGraphRefreshes = new Set<string>();

const EMPTY_GRAPH_PRESENTATION = projectGraphPresentation(undefined, [], null);
const presentationInputs = new Map<
  string,
  {
    cache: GraphResultState | undefined;
    running: string[];
    pending: string[];
    failure: GraphResultPresentation["failure"];
    value: GraphResultPresentation;
  }
>();
let previousPresentations: Record<string, GraphResultPresentation> = {};
let previousPresentationSources: readonly unknown[] = [];
const graphPresentations = createReadProjection(() => {
  const { runs } = resultProjection.getState();
  const { sessions, resultStates: caches } = useGraphProjectionStore.getState();
  const executions = useExecutionStore.getState().graphs;
  const sources = [caches, sessions, executions, runs];
  if (shallow(previousPresentationSources, sources)) return { graphs: previousPresentations };
  previousPresentationSources = sources;
  const runningByGraph: Record<string, string[]> = {};
  const pendingByGraph: Record<string, string[]> = {};
  for (const run of Object.values(runs)) {
    if (run.semanticInputHash !== sessions[run.request.graphPath]?.semanticInputHash) continue;
    if (run.active) (runningByGraph[run.request.graphPath] ??= []).push(run.request.output.nodeId);
    if (run.pendingState)
      (pendingByGraph[run.request.graphPath] ??= []).push(portAddressKey(run.request.output));
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
    const failure = executions[path]?.runFailure ?? null;
    const previous = presentationInputs.get(path);
    if (
      previous &&
      previous.cache === cache &&
      previous.failure === failure &&
      shallow(previous.running, running) &&
      shallow(previous.pending, pending)
    ) {
      next[path] = previous.value;
    } else {
      const value = projectGraphPresentation(
        cache,
        running,
        failure,
        previous?.value,
        new Set(pending),
      );
      presentationInputs.set(path, { cache, running, pending, failure, value });
      next[path] = value;
    }
  }
  for (const path of presentationInputs.keys())
    if (!paths.has(path)) presentationInputs.delete(path);
  if (!shallow(previousPresentations, next)) previousPresentations = next;
  return { graphs: previousPresentations };
}, [resultProjection, useGraphProjectionStore, useExecutionStore]);

function currentGraphStateRequest(request: ResultGraphStateRequest): boolean {
  const current = useGraphProjectionStore.getState().sessions[request.graphPath];
  return Boolean(
    current &&
    current.sessionId === request.sessionId &&
    current.projectionGeneration === request.projectionGeneration &&
    current.semanticInputHash === request.semanticInputHash,
  );
}

function scheduleGraphStateRefresh(graphPath: string): void {
  const alreadyQueued = pendingGraphRefreshes.size > 0;
  pendingGraphRefreshes.add(graphPath);
  if (alreadyQueued) return;
  queueMicrotask(() => {
    const graphs = [...pendingGraphRefreshes];
    pendingGraphRefreshes.clear();
    if (!captureProjectLifecycleState().projectInstanceId) return;
    for (const graphPath of graphs) {
      const session = useGraphProjectionStore.getState().sessions[graphPath];
      if (!session) continue;
      void resultQueryCoordinator.loadGraphState({
        graphPath,
        semanticInputHash: session.semanticInputHash,
        sessionId: session.sessionId,
        projectionGeneration: session.projectionGeneration,
      });
    }
  });
}

function publishGraphState(
  request: ResultGraphStateRequest,
  projection: DeepReadonly<GraphResultState | null>,
): void {
  if (!currentGraphStateRequest(request)) return;
  // No matching backend basis is not a replacement for the currently displayed frame.
  if (!projection) return;
  const graphPath = request.graphPath;
  if (resultExecutionSessionId && resultExecutionSessionId !== projection.executionSessionId)
    resetResultQueryProject();
  resultExecutionSessionId = projection.executionSessionId;
  useGraphProjectionStore.getState().setResultState(graphPath, projection);
  resultProjection.setState((state) => {
    const pending = Object.entries(state.runs).filter(
      ([, run]) => run.request.graphPath === graphPath && run.pendingState,
    );
    if (!pending.length) return state;
    return {
      ...state,
      runs: {
        ...state.runs,
        ...Object.fromEntries(pending.map(([key, run]) => [key, { ...run, pendingState: false }])),
      },
    };
  });
  reconcileCurrentResults(
    graphPath,
    useGraphProjectionStore.getState().resultStates[graphPath] ?? null,
  );
}

function reconcileCurrentResults(
  graphPath: string,
  projection: DeepReadonly<GraphResultState | null>,
): void {
  const outputs = new Map(
    (projection?.outputs ?? []).map((entry) => [graphOutputKey(entry.output), entry]),
  );
  const previous = Object.values(resultProjection.getState().pinResults).filter(
    (value) => value?.provenance.output?.graphPath === graphPath,
  );
  invalidateOutputs(
    previous.flatMap((value) => {
      if (!value?.provenance.output) return [];
      const output = value.provenance.output;
      const current = outputs.get(graphOutputKey(output));
      return current?.state === "valid" &&
        current.resultId === value.resultId &&
        value.executionSessionId === projection?.executionSessionId
        ? []
        : [{ graphPath, output: output.port }];
    }),
  );
  resultProjection.setState((state) => {
    const key = `graphState:${graphPath}`;
    if (!state.failures[key]) return state;
    const failures = { ...state.failures };
    delete failures[key];
    return { ...state, failures };
  });
  for (const { output, state, resultId } of projection?.outputs ?? []) {
    const key = graphOutputKey(output);
    if (state === "valid" && resultProjection.getState().pinResults[key]?.resultId !== resultId)
      void resultQueryCoordinator.loadPinResult({ graphPath, output: output.port });
  }
}

/** The publication owner calls this after installing an entire graph frame. */
export function reconcileGraphResultQueries(graphPath: string, previous?: GraphEditorState): void {
  const { sessions, resultStates } = useGraphProjectionStore.getState();
  const current = sessions[graphPath];
  if (!current) {
    resetGraphResultQueries(graphPath);
    return;
  }
  resultQueryCoordinator.resetGraphState(graphPath);
  pendingGraphRefreshes.delete(graphPath);
  const resultState = resultStates[graphPath];
  if (resultState) {
    if (resultExecutionSessionId && resultExecutionSessionId !== resultState.executionSessionId)
      resetResultQueryProject();
    resultExecutionSessionId = resultState.executionSessionId;
  }
  if (
    previous &&
    (previous.semanticInputHash !== current.semanticInputHash ||
      previous.sessionId !== current.sessionId)
  ) {
    const obsolete = new Set(
      Object.entries(resultProjection.getState().runs)
        .filter(([, run]) => run.request.graphPath === graphPath)
        .map(([key]) => key),
    );
    if (obsolete.size)
      resultProjection.setState((state) => ({
        ...state,
        runs: Object.fromEntries(Object.entries(state.runs).filter(([key]) => !obsolete.has(key))),
      }));
    useExecutionStore.getState().clearGraphRunProjections(graphPath);
  }
  reconcileCurrentResults(graphPath, resultState ?? null);
  if (
    Object.values(resultProjection.getState().runs).some(
      (run) => run.request.graphPath === graphPath && run.pendingState,
    )
  )
    scheduleGraphStateRefresh(graphPath);
}

function publishCurrentResult(
  request: ResultPinRequest,
  result: DeepReadonly<ResultDescriptor | null>,
): void {
  const key = pinResultKey(request);
  const previous = resultProjection.getState().pinResults[key];
  if (result) {
    const draft = useGraphProjectionStore.getState().sessions[request.graphPath];
    if (draft) {
      const cache = useGraphProjectionStore.getState().resultStates[request.graphPath];
      const current = cache?.outputs.find((entry) => graphOutputKey(entry.output) === key);
      if (
        cache?.semanticInputHash !== draft.semanticInputHash ||
        cache.executionSessionId !== result.executionSessionId ||
        current?.state !== "valid" ||
        current?.resultId !== result.resultId
      )
        return;
    }
    const pending = resultProjection.getState().runs[key];
    if (pending && BigInt(pending.runId) > BigInt(result.provenance.runId)) return;
  }
  if (
    previous &&
    (!result || resultReferenceKey(previous) !== resultReferenceKey(result)) &&
    !resultQueryCoordinator.isPayloadRetained(previous)
  )
    resetResultQuery(previous);
  resultProjection.setState((state) => ({
    ...state,
    pinResults: result
      ? { ...state.pinResults, [key]: result }
      : Object.fromEntries(
          Object.entries(state.pinResults).filter(([existing]) => existing !== key),
        ),
    descriptors: result
      ? { ...state.descriptors, [resultReferenceKey(result)]: result }
      : state.descriptors,
  }));
}

function invalidateOutputs(requests: readonly ResultPinRequest[]): void {
  const projectInstanceId = captureProjectLifecycleState().projectInstanceId;
  if (!projectInstanceId) return;
  for (const request of requests) {
    resultQueryCoordinator.resetPinResult(request);
    publishCurrentResult(request, null);
  }
}

export function resetGraphResultQueries(graphPath: string): void {
  pendingGraphRefreshes.delete(graphPath);
  resultQueryCoordinator.resetGraphState(graphPath);
  const requests = new Map<string, ResultPinRequest>();
  for (const [key, pending] of Object.entries(resultProjection.getState().runs)) {
    if (pending.request.graphPath === graphPath) {
      requests.set(key, pending.request);
    }
  }
  for (const result of Object.values(resultProjection.getState().pinResults)) {
    const output = result?.provenance.output;
    if (output?.graphPath === graphPath) {
      const request = { graphPath, output: output.port };
      requests.set(pinResultKey(request), request);
    }
  }
  invalidateOutputs([...requests.values()]);
  resultProjection.setState((state) => ({
    ...state,
    runs: Object.fromEntries(
      Object.entries(state.runs).filter(([, run]) => run.request.graphPath !== graphPath),
    ),
    pinResults: Object.fromEntries(
      Object.entries(state.pinResults).filter(([key]) => !requests.has(key)),
    ),
  }));
}

export function observeResultRunEvent(event: RunEvent): void {
  if (event.kind.type === "runStarted") {
    if (
      resultExecutionSessionId !== null &&
      resultExecutionSessionId !== event.run.executionSessionId
    )
      resetResultQueryProject();
    resultExecutionSessionId = event.run.executionSessionId;
    const requests = event.kind.outputs
      .map((output) => ({ graphPath: output.graphPath, output: output.port }))
      .filter((request) => {
        const owner = resultProjection.getState().runs[pinResultKey(request)];
        return !owner || BigInt(owner.runId) < BigInt(event.run.runId);
      });
    if (requests.length === 0) return;
    invalidateOutputs(requests);
    resultProjection.setState((state) => ({
      ...state,
      runs: {
        ...state.runs,
        ...Object.fromEntries(
          requests.map((request) => [
            pinResultKey(request),
            {
              runId: event.run.runId,
              request,
              active: true,
              pendingState: true,
              semanticInputHash:
                useGraphProjectionStore.getState().sessions[request.graphPath]?.semanticInputHash,
            },
          ]),
        ),
      },
    }));
  } else if (["runCompleted", "runErrored", "runCancelled"].includes(event.kind.type)) {
    if (event.run.executionSessionId !== resultExecutionSessionId) return;
    const terminalRuns: Record<string, OutputRun> = {};
    for (const [key, pending] of Object.entries(resultProjection.getState().runs)) {
      if (pending.runId !== event.run.runId || !pending.active) continue;
      terminalRuns[key] = { ...pending, active: false, pendingState: true };
      if (
        event.kind.type === "runCompleted" &&
        !useGraphProjectionStore.getState().sessions[event.run.graphPath]
      )
        void resultQueryCoordinator.loadPinResult(pending.request);
    }
    if (Object.keys(terminalRuns).length > 0)
      resultProjection.setState((state) => ({
        ...state,
        runs: { ...state.runs, ...terminalRuns },
      }));
  }
  resultQueryCoordinator.resetGraphState(event.run.graphPath);
  scheduleGraphStateRefresh(event.run.graphPath);
}
