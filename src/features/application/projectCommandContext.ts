import { getGraphProjectionBasis } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

export interface ProjectCommandContext {
  projectInstanceId: string;
  projectEpoch: number;
  publicationRevision: number;
  operationId: string;
  operationPendingKey: string;
  isCurrent(): boolean;
  assertCurrent(): void;
}

export interface GraphSaveCommandContext extends ProjectCommandContext {
  expectedRevision: number;
}

export interface RevisionedProjectCommandSnapshot<T> {
  readonly context: ProjectCommandContext;
  readonly authority: T;
}

export function captureProjectCommandContext(
  requestedOperationId: string = crypto.randomUUID(),
): ProjectCommandContext {
  const identity = captureProjectIdentity();
  const publicationRevision = projectPublicationCoordinator.capturePublicationRevision();
  assertCurrentProjectIdentity(identity);
  const operationId = requestedOperationId;
  const isCurrent = () => isCurrentProjectIdentity(identity);
  return {
    projectInstanceId: identity.projectInstanceId,
    projectEpoch: identity.epoch,
    publicationRevision,
    operationId,
    operationPendingKey: `${identity.projectInstanceId}:${operationId}`,
    isCurrent,
    assertCurrent: () => assertCurrentProjectIdentity(identity),
  };
}

export function captureRevisionedProjectCommandSnapshot<T>(
  readAuthority: () => T,
): RevisionedProjectCommandSnapshot<T> {
  const context = captureProjectCommandContext();
  const authority = readAuthority();
  context.assertCurrent();
  return Object.freeze({ context, authority });
}

export function isGraphSaveCommandRevisionCurrent(
  context: GraphSaveCommandContext,
  graphPath: string,
): boolean {
  if (!context.isCurrent()) return false;
  const basis = getGraphProjectionBasis(useGraphDataStore.getState(), graphPath);
  return basis?.graphRevision === context.expectedRevision;
}

export function captureGraphSaveCommandContext(graphPath: string): GraphSaveCommandContext {
  const context = captureProjectCommandContext();
  const basis = getGraphProjectionBasis(useGraphDataStore.getState(), graphPath);
  if (!basis) {
    throw new Error(`Graph projection '${graphPath}' is not loaded`);
  }
  return { ...context, expectedRevision: basis.graphRevision };
}
