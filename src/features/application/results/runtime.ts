import { publishResultSessionEnd } from "@/services/result/resultSessionChannel";
import { resultReferenceKey, type ResultReference } from "@/shared/types/domain/result";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { pinPreviewCacheKey, useExecutionStore } from "@/features/core/execution";
import { graphOutputKey } from "@/features/domain/editorProjection";
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
  resultAnalysisParameters,
  type ResultAnalysisQuery,
  type ResultPageRequest,
  type ResultPinRequest,
  type ResultQueryReadCapability,
  type ResultQueryScope,
} from "./resultQueryCoordinator";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

interface ResultProjectionState {
  readonly projectInstanceId: string | null;
  readonly descriptors: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly values: Record<string, DeepReadonly<ResultValue | null>>;
  readonly pages: Record<string, DeepReadonly<ResultPage | null>>;
  readonly analyses: Record<
    string,
    { readonly parameters: string; readonly value: DeepReadonly<ResultAnalysis | null> }
  >;
  readonly pinResults: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly pinStatuses: Record<string, "running" | "unavailable" | "failed" | "cancelled">;
  readonly failures: Record<string, DeepReadonly<ErrorReference>>;
}

const emptyState: ResultProjectionState = {
  projectInstanceId: null,
  descriptors: {},
  values: {},
  pages: {},
  analyses: {},
  pinResults: {},
  failures: {},
  pinStatuses: {},
};

const resultProjection = createBoundApplicationStore<ResultProjectionState>(() => emptyState);

export function useCurrentResultDescriptors() {
  return resultProjection((state) => state.pinResults);
}

function pinResultKey(request: ResultPinRequest): string {
  return graphOutputKey({ graphPath: request.graphPath, port: request.output });
}

function scopeKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
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
  publishDescriptor(
    projectInstanceId: string | null,
    reference: ResultReference,
    descriptor: DeepReadonly<ResultDescriptor | null>,
  ) {
    if (!descriptor) {
      resetResultQuery(reference);
      return;
    }
    // Reading a retained snapshot must never replace the pin's current result.
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      descriptors: { ...state.descriptors, [resultReferenceKey(reference)]: descriptor },
    }));
  },
  publishValue(
    projectInstanceId: string | null,
    reference: ResultReference,
    value: DeepReadonly<ResultValue | null>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      values: { ...state.values, [resultReferenceKey(reference)]: value },
      failures: Object.fromEntries(
        Object.entries(state.failures).filter(
          ([key]) => key !== `value:${resultReferenceKey(reference)}`,
        ),
      ),
    }));
  },
  publishPage(
    projectInstanceId: string | null,
    request: ResultPageRequest,
    page: DeepReadonly<ResultPage | null>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      pages: { ...state.pages, [resultPageKey(request)]: page },
      failures: Object.fromEntries(
        Object.entries(state.failures).filter(([key]) => key !== `page:${resultPageKey(request)}`),
      ),
    }));
  },
  publishAnalysis(
    projectInstanceId: string | null,
    request: ResultAnalysisQuery,
    value: DeepReadonly<ResultAnalysis | null>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      analyses: {
        ...state.analyses,
        [resultAnalysisKey(request)]: { parameters: resultAnalysisParameters(request), value },
      },
      failures: Object.fromEntries(
        Object.entries(state.failures).filter(
          ([key]) => key !== `analysis:${resultAnalysisKey(request)}`,
        ),
      ),
    }));
  },
  publishPinResult(
    projectInstanceId: string | null,
    request: ResultPinRequest,
    result: DeepReadonly<ResultDescriptor | null>,
  ) {
    publishCurrentResult(projectInstanceId, request, result);
  },
  publishFailure(projectInstanceId: string | null, scope: ResultQueryScope, issue: ErrorReference) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
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
  getAnalysis: (request) => {
    const entry = resultProjection.getState().analyses[resultAnalysisKey(request)];
    return entry?.parameters === resultAnalysisParameters(request) ? entry.value : null;
  },
  getPinResult: (request) => resultProjection.getState().pinResults[pinResultKey(request)] ?? null,
  getFailure: (scope) => resultProjection.getState().failures[scopeKey(scope)] ?? null,
};

export const resultQueryCoordinator = createResultQueryCoordinator({
  readCurrentProjectInstanceId: () => captureProjectLifecycleState().projectInstanceId,
  service: {
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
  outputRuns.clear();
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

// Each output retains one run owner so delayed run events cannot invalidate a newer value.
const outputRuns = new Map<string, { runId: string; request: ResultPinRequest; active: boolean }>();

function publishCurrentResult(
  projectInstanceId: string | null,
  request: ResultPinRequest,
  result: DeepReadonly<ResultDescriptor | null>,
): void {
  const key = pinResultKey(request);
  const previous = resultProjection.getState().pinResults[key];
  if (result) {
    const pending = outputRuns.get(key);
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
    projectInstanceId,
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
    publishCurrentResult(projectInstanceId, request, null);
    const execution = useExecutionStore.getState();
    const preview = execution.graphs[request.graphPath]?.pinPreviews.get(
      pinPreviewCacheKey(request.graphPath, request.output),
    );
    if (preview?.status === "ready")
      execution.removePinPreview(request.graphPath, request.output, preview.generation);
  }
}

export function invalidateGraphResults(graphPath: string): void {
  const requests = new Map<string, ResultPinRequest>();
  for (const [key, pending] of outputRuns) {
    if (pending.request.graphPath === graphPath) {
      requests.set(key, pending.request);
      outputRuns.delete(key);
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
    pinResults: Object.fromEntries(
      Object.entries(state.pinResults).filter(([key]) => !requests.has(key)),
    ),
    pinStatuses: Object.fromEntries(
      Object.entries(state.pinStatuses).filter(([key]) => !requests.has(key)),
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
        const owner = outputRuns.get(pinResultKey(request));
        return !owner || BigInt(owner.runId) < BigInt(event.run.runId);
      });
    if (requests.length === 0) return;
    for (const request of requests)
      outputRuns.set(pinResultKey(request), { runId: event.run.runId, request, active: true });
    invalidateOutputs(requests);
    resultProjection.setState((state) => ({
      ...state,
      pinStatuses: {
        ...state.pinStatuses,
        ...Object.fromEntries(
          requests.map((request) => [pinResultKey(request), "running" as const]),
        ),
      },
    }));
  } else if (["runCompleted", "runErrored", "runCancelled"].includes(event.kind.type)) {
    if (event.run.executionSessionId !== resultExecutionSessionId) return;
    for (const [key, pending] of outputRuns) {
      if (pending.runId !== event.run.runId || !pending.active) continue;
      pending.active = false;
      const status =
        event.kind.type === "runErrored"
          ? "failed"
          : event.kind.type === "runCancelled"
            ? "cancelled"
            : "unavailable";
      resultProjection.setState((state) => ({
        ...state,
        pinStatuses: { ...state.pinStatuses, [key]: status },
      }));
      if (event.kind.type === "runCompleted")
        void resultQueryCoordinator.loadPinResult(pending.request);
    }
  }
}

export function readPinResultStatus(
  request: ResultPinRequest,
): "running" | "unavailable" | "failed" | "cancelled" {
  return resultProjection.getState().pinStatuses[pinResultKey(request)] ?? "unavailable";
}

useGraphProjectionStore.subscribe((state, previous) => {
  for (const [graphPath, graph] of Object.entries(previous.graphEntities)) {
    const current = state.graphEntities[graphPath];
    if (!current || JSON.stringify(current.basis) !== JSON.stringify(graph.basis))
      invalidateGraphResults(graphPath);
  }
});
