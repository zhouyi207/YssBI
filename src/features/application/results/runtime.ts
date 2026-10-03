import { type GraphEditorState } from "@/features/core/dataStore/graphProjection";
import { publishResultSessionEnd } from "@/services/result/resultSessionChannel";
import { resultReferenceKey, type ResultReference } from "@/shared/types/domain/result";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { isCurrentGraphRun, isGraphRunResultPending } from "@/features/core/graph/read";

import { useExecutionStore } from "@/features/core/execution";
import { graphOutputKey } from "@/features/domain/editorProjection";
import type { RunEvent } from "@/shared/types/domain/runEvent";

import { ResultService } from "@/services/result/resultService";
import { toErrorReference } from "@/features/application/errorReference";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
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
  type ResultQueryScope,
  type ResultGraphStateRequest,
} from "./resultQueryCoordinator";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { GraphResultState } from "@/shared/types/domain/result";
import { castDraft, produce, type Draft } from "immer";
import {
  emptyState,
  resultProjection,
  pinResultKey,
  scopeKey,
  removeResultPayload,
  removeResultProjection,
  matchesResultOutput,
  isCurrentPinResult,
  type ResultProjectionState,
} from "./resultProjection";
export { resultQueryRead, usePinResultSearchEntries } from "./resultProjection";
export { useGraphResultPresentation } from "./graphPresentationRead";

const resultQueryPublication = {
  publishGraphState,
  releasePayload(reference: ResultReference) {
    const key = resultReferenceKey(reference);
    resultProjection.setState(
      produce((state: Draft<ResultProjectionState>) => {
        if (
          !Object.values(state.pinResults).some(
            (value) => value && resultReferenceKey(value) === key,
          )
        )
          delete state.descriptors[key];
        removeResultPayload(state, key);
      }),
    );
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
    const key = resultReferenceKey(reference);
    resultProjection.setState(
      produce((state: Draft<ResultProjectionState>) => {
        state.values[key] = castDraft(value);
        delete state.failures[`value:${key}`];
      }),
    );
  },
  publishPage(request: ResultPageRequest, page: DeepReadonly<ResultPage | null>) {
    const key = resultPageKey(request);
    resultProjection.setState(
      produce((state: Draft<ResultProjectionState>) => {
        state.pages[key] = castDraft(page);
        delete state.failures[`page:${key}`];
      }),
    );
  },
  publishAnalysis(request: ResultAnalysisQuery, value: DeepReadonly<ResultAnalysis | null>) {
    const key = resultAnalysisKey(request);
    resultProjection.setState(
      produce((state: Draft<ResultProjectionState>) => {
        state.analyses[key] = castDraft(value);
        delete state.failures[`analysis:${key}`];
      }),
    );
  },
  publishPinResult(request: ResultPinRequest, result: DeepReadonly<ResultDescriptor | null>) {
    const isCurrent = result ? undefined : captureGraphRefreshEligibility(request.graphPath);
    publishCurrentResult(request, result);
    if (isCurrent?.()) scheduleGraphStateRefresh(request.graphPath);
  },
  publishFailure(scope: ResultQueryScope, issue: ErrorReference) {
    if (scope.kind === "graphState" && !currentGraphStateRequest(scope)) return;
    resultProjection.setState((state) => ({
      ...state,
      failures: { ...state.failures, [scopeKey(scope)]: issue },
    }));
  },
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
  const owner = captureProjectLifecycleState();
  publishResultSessionEnd(resultExecutionSessionId);
  resultQueryCoordinator.resetProject();
  pendingGraphRefreshes.clear();
  resultExecutionSessionId = null;
  resultProjection.setState(emptyState);
  if (!isProjectLifecycleStateCurrent(owner) || resultExecutionSessionId !== null) return;
  useExecutionStore.getState().clearOutputRuns();
}

export function prepareResultExecutionSession(executionSessionId: string): boolean {
  const owner = captureProjectLifecycleState();
  if (resultExecutionSessionId && resultExecutionSessionId !== executionSessionId)
    resetResultQueryProject();
  if (
    !isProjectLifecycleStateCurrent(owner) ||
    (resultExecutionSessionId !== null && resultExecutionSessionId !== executionSessionId)
  )
    return false;
  resultExecutionSessionId = executionSessionId;
  return true;
}

export function resetResultQuery(reference: ResultReference): void {
  const key = resultReferenceKey(reference);
  resultQueryCoordinator.resetResult(reference);
  resultProjection.setState(
    produce((state: Draft<ResultProjectionState>) => {
      removeResultProjection(state, key);
    }),
  );
}

