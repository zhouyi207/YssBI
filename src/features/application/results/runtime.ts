import { createBoundApplicationStore } from "@/features/core/state/applicationStore";

import { ResultService } from "@/services/result/resultService";
import { toErrorReference } from "@/features/application/errorReference";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import type { ErrorReference } from "@/features/application/errorReference";
import type { PinResultEntry, ResultDescriptor, ResultPage, ResultValue } from "./types";
import {
  createResultQueryCoordinator,
  type ResultPageRequest,
  type ResultPinHistoryRequest,
  type ResultQueryReadCapability,
  type ResultQueryScope,
} from "./resultQueryCoordinator";
import type { DeepReadonly } from "@/shared/types/deepReadonly";

interface ResultProjectionState {
  readonly projectInstanceId: string | null;
  readonly descriptors: Record<string, DeepReadonly<ResultDescriptor | null>>;
  readonly values: Record<string, DeepReadonly<ResultValue | null>>;
  readonly pages: Record<string, DeepReadonly<ResultPage | null>>;
  readonly pinHistories: Record<string, DeepReadonly<readonly PinResultEntry[]>>;
  readonly failures: Record<string, DeepReadonly<ErrorReference>>;
}

const emptyState: ResultProjectionState = {
  projectInstanceId: null,
  descriptors: {},
  values: {},
  pages: {},
  pinHistories: {},
  failures: {},
};

const resultProjection = createBoundApplicationStore<ResultProjectionState>(() => emptyState);

function pageKey(request: ResultPageRequest): string {
  return `${request.resultId}:${request.offset}:${request.limit}`;
}

function pinHistoryKey(request: ResultPinHistoryRequest): string {
  return `${request.graphPath}:${JSON.stringify(request.output)}`;
}

function scopeKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "descriptor":
    case "value":
      return `${scope.kind}:${scope.resultId}`;
    case "page":
      return `page:${pageKey(scope)}`;
    case "pinHistory":
      return `pinHistory:${pinHistoryKey(scope)}`;
  }
}

const resultQueryPublication = {
  publishDescriptor(
    projectInstanceId: string,
    resultId: string,
    descriptor: DeepReadonly<ResultDescriptor | null>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      descriptors: { ...state.descriptors, [resultId]: descriptor },
    }));
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
  publishPinHistory(
    projectInstanceId: string,
    request: ResultPinHistoryRequest,
    entries: DeepReadonly<readonly PinResultEntry[]>,
  ) {
    resultProjection.setState((state) => ({
      ...state,
      projectInstanceId,
      pinHistories: { ...state.pinHistories, [pinHistoryKey(request)]: entries },
    }));
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
  getPinHistory: (request) =>
    resultProjection.getState().pinHistories[pinHistoryKey(request)] ?? null,
  getFailure: (scope) => resultProjection.getState().failures[scopeKey(scope)] ?? null,
};

export const resultQueryCoordinator = createResultQueryCoordinator({
  readCurrentProjectInstanceId: () => useProjectIOStore.getState().projectInstanceId,
  service: {
    getDescriptor: (resultId) => ResultService.getDescriptor(resultId),
    getValue: (resultId) => ResultService.getValue(resultId),
    getPage: (resultId, offset, limit) => ResultService.getPage(resultId, offset, limit),
    getPinHistory: (graphPath, output) => ResultService.getPinHistory(graphPath, output),
  },
  publication: resultQueryPublication,
  toErrorReference,
});

export function resetResultQueryProject(): void {
  resultQueryCoordinator.resetProject();
  resultProjection.setState(emptyState);
}

export function resetResultQuery(resultId: string): void {
  resultQueryCoordinator.resetResult(resultId);
  resultProjection.setState((state) => {
    const descriptors = { ...state.descriptors };
    const values = { ...state.values };
    const failures = { ...state.failures };
    delete descriptors[resultId];
    delete values[resultId];
    delete failures[`descriptor:${resultId}`];
    delete failures[`value:${resultId}`];
    return { ...state, descriptors, values, failures };
  });
}
