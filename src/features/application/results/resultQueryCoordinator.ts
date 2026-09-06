import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { freezeProjectionSnapshot, type DeepReadonly } from "@/shared/types/deepReadonly";
import type { PortAddressDto } from "@/shared/types/domain/editorProjection";
import type { ResultDescriptor, ResultPage, ResultValue } from "./types";
import { portAddressKey } from "@/features/domain/editorProjection";

export interface ResultIdentityRequest {
  readonly resultId: string;
}

export interface ResultPageRequest extends ResultIdentityRequest {
  readonly offset: number;
  readonly limit: number;
}

export interface ResultPinRequest {
  readonly graphPath: string;
  readonly output: PortAddressDto;
}

export type ResultQueryScope =
  | ({ readonly kind: "descriptor" | "value" } & ResultIdentityRequest)
  | ({ readonly kind: "page" } & ResultPageRequest)
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
  loadPinResult(request: ResultPinRequest): Promise<ResultQueryOutcome>;
  resetProject(): void;
  resetResult(resultId: string): void;
}

export interface ResultQueryServicePort {
  readonly getDescriptor: (resultId: string) => Promise<ResultDescriptor | null>;
  readonly getValue: (resultId: string) => Promise<ResultValue | null>;
  readonly getPage: (resultId: string, offset: number, limit: number) => Promise<ResultPage | null>;
  readonly getPinResult: (
    graphPath: string,
    output: PortAddressDto,
  ) => Promise<ResultDescriptor | null>;
}

export interface ResultQueryPublication {
  readonly publishDescriptor: (
    projectInstanceId: string,
    resultId: string,
    descriptor: DeepReadonly<ResultDescriptor | null>,
  ) => void;
  readonly publishValue: (
    projectInstanceId: string,
    resultId: string,
    value: DeepReadonly<ResultValue | null>,
  ) => void;
  readonly publishPage: (
    projectInstanceId: string,
    request: ResultPageRequest,
    page: DeepReadonly<ResultPage | null>,
  ) => void;
  readonly publishPinResult: (
    projectInstanceId: string,
    request: ResultPinRequest,
    result: DeepReadonly<ResultDescriptor | null>,
  ) => void;
  readonly publishFailure: (
    projectInstanceId: string,
    scope: ResultQueryScope,
    issue: ErrorReference,
  ) => void;
}

/** Read side of the Application-owned result projection used by staged hooks. */
export interface ResultQueryReadCapability {
  readonly subscribe: (listener: () => void) => () => void;
  readonly getDescriptor: (resultId: string) => DeepReadonly<ResultDescriptor | null>;
  readonly getValue: (resultId: string) => DeepReadonly<ResultValue | null>;
  readonly getPage: (request: ResultPageRequest) => DeepReadonly<ResultPage | null>;
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
  readonly projectInstanceId: string;
  readonly projectEpoch: number;
  readonly queryKey: string;
  readonly scope: ResultQueryScope;
}

type ResultQueryValue = ResultDescriptor | ResultValue | ResultPage;

function queryPart(value: string): string {
  return `${value.length}:${value}`;
}

function queryKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case "descriptor":
    case "value":
      return `${scope.kind}:${queryPart(scope.resultId)}`;
    case "page":
      return `${scope.kind}:${queryPart(scope.resultId)}:${scope.offset}:${scope.limit}`;
    case "pinResult":
      return `${scope.kind}:${queryPart(scope.graphPath)}:${portAddressKey(scope.output)}`;
  }
}

function resultIdFor(scope: ResultQueryScope): string | null {
  return scope.kind === "pinResult" ? null : scope.resultId;
}

function validIdentity(value: string | null): value is string {
  return value !== null && value.length > 0;
}

function validIdentityRequest(request: ResultIdentityRequest): boolean {
  return validIdentity(request.resultId);
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
    publish: (projectInstanceId: string, value: DeepReadonly<T | null>) => void,
    fallbackCode: string,
  ): Promise<ResultQueryOutcome> => {
    const projectInstanceId = captureProject();
    if (!projectInstanceId) return { status: "notReady" };

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
    const scope: ResultQueryScope = { kind: "descriptor", resultId: request.resultId };
    return load(
      scope,
      () => dependencies.service.getDescriptor(request.resultId),
      (projectInstanceId, value) =>
        dependencies.publication.publishDescriptor(projectInstanceId, request.resultId, value),
      "result_descriptor_read_failed",
    );
  };

  const loadValue = (request: ResultIdentityRequest): Promise<ResultQueryOutcome> => {
    if (!validIdentityRequest(request)) return Promise.resolve({ status: "notReady" });
    const scope: ResultQueryScope = { kind: "value", resultId: request.resultId };
    return load(
      scope,
      () => dependencies.service.getValue(request.resultId),
      (projectInstanceId, value) =>
        dependencies.publication.publishValue(projectInstanceId, request.resultId, value),
      "result_value_read_failed",
    );
  };

  const loadPage = (request: ResultPageRequest): Promise<ResultQueryOutcome> => {
    if (!validPageRequest(request)) return Promise.resolve({ status: "notReady" });
    const scope: ResultQueryScope = {
      kind: "page",
      resultId: request.resultId,
      offset: request.offset,
      limit: request.limit,
    };
    return load(
      scope,
      () => dependencies.service.getPage(request.resultId, request.offset, request.limit),
      (projectInstanceId, value) =>
        dependencies.publication.publishPage(projectInstanceId, request, value),
      "result_page_read_failed",
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
    loadPinResult,
    resetProject: () => {
      projectEpoch += 1;
      requests.clear();
    },
    resetResult: (resultId) => {
      for (const [key, owner] of requests) {
        if (resultIdFor(owner.scope) === resultId) requests.delete(key);
      }
    },
  };
}