let resultExecutionSessionId: string | null = null;

const pendingGraphRefreshes = new Set<string>();

function currentGraphStateRequest(request: ResultGraphStateRequest): boolean {
  const current = useResourceStore.getState().sessions[request.graphPath];
  return Boolean(
    current &&
    current.sessionId === request.sessionId &&
    current.projectionGeneration === request.projectionGeneration &&
    current.semanticInputHash === request.semanticInputHash,
  );
}

function captureGraphRefreshEligibility(graphPath: string): () => boolean {
  const owner = captureProjectLifecycleState();
  const { sessions, resultStates } = useResourceStore.getState();
  const session = sessions[graphPath];
  const request = session && {
    graphPath,
    sessionId: session.sessionId,
    projectionGeneration: session.projectionGeneration,
    semanticInputHash: session.semanticInputHash,
  };
  const executionSessionId = resultExecutionSessionId;
  return () =>
    isProjectLifecycleStateCurrent(owner) &&
    resultExecutionSessionId === executionSessionId &&
    useResourceStore.getState().resultStates[graphPath] === resultStates[graphPath] &&
    (request
      ? currentGraphStateRequest(request)
      : !useResourceStore.getState().sessions[graphPath]);
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
      const session = useResourceStore.getState().sessions[graphPath];
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
  const owner = captureProjectLifecycleState();
  const isCurrent = () =>
    isProjectLifecycleStateCurrent(owner) &&
    currentGraphStateRequest(request) &&
    resultExecutionSessionId === projection.executionSessionId;
  const graphPath = request.graphPath;
  if (!prepareResultExecutionSession(projection.executionSessionId)) return;
  if (!isCurrent()) return;
  useResourceStore.getState().setGraphResultState(graphPath, projection);
  if (!isCurrent()) return;
  const resultState = useResourceStore.getState().resultStates[graphPath];
  if (
    resultState?.executionSessionId !== projection.executionSessionId ||
    resultState.revision !== projection.revision
  )
    return;
  reconcileCurrentResults(
    graphPath,
    resultState ?? null,
    () => isCurrent() && useResourceStore.getState().resultStates[graphPath] === resultState,
  );
}

function reconcileCurrentResults(
  graphPath: string,
  projection: DeepReadonly<GraphResultState | null>,
  isCurrent: () => boolean,
): void {
  const previous = Object.values(resultProjection.getState().pinResults).filter(
    (value) => value?.provenance.output?.graphPath === graphPath,
  );
  invalidateOutputs(
    previous.flatMap((value) => {
      if (!value?.provenance.output) return [];
      const output = value.provenance.output;
      return matchesResultOutput(projection, graphOutputKey(output), value)
        ? []
        : [{ graphPath, output: output.port }];
    }),
    (state) => {
      delete state.failures[`graphState:${graphPath}`];
    },
  );
  if (!isCurrent()) return;
  for (const { output, state, resultId } of projection?.outputs ?? []) {
    const key = graphOutputKey(output);
    if (state === "valid" && resultProjection.getState().pinResults[key]?.resultId !== resultId)
      void resultQueryCoordinator.loadPinResult({ graphPath, output: output.port });
  }
}

