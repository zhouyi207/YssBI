import i18n from 'i18next';
import { DEFAULT_LANGUAGE } from '@/shared/types/settings';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';

import { markResourceStale, useResourceStore } from '@/features/core/resource';
import { GraphProjectionService } from '@/services/nodeSystem/graphProjectionService';
import { inferGraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import { formatErrorMessage } from '@/shared/utils/formatErrorMessage';
import { logger } from '@/utils/appLogger';
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

const latestGenerationByGraph = new Map<string, number>();
const lifecycleTokenByGraph = new Map<string, number>();

interface PendingInvalidation {
  inFlight: Promise<boolean>;
  trailingPromise?: Promise<boolean>;
  resolveTrailing?: (result: boolean) => void;
  rejectTrailing?: (error: unknown) => void;
}

const pendingInvalidationByGraph = new Map<string, PendingInvalidation>();
let nextLifecycleToken = Date.now() * 1_000;
let coordinatorEpoch = 0;

function nextRequestGeneration(graphPath: string): number {
  const bucketGeneration = useGraphDataStore.getState().graphEntities[graphPath]?.requestGeneration ?? 0;
  const generation = Math.max(latestGenerationByGraph.get(graphPath) ?? 0, bucketGeneration) + 1;
  latestGenerationByGraph.set(graphPath, generation);
  return generation;
}

function setGraphProjectionStale(graphPath: string, stale: boolean): void {
  const kind = inferGraphResourceKind(graphPath);
  if (kind) markResourceStale({ id: graphPath, kind }, stale);
}

function ownsLatestRequest(
  graphPath: string,
  requestGeneration: number,
  requestEpoch: number,
  lifecycleToken: number,
): boolean {
  return requestEpoch === coordinatorEpoch
    && lifecycleTokenByGraph.get(graphPath) === lifecycleToken
    && latestGenerationByGraph.get(graphPath) === requestGeneration;
}

function startGraphLifecycle(graphPath: string): number {
  const lifecycleToken = ++nextLifecycleToken;
  lifecycleTokenByGraph.set(graphPath, lifecycleToken);
  return lifecycleToken;
}

function currentOrStartGraphLifecycle(graphPath: string): number {
  return lifecycleTokenByGraph.get(graphPath) ?? startGraphLifecycle(graphPath);
}

function hasInstalledGenerationAtLeast(graphPath: string, requestGeneration: number): boolean {
  const bucket = useGraphDataStore.getState().graphEntities[graphPath];
  return bucket != null && bucket.requestGeneration >= requestGeneration;
}

type ProjectionRequestOperation = 'load' | 'hydrate';

async function requestGraphProjection(
  graphPath: string,
  operation: ProjectionRequestOperation,
  lifecycleToken: number,
  identity: ProjectIdentitySnapshot,
  request: (graphPath: string, locale: string, lifecycleToken: number) => Promise<EditorGraphProjectionDto>,
  locale = currentProjectionLocale(),
): Promise<boolean> {
  const requestGeneration = nextRequestGeneration(graphPath);
  const requestEpoch = coordinatorEpoch;
  setGraphProjectionStale(graphPath, true);

  let projection: EditorGraphProjectionDto;
  try {
    projection = await request(graphPath, locale, lifecycleToken);
    if (!isCurrentProjectIdentity(identity)) return false;
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return false;
    logger.graph.error(
      `Graph projection ${operation} IPC failed for '${graphPath}': ${formatErrorMessage(error, 'Unknown IPC error')}`,
      'GraphProjectionCoordinator',
    );
    if (ownsLatestRequest(graphPath, requestGeneration, requestEpoch, lifecycleToken)) {
      setGraphProjectionStale(graphPath, true);
    }
    return false;
  }

  if (!isCurrentProjectIdentity(identity)
    || !ownsLatestRequest(graphPath, requestGeneration, requestEpoch, lifecycleToken)) return false;
  const result = useGraphDataStore
    .getState()
    .replaceProjection(graphPath, projection, requestGeneration);
  if (result.applied) {
    const kind = inferGraphResourceKind(graphPath);
    if (kind) {
      useResourceStore.getState().patchResource(
        { id: graphPath, kind },
        { revision: projection.sourceRevision },
      );
    }
  }
  if (!result.applied && result.reason === 'invalid') {
    logger.graph.error(
      `Graph projection ${operation} contract invalid for '${graphPath}': ${formatErrorMessage(result.error, 'Unknown projection contract error')}`,
      'GraphProjectionCoordinator',
    );
  }
  const current = result.applied
    || (result.reason === 'stale-generation'
      && hasInstalledGenerationAtLeast(graphPath, requestGeneration));
  if (current) setGraphProjectionStale(graphPath, false);
  return current;
}

export function currentProjectionLocale(): string {
  return i18n.resolvedLanguage || i18n.language || DEFAULT_LANGUAGE;
}

export function beginGraphLoadLifecycle(graphPath: string): number {
  return startGraphLifecycle(graphPath);
}

export function invalidateGraphLifecycle(graphPath: string): number {
  const lifecycleToken = startGraphLifecycle(graphPath);
  nextRequestGeneration(graphPath);
  return lifecycleToken;
}

export function beginGraphUnloadLifecycle(graphPath: string): number {
  return invalidateGraphLifecycle(graphPath);
}

export function beginGraphRenameLifecycle(graphPath: string): number {
  return startGraphLifecycle(graphPath);
}

export function isGraphLifecycleCurrent(graphPath: string, lifecycleToken: number): boolean {
  return lifecycleTokenByGraph.get(graphPath) === lifecycleToken;
}

export async function prepareGraphProjectionForPublication(
  graphPath: string,
  projectInstanceId: string,
  publicationEpoch: number,
): Promise<EditorGraphProjectionDto | false> {
  const identity = { projectInstanceId, epoch: publicationEpoch };
  if (!isCurrentProjectIdentity(identity)) return false;
  const lifecycleToken = startGraphLifecycle(graphPath);
  const requestEpoch = coordinatorEpoch;
  try {
    const projection = await GraphProjectionService.loadGraph(
      graphPath,
      currentProjectionLocale(),
      lifecycleToken,
      projectInstanceId,
    );
    if (!isCurrentProjectIdentity(identity)
      || requestEpoch !== coordinatorEpoch
      || lifecycleTokenByGraph.get(graphPath) !== lifecycleToken) return false;
    return projection;
  } catch (error) {
    if (!isCurrentProjectIdentity(identity)) return false;
    logger.graph.error(
      `Graph projection publication prepare failed for '${graphPath}': ${formatErrorMessage(error, 'Unknown IPC error')}`,
      'GraphProjectionCoordinator',
    );
    return false;
  }
}

export function loadGraphProjection(
  graphPath: string,
  lifecycleToken = beginGraphLoadLifecycle(graphPath),
): Promise<boolean> {
  let identity: ProjectIdentitySnapshot;
  try {
    identity = captureProjectIdentity();
  } catch {
    return Promise.resolve(false);
  }
  return requestGraphProjection(
    graphPath,
    'load',
    lifecycleToken,
    identity,
    (path, locale, token) => GraphProjectionService.loadGraph(
      path,
      locale,
      token,
      identity.projectInstanceId,
    ),
  );
}

export function hydrateGraphProjection(graphPath: string, locale: string): Promise<boolean> {
  if (!useGraphDataStore.getState().hasGraph(graphPath)) {
    setGraphProjectionStale(graphPath, true);
    return Promise.resolve(false);
  }
  const identity = captureProjectIdentity();
  return requestGraphProjection(
    graphPath,
    'hydrate',
    currentOrStartGraphLifecycle(graphPath),
    identity,
    (path, requestLocale) => GraphProjectionService.hydrateGraph(
      identity.projectInstanceId,
      path,
      requestLocale,
    ),
    locale,
  );
}

function completeInvalidation(
  graphPath: string,
  pending: PendingInvalidation,
): void {
  if (pendingInvalidationByGraph.get(graphPath) !== pending) return;
  const resolveTrailing = pending.resolveTrailing;
  const rejectTrailing = pending.rejectTrailing;
  if (!resolveTrailing || !rejectTrailing) {
    pendingInvalidationByGraph.delete(graphPath);
    return;
  }

  pending.trailingPromise = undefined;
  pending.resolveTrailing = undefined;
  pending.rejectTrailing = undefined;
  const trailing = hydrateGraphProjection(graphPath, currentProjectionLocale());
  pending.inFlight = trailing;
  void trailing.then(resolveTrailing, rejectTrailing).finally(() => {
    completeInvalidation(graphPath, pending);
  });
}

function queueTrailingInvalidation(pending: PendingInvalidation): Promise<boolean> {
  if (!pending.trailingPromise) {
    pending.trailingPromise = new Promise<boolean>((resolve, reject) => {
      pending.resolveTrailing = resolve;
      pending.rejectTrailing = reject;
    });
  }
  return pending.trailingPromise;
}

export function invalidateGraphProjection(graphPath: string): Promise<boolean> {
  const pending = pendingInvalidationByGraph.get(graphPath);
  if (pending) return queueTrailingInvalidation(pending);

  const request = hydrateGraphProjection(graphPath, currentProjectionLocale());
  const next: PendingInvalidation = { inFlight: request };
  pendingInvalidationByGraph.set(graphPath, next);
  void request.finally(() => completeInvalidation(graphPath, next));
  return request;
}

export async function hydrateGraphProjections(
  graphPaths: Iterable<string>,
  locale: string,
): Promise<void> {
  await Promise.all(
    [...new Set(graphPaths)].map((graphPath) => hydrateGraphProjection(graphPath, locale)),
  );
}

export async function invalidateGraphProjections(graphPaths: Iterable<string>): Promise<void> {
  await hydrateGraphProjections(graphPaths, currentProjectionLocale());
}

export function invalidateGraphProjectionRequests(graphPath: string): void {
  nextRequestGeneration(graphPath);
}

export function resetGraphProjectionCoordinator(): void {
  coordinatorEpoch += 1;
  latestGenerationByGraph.clear();
  lifecycleTokenByGraph.clear();
  pendingInvalidationByGraph.clear();
}
