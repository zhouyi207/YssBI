import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { freezeProjectionSnapshot, type DeepReadonly } from "@/shared/types/deepReadonly";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import type { ResultDescriptor, ResultPage, ResultValue } from "./types";
import type {
  ResultAnalysis,
  ResultAnalysisRequest,
  ResultTablePart,
} from "@/shared/types/domain/resultReport";
import {
  isResultReference,
  resultReference,
  resultReferenceKey,
  type ResultReference,
} from "@/shared/types/domain/result";
import { portAddressKey } from "@/features/domain/editorProjection";

export type ResultIdentityRequest = ResultReference;

export interface ResultPageRequest extends ResultIdentityRequest {
  readonly offset: number;
  readonly limit: number;
  readonly part?: ResultTablePart;
}

export interface ResultAnalysisQuery {
  readonly reference: ResultReference;
  readonly analysis: ResultAnalysisRequest;
}

export function resultPageKey(request: ResultPageRequest): string {
  return `${resultReferenceKey(request)}:${request.part ?? "value"}`;
}

export function resultAnalysisKey(request: ResultAnalysisQuery): string {
  return `${resultReferenceKey(request.reference)}:${request.analysis.kind}`;
}

export function resultAnalysisParameters(request: ResultAnalysisQuery): string {
  const analysis = request.analysis;
  switch (analysis.kind) {
    case "residualPlot":
      return JSON.stringify([analysis.maxPoints, analysis.xRange ?? null]);
    case "acfPacf":
      return String(analysis.maxLag);
    case "serialTests":
      return JSON.stringify([analysis.lags, analysis.bgNomiss0]);
    case "hypothesis":
      return analysis.hypothesis;
  }
}

export interface ResultPinRequest {
  readonly graphPath: string;
  readonly output: PortAddressDto;
}

export type ResultQueryScope =
  | ({ readonly kind: "descriptor" | "value" } & ResultIdentityRequest)
  | ({ readonly kind: "page" } & ResultPageRequest)
  | ({ readonly kind: "analysis" } & ResultAnalysisQuery)
  | ({ readonly kind: "pinResult" } & ResultPinRequest);

export type ResultQueryOutcome =
  | { readonly status: "published" }
  | { readonly status: "stale" }
  | { readonly status: "notReady" }
  | { readonly status: "failed" };

export interface ResultQueryCoordinator {
  loadDescriptor(request: ResultIdentityRequest): Promise<ResultQueryOutcome>;
  loadValue(request: ResultIdentityRequest): Promise<ResultQueryOutcome>;
  loadPage(request: ResultPageRequest): Promise<ResultQueryOutcome>;
  loadAnalysis(request: ResultAnalysisQuery): Promise<ResultQueryOutcome>;
  loadPinResult(request: ResultPinRequest): Promise<ResultQueryOutcome>;
  retainPayload(reference: ResultReference): () => void;
  resetProject(): void;
  resetResult(reference: ResultReference): void;
  resetPinResult(request: ResultPinRequest): void;
  isPayloadRetained(reference: ResultReference): boolean;
}

export interface ResultQueryServicePort {
  readonly getDescriptor: (reference: ResultReference) => Promise<ResultDescriptor | null>;
  readonly getValue: (reference: ResultReference) => Promise<ResultValue | null>;
  readonly getPage: (
    reference: ResultReference,
    offset: number,
    limit: number,
    part?: ResultTablePart,
  ) => Promise<ResultPage | null>;
  readonly analyze: (
    reference: ResultReference,
    analysis: ResultAnalysisRequest,
  ) => Promise<ResultAnalysis>;
  readonly getPinResult: (
    graphPath: string,
    output: PortAddressDto,
  ) => Promise<ResultDescriptor | null>;
}

export interface ResultQueryPublication {
  readonly releasePayload: (reference: ResultReference) => void;
  readonly publishDescriptor: (
    projectInstanceId: string | null,
    reference: ResultReference,
    descriptor: DeepReadonly<ResultDescriptor | null>,
  ) => void;
  readonly publishValue: (
    projectInstanceId: string | null,
    reference: ResultReference,
    value: DeepReadonly<ResultValue | null>,
  ) => void;
  readonly publishPage: (
    projectInstanceId: string | null,
    request: ResultPageRequest,
    page: DeepReadonly<ResultPage | null>,
  ) => void;
  readonly publishAnalysis: (
    projectInstanceId: string | null,
    request: ResultAnalysisQuery,
    value: DeepReadonly<ResultAnalysis | null>,
  ) => void;
  readonly publishPinResult: (
    projectInstanceId: string | null,
    request: ResultPinRequest,
    result: DeepReadonly<ResultDescriptor | null>,
  ) => void;
  readonly publishFailure: (
    projectInstanceId: string | null,
    scope: ResultQueryScope,
    issue: ErrorReference,
  ) => void;
}

