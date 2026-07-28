
import { currentProjectionLocale, hydrateGraphProjection } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { getGraphSourceRevision } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { EMPTY_HISTORY_STATE, useHistoryStore } from '@/features/core/history/historyStore';
import { HistoryService } from '@/services/nodeSystem/historyService';

import type {
  HistoryMutationDto,
  HistoryStatusDto,
  MutationRequestDto,

  ResourceKeyDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import {
  ProjectPublicationError,
  projectPublicationCoordinator,
} from './projectPublicationCoordinator';
import {
  completePendingMutation,
  registerPendingMutation,
  type PendingMutationRecord,
} from './pendingMutationRegistry';

export type HistoryDirection = 'undo' | 'redo';

export interface ExecuteHistoryMutationInput {
  direction: HistoryDirection;
  graphPath: string;
  locale: string;
}

export interface HistoryCoordinatorDependencies {
  createOperationId(): string;
  undo(
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto>;
  redo(
    locale: string,
    request: MutationRequestDto<HistoryMutationDto>,
  ): Promise<ResourceMutationResultDto>;
  hydrateGraph(graphPath: string, locale: string): Promise<unknown>;
  getStatus(): Promise<HistoryStatusDto>;
}

export type ExecuteHistoryMutationOutcome =
  | { status: 'applied'; result: ResourceMutationResultDto }
  | { status: 'conflict' }
  | { status: 'stale' };

let coordinatorEpoch = 0;
let historyStatusKnown = false;
let statusRequest: Promise<void> | null = null;
const pendingHistoryOperations = new Set<string>();

const defaultDependencies: HistoryCoordinatorDependencies = {
  createOperationId: () => crypto.randomUUID(),
  undo: (locale, request) => HistoryService.undo(locale, request),
  redo: (locale, request) => HistoryService.redo(locale, request),
  hydrateGraph: hydrateGraphProjection,
  getStatus: () => HistoryService.getStatus(),
};

function invalidHistoryResult(message: string): never {
  throw new Error(`invalid history result: ${message}`);
}

function isHistoryConflict(error: unknown): boolean {
  return typeof error === 'object'
    && error !== null
    && 'code' in error
    && (error as { code?: unknown }).code === 'history_revision_conflict';
}

function anchorResource(graphPath: string): ResourceKeyDto {
  return { kind: 'graph', key: graphPath };
}

function validateHistoryResult(
  pending: PendingMutationRecord,
  resource: ResourceKeyDto,
  result: ResourceMutationResultDto,
): string | undefined {
  if (result.deltas.some((delta) => delta.causedBy !== pending.operationId)) {
    return 'operation correlation does not match the pending request';
  }
  const anchorDelta = result.deltas.find((delta) =>
    delta.resource.kind === resource.kind && delta.resource.key === resource.key,
  );
  if (anchorDelta && anchorDelta.fromRevision !== pending.baseRevision) {
    return 'anchor revision does not match the request';
  }
  return undefined;
}

async function hydrateAffectedGraphs(
  graphPaths: Iterable<string>,
  locale: string,
  dependencies: HistoryCoordinatorDependencies,
): Promise<void> {
  await Promise.all(
    [...new Set(graphPaths)].map((graphPath) => dependencies.hydrateGraph(graphPath, locale)),
  );
}

export function setHistoryStatus(status: HistoryStatusDto): void {
  historyStatusKnown = true;
  useHistoryStore.setState({ canUndo: status.canUndo, canRedo: status.canRedo });
}

export async function refreshHistoryStatus(
  overrides: Partial<HistoryCoordinatorDependencies> = {},
): Promise<void> {
  const dependencies = { ...defaultDependencies, ...overrides };
  const epoch = coordinatorEpoch;
  useHistoryStore.setState({ pending: true });
  try {
    const status = await dependencies.getStatus();
    if (epoch === coordinatorEpoch) setHistoryStatus(status);
  } finally {
    if (epoch === coordinatorEpoch) useHistoryStore.setState({ pending: false });
  }
}

export function ensureHistoryStatus(): Promise<void> {
  if (historyStatusKnown) return Promise.resolve();
  if (statusRequest) return statusRequest;
  const request = refreshHistoryStatus();
  statusRequest = request;
  void request.then(
    () => {
      if (statusRequest === request) statusRequest = null;
    },
    () => {
      if (statusRequest === request) statusRequest = null;
    },
  );
  return request;
}

export async function executeHistoryMutation(
  input: ExecuteHistoryMutationInput,
  overrides: Partial<HistoryCoordinatorDependencies> = {},
): Promise<ExecuteHistoryMutationOutcome> {
  if (useHistoryStore.getState().pending) {
    throw new Error('a history request is already pending');
  }
  const dependencies = { ...defaultDependencies, ...overrides };
  const sourceRevision = getGraphSourceRevision(useGraphDataStore.getState(), input.graphPath);
  if (sourceRevision == null) {
    throw new Error(`history anchor '${input.graphPath}' is not loaded`);
  }

  const operationId = dependencies.createOperationId();
  const resource = anchorResource(input.graphPath);
  const pending: PendingMutationRecord = {
    operationId,
    graphPath: input.graphPath,
    baseRevision: sourceRevision,
  };
  const request: MutationRequestDto<HistoryMutationDto> = {
    resource,
    baseRevision: sourceRevision,
    operationId,
    payload: {},
  };
  const epoch = coordinatorEpoch;
  registerPendingMutation(pending);
  pendingHistoryOperations.add(operationId);
  useHistoryStore.setState({ pending: true });

  try {
    let result: ResourceMutationResultDto;
    try {
      result = await dependencies[input.direction](input.locale, request);
    } catch (error) {
      if (!isHistoryConflict(error)) throw error;
      if (epoch !== coordinatorEpoch) return { status: 'stale' };
      await hydrateAffectedGraphs([input.graphPath], input.locale, dependencies);
      if (epoch !== coordinatorEpoch) return { status: 'stale' };
      return { status: 'conflict' };
    }

    if (epoch !== coordinatorEpoch) return { status: 'stale' };
    try {
      await projectPublicationCoordinator.submit({
        result,
        fallbackPaths: [input.graphPath],
        validate: (candidate) => validateHistoryResult(pending, resource, candidate),
      });
    } catch (error) {
      if (error instanceof ProjectPublicationError && error.code === 'stale_project_lifecycle') {
        return { status: 'stale' };
      }
      invalidHistoryResult(error instanceof Error ? error.message : String(error));
    }
    if (epoch !== coordinatorEpoch) return { status: 'stale' };
    return { status: 'applied', result };
  } finally {
    completePendingMutation(operationId);
    pendingHistoryOperations.delete(operationId);
    if (epoch === coordinatorEpoch) useHistoryStore.setState({ pending: false });
  }
}

export function undoEditorHistory(graphPath: string): Promise<ExecuteHistoryMutationOutcome> {
  return executeHistoryMutation({
    direction: 'undo',
    graphPath,
    locale: currentProjectionLocale(),
  });
}

export function redoEditorHistory(graphPath: string): Promise<ExecuteHistoryMutationOutcome> {
  return executeHistoryMutation({
    direction: 'redo',
    graphPath,
    locale: currentProjectionLocale(),
  });
}

export function resetHistoryCoordinator(): void {
  coordinatorEpoch += 1;
  historyStatusKnown = false;
  statusRequest = null;
  for (const operationId of pendingHistoryOperations) completePendingMutation(operationId);
  pendingHistoryOperations.clear();
  useHistoryStore.setState(EMPTY_HISTORY_STATE, true);
}