/** The publication owner calls this after installing an entire graph frame. */
export function reconcileGraphResultQueries(graphPath: string, previous?: GraphEditorState): void {
  const owner = captureProjectLifecycleState();
  const { sessions, resultStates } = useResourceStore.getState();
  const current = sessions[graphPath];
  if (!current) {
    resetGraphResultQueries(graphPath);
    return;
  }
  resultQueryCoordinator.resetGraphState(graphPath);
  pendingGraphRefreshes.delete(graphPath);
  const resultState = resultStates[graphPath];
  const executionSessionId = resultState?.executionSessionId ?? resultExecutionSessionId;
  const request = {
    graphPath,
    sessionId: current.sessionId,
    projectionGeneration: current.projectionGeneration,
    semanticInputHash: current.semanticInputHash,
  };
  const isCurrent = () =>
    isProjectLifecycleStateCurrent(owner) &&
    currentGraphStateRequest(request) &&
    useResourceStore.getState().resultStates[graphPath] === resultState &&
    resultExecutionSessionId === executionSessionId;
  if (resultState && !prepareResultExecutionSession(resultState.executionSessionId)) return;
  if (!isCurrent()) return;
  const changed = Boolean(
    previous &&
    (previous.semanticInputHash !== current.semanticInputHash ||
      previous.sessionId !== current.sessionId),
  );
  if (changed) useExecutionStore.getState().releaseGraphExecutionState(graphPath);
  if (!isCurrent()) return;
  reconcileCurrentResults(graphPath, resultState ?? null, isCurrent);
  if (!isCurrent()) return;
  if (
    Object.values(useExecutionStore.getState().graphs[graphPath]?.outputRuns ?? {}).some((run) =>
      isGraphRunResultPending(run.run, run.resultRevision),
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
    if (!isCurrentPinResult(request.graphPath, key, result)) return;
  }
  const releasePrevious =
    previous &&
    (!result || resultReferenceKey(previous) !== resultReferenceKey(result)) &&
    !resultQueryCoordinator.isPayloadRetained(previous);
  if (releasePrevious) resultQueryCoordinator.resetResult(previous);
  resultProjection.setState(
    produce((state: Draft<ResultProjectionState>) => {
      if (releasePrevious) removeResultProjection(state, resultReferenceKey(previous));
      if (result) {
        state.pinResults[key] = castDraft(result);
        state.descriptors[resultReferenceKey(result)] = castDraft(result);
      } else {
        delete state.pinResults[key];
      }
    }),
  );
}

function invalidateOutputs(
  requests: readonly ResultPinRequest[],
  update?: (state: Draft<ResultProjectionState>) => void,
): void {
  const admitted = captureProjectLifecycleState().projectInstanceId ? requests : [];
  const retired = new Set<string>();
  for (const request of admitted) {
    resultQueryCoordinator.resetPinResult(request);
    const previous = resultProjection.getState().pinResults[pinResultKey(request)];
    if (previous && !resultQueryCoordinator.isPayloadRetained(previous)) {
      resultQueryCoordinator.resetResult(previous);
      retired.add(resultReferenceKey(previous));
    }
  }
  resultProjection.setState(
    produce((state: Draft<ResultProjectionState>) => {
      for (const key of retired) removeResultProjection(state, key);
      for (const request of admitted) delete state.pinResults[pinResultKey(request)];
      update?.(state);
    }),
  );
}

export function resetGraphResultQueries(graphPath: string, isCurrent?: () => boolean): void {
  if (isCurrent && !isCurrent()) return;
  const owner = captureProjectLifecycleState();
  const session = useResourceStore.getState().sessions[graphPath];
  const executionSessionId = resultExecutionSessionId;
  const outputRuns = useExecutionStore.getState().graphs[graphPath]?.outputRuns;
  pendingGraphRefreshes.delete(graphPath);
  resultQueryCoordinator.resetGraphState(graphPath);
  const requests = new Map<string, ResultPinRequest>();
  for (const [key, pending] of Object.entries(
    useExecutionStore.getState().graphs[graphPath]?.outputRuns ?? {},
  ))
    requests.set(key, { graphPath, output: pending.output.port });
  for (const result of Object.values(resultProjection.getState().pinResults)) {
    const output = result?.provenance.output;
    if (output?.graphPath === graphPath) {
      const request = { graphPath, output: output.port };
      requests.set(pinResultKey(request), request);
    }
  }
  invalidateOutputs([...requests.values()], (state) => {
    for (const key of requests.keys()) delete state.pinResults[key];
  });
  if (
    !isProjectLifecycleStateCurrent(owner) ||
    (isCurrent && !isCurrent()) ||
    useResourceStore.getState().sessions[graphPath] !== session ||
    resultExecutionSessionId !== executionSessionId ||
    useExecutionStore.getState().graphs[graphPath]?.outputRuns !== outputRuns
  )
    return;
  useExecutionStore.getState().clearOutputRuns(graphPath);
}

export function observeResultRunEvent(event: RunEvent): void {
  if (!isCurrentGraphRun(event.run)) return;
  const outputs = useExecutionStore.getState().getRunEventOutputs(event);
  const requests = outputs.map((output) => ({ graphPath: output.graphPath, output: output.port }));
  if (event.kind.type === "runStarted") {
    if (requests.length === 0) return;
    const isCurrent = captureGraphRefreshEligibility(event.run.graphPath);
    invalidateOutputs(requests);
    if (!isCurrent()) return;
  } else if (
    event.kind.type === "runCompleted" &&
    !useResourceStore.getState().sessions[event.run.graphPath]
  ) {
    for (const request of requests) void resultQueryCoordinator.loadPinResult(request);
  }
  resultQueryCoordinator.resetGraphState(event.run.graphPath);
  scheduleGraphStateRefresh(event.run.graphPath);
}