/** Read side of the Application-owned result projection used by staged hooks. */
export interface ResultQueryReadCapability {
  readonly subscribe: (listener: () => void) => () => void;
  readonly getDescriptor: (reference: ResultReference) => DeepReadonly<ResultDescriptor | null>;
  readonly getValue: (reference: ResultReference) => DeepReadonly<ResultValue | null>;
  readonly getPage: (request: ResultPageRequest) => DeepReadonly<ResultPage | null>;
  readonly getAnalysis: (request: ResultAnalysisQuery) => DeepReadonly<ResultAnalysis | null>;
  readonly getPinResult: (
    request: ResultPinRequest,
  ) => DeepReadonly<ResultDescriptor | null> | null;
  readonly getFailure: (scope: ResultQueryScope) => DeepReadonly<ErrorReference> | null;
}

export interface ResultQueryDependencies {
  readonly readCurrentProjectInstanceId: () => string | null;
  readonly service: ResultQueryServicePort;
  readonly publication: ResultQueryPublication;
  readonly toErrorReference?: (error: unknown, fallbackCode: string) => ErrorReference;
}

interface RequestOwner {
  readonly projectInstanceId: string | null;
  readonly projectEpoch: number;
  readonly queryKey: string;
  readonly scope: ResultQueryScope;
}

type ResultQueryValue = ResultDescriptor | ResultValue | ResultPage | ResultAnalysis;

function queryPart(value: string): string {
  return `${value.length}:${value}`;
}

function queryKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "descriptor":
    case "value":
      return `${scope.kind}:${queryPart(resultReferenceKey(scope))}`;
    case "page":
      return `${scope.kind}:${queryPart(resultPageKey(scope))}`;
    case "analysis":
      return `${scope.kind}:${queryPart(resultAnalysisKey(scope))}`;
    case "pinResult":
      return `${scope.kind}:${queryPart(scope.graphPath)}:${portAddressKey(scope.output)}`;
  }
}

function referenceFor(scope: ResultQueryScope): ResultReference | null {
  return scope.kind === "analysis" ? scope.reference : scope.kind === "pinResult" ? null : scope;
}

function validIdentity(value: string | null): value is string {
  return value !== null && value.length > 0;
}

function validIdentityRequest(request: ResultIdentityRequest): boolean {
  return isResultReference(request);
}

function validPageRequest(request: ResultPageRequest): boolean {
  return (
    validIdentityRequest(request) &&
    Number.isSafeInteger(request.offset) &&
    request.offset >= 0 &&
    Number.isSafeInteger(request.limit) &&
    request.limit > 0
  );
}

function validPinRequest(request: ResultPinRequest): boolean {
  return (
    request.graphPath.length > 0 && typeof request.output === "object" && request.output !== null
  );
}

function isErrorReference(value: unknown): value is ErrorReference {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { code?: unknown }).code === "string" &&
    ((value as { incidentId?: unknown }).incidentId === null ||
      typeof (value as { incidentId?: unknown }).incidentId === "string")
  );
}

function fallbackIssue(code: string): ErrorReference {
  return { code, incidentId: null };
}

