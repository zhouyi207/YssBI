import { publishResultInvalidation } from "@/services/result/resultInvalidationChannel";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { pinPreviewCacheKey, useExecutionStore } from "@/features/core/execution";
import { graphOutputKey } from "@/features/domain/editorProjection";
import type { RunEvent } from "@/shared/types/domain/runEvent";
import { createBoundApplicationStore } from "@/features/core/state/applicationStore";

import { ResultService } from "@/services/result/resultService";
import { toErrorReference } from "@/features/application/errorReference";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import type { ErrorReference } from "@/features/application/errorReference";
import type { ResultDescriptor, ResultPage, ResultValue } from "./types";
import {
  createResultQueryCoordinator,
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
  readonly pinResults: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly pinStatuses: Record<string, "running" | "unavailable" | "failed" | "cancelled">;
  readonly failures: Record<string, DeepReadonly<ErrorReference>>;
}

const emptyState: ResultProjectionState = {
  projectInstanceId: null,
  descriptors: {},
  values: {},
  pages: {},
  pinResults: {},
  failures: {},
  pinStatuses: {},
};

const resultProjection = createBoundApplicationStore<ResultProjectionState>(() => emptyState);

function pageKey(request: ResultPageRequest): string {
  return `${request.resultId}:${request.offset}:${request.limit}`;
}

function pinResultKey(request: ResultPinRequest): string {
  return graphOutputKey({ graphPath: request.graphPath, port: request.output });
}

function scopeKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "descriptor":
    case "value":
      return `${scope.kind}:${scope.resultId}`;
    case "page":
      return `page:${pageKey(scope)}`;
    case "pinResult":
      return `pinResult:${pinResultKey(scope)}`;
  }
}

const resultQueryPublication = {
  publishDescriptor(
    projectInstanceId: string,
    resultId: string,
    descriptor: DeepReadonly<ResultDescriptor | null>,
  ) {
    if (!descriptor) {
      resetResultQuery(resultId);
      return;
    }
    const output = descriptor.provenance.output;
    if (output) {
      const request = { graphPath: output.graphPath, output: output.port };
      const pending = outputRuns.get(pinResultKey(request));
      if (pending && BigInt(pending.runId) > BigInt(descriptor.provenance.runId)) return;
      publishCurrentResult(projectInstanceId, request, descriptor);
    } else {
      resultProjection.setState((state) => ({
        ...state,
        projectInstanceId,
        descriptors: { ...state.descriptors, [resultId]: descriptor },
      }));
    }
  },
  publishValue(
    projectInstanceId: string,
    resultId: string,
    value: DeepReadonly<ResultValue | null>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      values: { ...state.values, [resultId]: value },
    }));
  },
  publishPage(
    projectInstanceId: string,
    request: ResultPageRequest,
    page: DeepReadonly<ResultPage | null>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      pages: { ...state.pages, [pageKey(request)]: page },
    }));
  },
  publishPinResult(
    projectInstanceId: string,
    request: ResultPinRequest,
    result: DeepReadonly<ResultDescriptor | null>,
  ) {
    publishCurrentResult(projectInstanceId, request, result);
  },
  publishFailure(projectInstanceId: string, scope: ResultQueryScope, issue: ErrorReference) {
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
  getDescriptor: (resultId) => resultProjection.getState().descriptors[resultId] ?? null,
  getValue: (resultId) => resultProjection.getState().values[resultId] ?? null,
  getPage: (request) => resultProjection.getState().pages[pageKey(request)] ?? null,
  getPinResult: (request) => resultProjection.getState().pinResults[pinResultKey(request)] ?? null,
  getFailure: (scope) => resultProjection.getState().failures[scopeKey(scope)] ?? null,
};

export const resultQueryCoordinator = createResultQueryCoordinator({
  readCurrentProjectInstanceId: () => useProjectIOStore.getState().projectInstanceId,
  service: {
    getDescriptor: (resultId) => ResultService.getDescriptor(resultId),
    getValue: (resultId) => ResultService.getValue(resultId),
    getPage: (resultId, offset, limit) => ResultService.getPage(resultId, offset, limit),
    getPinResult: (graphPath, output) => ResultService.getPinResult(graphPath, output),
  },
  publication: resultQueryPublication,
  toErrorReference,
});

export function resetResultQueryProject(): void {
  publishResultInvalidation(null);
  resultQueryCoordinator.resetProject();
  outputRuns.clear();
  resultProjectSessionId = null;
  for (const graph of Object.values(useExecutionStore.getState().graphs)) {
    for (const projection of graph.pinResults.values()) {
      useExecutionStore.getState().recordPinResult({ ...projection, result: null });
    }
  }
  resultProjection.setState(emptyState);
}

export function resetResultQuery(resultId: string): void {
  resultQueryCoordinator.resetResult(resultId);
  const output = resultProjection.getState().descriptors[resultId]?.provenance.output;
  if (output)
    useExecutionStore
      .getState()
      .recordPinResult({ graphPath: output.graphPath, output: output.port, result: null });
  resultProjection.setState((state) => {
    const descriptors = { ...state.descriptors };
    const values = { ...state.values };
    const pages = Object.fromEntries(
      Object.entries(state.pages).filter(([key]) => !key.startsWith(`${resultId}:`)),
    );
    const failures = Object.fromEntries(
      Object.entries(state.failures).filter(
        ([key]) =>
          key !== `descriptor:${resultId}` &&
          key !== `value:${resultId}` &&
          !key.startsWith(`page:${resultId}:`),
      ),
    );
    delete descriptors[resultId];
    delete values[resultId];
    const pinResults = Object.fromEntries(
      Object.entries(state.pinResults).map(([key, result]) => [
        key,
        result?.resultId === resultId ? null : result,
      ]),
    );
    return { ...state, descriptors, values, pages, failures, pinResults };
  });
}

let resultProjectSessionId: string | null = null;

// Each output retains one run owner so delayed run events cannot invalidate a newer value.
const outputRuns = new Map<string, { runId: string; request: ResultPinRequest; active: boolean }>();

function publishCurrentResult(
  projectInstanceId: string,
  request: ResultPinRequest,
  result: DeepReadonly<ResultDescriptor | null>,
): void {
  const key = pinResultKey(request);
  const previous = resultProjection.getState().pinResults[key];
  if (previous && previous.resultId !== result?.resultId) resetResultQuery(previous.resultId);
  resultProjection.setState((state) => ({
    ...state,
    projectInstanceId,
    pinResults: result
      ? { ...state.pinResults, [key]: result }
      : Object.fromEntries(
          Object.entries(state.pinResults).filter(([existing]) => existing !== key),
        ),
    descriptors: result ? { ...state.descriptors, [result.resultId]: result } : state.descriptors,
  }));
  useExecutionStore
    .getState()
    .recordPinResult({ ...request, result: structuredClone(result) as ResultDescriptor | null });
}

function invalidateOutputs(requests: readonly ResultPinRequest[]): void {
  resultQueryCoordinator.resetProject();
  const projectInstanceId = useProjectIOStore.getState().projectInstanceId;
  if (!projectInstanceId) return;
  const previousIds = requests.flatMap((request) => {
    const result = resultProjection.getState().pinResults[pinResultKey(request)];
    return result ? [result.resultId] : [];
  });
  if (previousIds.length > 0) publishResultInvalidation(previousIds);
  for (const request of requests) {
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
  for (const result of Object.values(resultProjection.getState().descriptors)) {
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
    if (resultProjectSessionId !== null && resultProjectSessionId !== event.run.projectSessionId)
      resetResultQueryProject();
    resultProjectSessionId = event.run.projectSessionId;
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
