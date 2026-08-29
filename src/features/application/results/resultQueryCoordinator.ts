import {
  toErrorReference,
  type ErrorReference,
} from '@/features/application/errorReference';
import {
  freezeProjectionSnapshot,
  type DeepReadonly,
} from '@/shared/types/deepReadonly';
import type { PortAddressDto } from '@/shared/types/domain/editorProjection';
import type {
  PinResultEntry,
  ResultDescriptor,
  ResultPage,
  ResultValue,
} from './types';
import { portAddressKey } from '@/features/domain/editorProjection';

export interface ResultIdentityRequest {
  readonly resultId: string;
}

export interface ResultPageRequest extends ResultIdentityRequest {
  readonly offset: number;
  readonly limit: number;
}

export interface ResultPinHistoryRequest {
  readonly graphPath: string;
  readonly output: PortAddressDto;
}

export type ResultQueryScope =
  | ({ readonly kind: 'descriptor' | 'value' } & ResultIdentityRequest)
  | ({ readonly kind: 'page' } & ResultPageRequest)
  | ({ readonly kind: 'pinHistory' } & ResultPinHistoryRequest);

export type ResultQueryOutcome =
  | { readonly status: 'published' }
  | { readonly status: 'stale' }
  | { readonly status: 'notReady' }
  | { readonly status: 'failed' };

export interface ResultQueryCoordinator {
  loadDescriptor(request: ResultIdentityRequest): Promise<ResultQueryOutcome>;
  loadValue(request: ResultIdentityRequest): Promise<ResultQueryOutcome>;
  loadPage(request: ResultPageRequest): Promise<ResultQueryOutcome>;
  loadPinHistory(request: ResultPinHistoryRequest): Promise<ResultQueryOutcome>;
  resetProject(): void;
  resetResult(resultId: string): void;
}

export interface ResultQueryServicePort {
  readonly getDescriptor: (resultId: string) => Promise<ResultDescriptor | null>;
  readonly getValue: (resultId: string) => Promise<ResultValue | null>;
  readonly getPage: (
    resultId: string,
    offset: number,
    limit: number,
  ) => Promise<ResultPage | null>;
  readonly getPinHistory: (
    graphPath: string,
    output: PortAddressDto,
  ) => Promise<readonly PinResultEntry[]>;
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
  readonly publishPinHistory: (
    projectInstanceId: string,
    request: ResultPinHistoryRequest,
    entries: DeepReadonly<readonly PinResultEntry[]>,
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
  readonly getDescriptor: (
    resultId: string,
  ) => DeepReadonly<ResultDescriptor | null>;
  readonly getValue: (resultId: string) => DeepReadonly<ResultValue | null>;
  readonly getPage: (
    request: ResultPageRequest,
  ) => DeepReadonly<ResultPage | null>;
  readonly getPinHistory: (
    request: ResultPinHistoryRequest,
  ) => DeepReadonly<readonly PinResultEntry[]> | null;
  readonly getFailure: (
    scope: ResultQueryScope,
  ) => DeepReadonly<ErrorReference> | null;
}

export interface ResultQueryDependencies {
  readonly readCurrentProjectInstanceId: () => string | null;
  readonly service: ResultQueryServicePort;
  readonly publication: ResultQueryPublication;
  readonly toErrorReference?: (
    error: unknown,
    fallbackCode: string,
  ) => ErrorReference;
}

interface RequestOwner {
  readonly projectInstanceId: string;
  readonly projectEpoch: number;
  readonly resultEpoch: number | null;
  readonly queryKey: string;
  readonly queryGeneration: number;
  readonly scope: ResultQueryScope;
}

type ResultQueryValue = ResultDescriptor | ResultValue | ResultPage | readonly PinResultEntry[];

function queryPart(value: string): string {
  return `${value.length}:${value}`;
}

function queryKey(scope: ResultQueryScope): string {
  switch (scope.kind) {
    case 'descriptor':
    case 'value':
      return `${scope.kind}:${queryPart(scope.resultId)}`;
    case 'page':
      return `${scope.kind}:${queryPart(scope.resultId)}:${scope.offset}:${scope.limit}`;
    case 'pinHistory':
      return `${scope.kind}:${queryPart(scope.graphPath)}:${portAddressKey(scope.output)}`;
  }
}

function resultIdFor(scope: ResultQueryScope): string | null {
  return scope.kind === 'pinHistory' ? null : scope.resultId;
}

function validIdentity(value: string | null): value is string {
  return value !== null && value.length > 0;
}

function validIdentityRequest(request: ResultIdentityRequest): boolean {
  return validIdentity(request.resultId);
}

function validPageRequest(request: ResultPageRequest): boolean {
  return validIdentityRequest(request)
    && Number.isSafeInteger(request.offset)
    && request.offset >= 0
    && Number.isSafeInteger(request.limit)
    && request.limit > 0;
}

function validPinHistoryRequest(request: ResultPinHistoryRequest): boolean {
  return request.graphPath.length > 0 && typeof request.output === 'object'
    && request.output !== null;
}

function isErrorReference(value: unknown): value is ErrorReference {
  return typeof value === 'object'
    && value !== null
    && typeof (value as { code?: unknown }).code === 'string'
    && ((value as { incidentId?: unknown }).incidentId === null
      || typeof (value as { incidentId?: unknown }).incidentId === 'string');
}

function fallbackIssue(code: string): ErrorReference {
  return { code, incidentId: null };
}

export function createResultQueryCoordinator(
  dependencies: ResultQueryDependencies,
): ResultQueryCoordinator {
  let projectEpoch = 0;
  const resultEpochs = new Map<string, number>();
  const queryGenerations = new Map<string, number>();

  const captureProject = (): string | null => {
    try {
      const projectInstanceId = dependencies.readCurrentProjectInstanceId();
      return validIdentity(projectInstanceId) ? projectInstanceId : null;
    } catch {
      return null;
    }
  };

  const nextResultEpoch = (resultId: string): number => {
    const next = (resultEpochs.get(resultId) ?? 0) + 1;
    resultEpochs.set(resultId, next);
    return next;
  };

  const nextQueryGeneration = (key: string): number => {
    const next = (queryGenerations.get(key) ?? 0) + 1;
    queryGenerations.set(key, next);
    return next;
  };

  const isCurrent = (owner: RequestOwner): boolean => {
    if (owner.projectEpoch !== projectEpoch
      || queryGenerations.get(owner.queryKey) !== owner.queryGeneration) {
      return false;
    }
    const currentProject = captureProject();
    if (currentProject !== owner.projectInstanceId) return false;
    const resultId = resultIdFor(owner.scope);
    return resultId === null
      || (resultEpochs.get(resultId) ?? 0) === owner.resultEpoch;
  };

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
    publish: (projectInstanceId: string, value: DeepReadonly<T>) => void,
    fallbackCode: string,
  ): Promise<ResultQueryOutcome> => {
    const projectInstanceId = captureProject();
    if (!projectInstanceId) return { status: 'notReady' };

    const key = queryKey(scope);
    const resultId = resultIdFor(scope);
    const owner: RequestOwner = {
      projectInstanceId,
      projectEpoch,
      resultEpoch: resultId === null ? null : resultEpochs.get(resultId) ?? 0,
      queryKey: key,
      queryGeneration: nextQueryGeneration(key),
      scope,
    };

    try {
      const value = await read();
      if (!isCurrent(owner)) return { status: 'stale' };
      if (value === null) return { status: 'notReady' };

      const snapshot = freezeProjectionSnapshot(value);
      if (!isCurrent(owner)) return { status: 'stale' };
      publish(owner.projectInstanceId, snapshot);
      return { status: 'published' };
    } catch (error) {
      if (!isCurrent(owner)) return { status: 'stale' };
      try {
        dependencies.publication.publishFailure(
          owner.projectInstanceId,
          owner.scope,
          issueFor(error, fallbackCode),
        );
      } catch {
        // A failure publication cannot reopen the rejected query.
      }
      return { status: 'failed' };
    }
  };