export function createResultQueryCoordinator(
  dependencies: ResultQueryDependencies,
): ResultQueryCoordinator {
  let projectEpoch = 0;
  const requests = new Map<string, RequestOwner>();
  const payloadConsumers = new Map<string, Set<symbol>>();

  const captureProject = (): string | null => {
    try {
      const projectInstanceId = dependencies.readCurrentProjectInstanceId();
      return validIdentity(projectInstanceId) ? projectInstanceId : null;
    } catch {
      return null;
    }
  };

  const isCurrent = (owner: RequestOwner): boolean =>
    owner.projectEpoch === projectEpoch &&
    requests.get(owner.queryKey) === owner &&
    captureProject() === owner.projectInstanceId;

  const issueFor = (error: unknown, fallbackCode: string): ErrorReference => {
    try {
      const mapped = dependencies.toErrorReference
        ? dependencies.toErrorReference(error, fallbackCode)
        : toErrorReference(error, fallbackCode);
      if (isErrorReference(mapped)) return mapped;
    } catch {
      // A mapper is advisory; the closed fallback remains authoritative.
    }
    return fallbackIssue(fallbackCode);
  };

  const load = async <T extends ResultQueryValue>(
    scope: ResultQueryScope,
    read: () => Promise<T | null>,
    publish: (projectInstanceId: string | null, value: DeepReadonly<T | null>) => void,
    fallbackCode: string,
  ): Promise<ResultQueryOutcome> => {
    const projectInstanceId = captureProject();

    const key = queryKey(scope);
    const owner: RequestOwner = {
      projectInstanceId,
      projectEpoch,
      queryKey: key,
      scope,
    };
    requests.set(key, owner);

    try {
      const value = await read();
      if (!isCurrent(owner)) return { status: "stale" };

      const snapshot = freezeProjectionSnapshot(value);
      if (!isCurrent(owner)) return { status: "stale" };
      publish(owner.projectInstanceId, snapshot);
      return { status: value === null ? "notReady" : "published" };
    } catch (error) {
      if (!isCurrent(owner)) return { status: "stale" };
      try {
        dependencies.publication.publishFailure(
          owner.projectInstanceId,
          owner.scope,
          issueFor(error, fallbackCode),
        );
      } catch {
        // A failure publication cannot reopen the rejected query.
      }
      return { status: "failed" };
    } finally {
      if (requests.get(key) === owner) requests.delete(key);
    }
  };

  const loadDescriptor = (request: ResultIdentityRequest): Promise<ResultQueryOutcome> => {
    if (!validIdentityRequest(request)) return Promise.resolve({ status: "notReady" });
    const scope: ResultQueryScope = { kind: "descriptor", ...resultReference(request) };
    return load(
      scope,
      () => dependencies.service.getDescriptor(resultReference(request)),
      (projectInstanceId, value) =>
        dependencies.publication.publishDescriptor(projectInstanceId, request, value),
      "result_descriptor_read_failed",
    );
  };

  const loadValue = (request: ResultIdentityRequest): Promise<ResultQueryOutcome> => {
    if (!validIdentityRequest(request)) return Promise.resolve({ status: "notReady" });
    const scope: ResultQueryScope = { kind: "value", ...resultReference(request) };
    return load(
      scope,
      () => dependencies.service.getValue(resultReference(request)),
      (projectInstanceId, value) =>
        dependencies.publication.publishValue(projectInstanceId, request, value),
      "result_value_read_failed",
    );
  };

  const loadPage = (request: ResultPageRequest): Promise<ResultQueryOutcome> => {
    if (!validPageRequest(request)) return Promise.resolve({ status: "notReady" });
    const scope: ResultQueryScope = {
      kind: "page",
      ...resultReference(request),
      offset: request.offset,
      limit: request.limit,
      ...(request.part ? { part: request.part } : {}),
    };
    return load(
      scope,
      () =>
        request.part
          ? dependencies.service.getPage(
              resultReference(request),
              request.offset,
              request.limit,
              request.part,
            )
          : dependencies.service.getPage(resultReference(request), request.offset, request.limit),
      (projectInstanceId, value) =>
        dependencies.publication.publishPage(projectInstanceId, request, value),
      "result_page_read_failed",
    );
  };

  const loadAnalysis = (request: ResultAnalysisQuery): Promise<ResultQueryOutcome> => {
    const scope: ResultQueryScope = { kind: "analysis", ...request };
    return load(
      scope,
      () => dependencies.service.analyze(request.reference, request.analysis),
      (projectInstanceId, value) =>
        dependencies.publication.publishAnalysis(projectInstanceId, request, value),
      "result_analysis_failed",
    );
  };

  const loadPinResult = (request: ResultPinRequest): Promise<ResultQueryOutcome> => {
    if (!validPinRequest(request)) return Promise.resolve({ status: "notReady" });
    const scope: ResultQueryScope = {
      kind: "pinResult",
      graphPath: request.graphPath,
      output: request.output,
    };
    return load(
      scope,
      async () => dependencies.service.getPinResult(request.graphPath, request.output),
      (projectInstanceId, value) =>
        dependencies.publication.publishPinResult(projectInstanceId, request, value),
      "result_pin_read_failed",
    );
  };

  return {
    loadDescriptor,
    loadValue,
    loadPage,
    loadAnalysis,
    loadPinResult,
    isPayloadRetained: (reference) =>
      (payloadConsumers.get(resultReferenceKey(reference))?.size ?? 0) > 0,
    retainPayload: (reference) => {
      const key = resultReferenceKey(reference);
      const consumers = payloadConsumers.get(key) ?? new Set<symbol>();
      const consumer = Symbol();
      consumers.add(consumer);
      payloadConsumers.set(key, consumers);
      return () => {
        if (!consumers.delete(consumer) || payloadConsumers.get(key) !== consumers) return;
        if (consumers.size > 0) return;
        payloadConsumers.delete(key);
        for (const [query, owner] of requests) {
          const target = referenceFor(owner.scope);
          if (owner.scope.kind !== "descriptor" && target && resultReferenceKey(target) === key)
            requests.delete(query);
        }
        dependencies.publication.releasePayload(reference);
      };
    },
    resetProject: () => {
      projectEpoch += 1;
      requests.clear();
      payloadConsumers.clear();
    },
    resetResult: (reference) => {
      for (const [key, owner] of requests) {
        const target = referenceFor(owner.scope);
        if (target && resultReferenceKey(target) === resultReferenceKey(reference))
          requests.delete(key);
      }
    },
    resetPinResult: (request) => requests.delete(queryKey({ kind: "pinResult", ...request })),
  };
}