  const loadDescriptor = (request: ResultIdentityRequest): Promise<ResultQueryOutcome> => {
    if (!validIdentityRequest(request)) return Promise.resolve({ status: 'notReady' });
    const scope: ResultQueryScope = { kind: 'descriptor', resultId: request.resultId };
    return load(
      scope,
      () => dependencies.service.getDescriptor(request.resultId),
      (projectInstanceId, value) => dependencies.publication.publishDescriptor(
        projectInstanceId,
        request.resultId,
        value,
      ),
      'result_descriptor_read_failed',
    );
  };

  const loadValue = (request: ResultIdentityRequest): Promise<ResultQueryOutcome> => {
    if (!validIdentityRequest(request)) return Promise.resolve({ status: 'notReady' });
    const scope: ResultQueryScope = { kind: 'value', resultId: request.resultId };
    return load(
      scope,
      () => dependencies.service.getValue(request.resultId),
      (projectInstanceId, value) => dependencies.publication.publishValue(
        projectInstanceId,
        request.resultId,
        value,
      ),
      'result_value_read_failed',
    );
  };

  const loadPage = (request: ResultPageRequest): Promise<ResultQueryOutcome> => {
    if (!validPageRequest(request)) return Promise.resolve({ status: 'notReady' });
    const scope: ResultQueryScope = {
      kind: 'page',
      resultId: request.resultId,
      offset: request.offset,
      limit: request.limit,
    };
    return load(
      scope,
      () => dependencies.service.getPage(request.resultId, request.offset, request.limit),
      (projectInstanceId, value) => dependencies.publication.publishPage(
        projectInstanceId,
        request,
        value,
      ),
      'result_page_read_failed',
    );
  };

  const loadPinHistory = (
    request: ResultPinHistoryRequest,
  ): Promise<ResultQueryOutcome> => {
    if (!validPinHistoryRequest(request)) return Promise.resolve({ status: 'notReady' });
    const scope: ResultQueryScope = {
      kind: 'pinHistory',
      graphPath: request.graphPath,
      output: request.output,
    };
    return load(
      scope,
      async () => dependencies.service.getPinHistory(request.graphPath, request.output),
      (projectInstanceId, value) => dependencies.publication.publishPinHistory(
        projectInstanceId,
        request,
        value,
      ),
      'result_pin_history_read_failed',
    );
  };

  return {
    loadDescriptor,
    loadValue,
    loadPage,
    loadPinHistory,
    resetProject: () => {
      projectEpoch += 1;
      resultEpochs.clear();
      queryGenerations.clear();
    },
    resetResult: (resultId) => {
      if (validIdentity(resultId)) nextResultEpoch(resultId);
    },
  };
}
