import { getGraphProjectionBasis } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import {
  assertCurrentProjectIdentity,
  captureProjectCommandIdentity,
  isCurrentProjectIdentity,
} from '@/services/project/projectIdentity';

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

export function captureProjectCommandContext(
  requestedOperationId: string = crypto.randomUUID(),
): ProjectCommandContext {
  const identity = captureProjectCommandIdentity();
  const operationId = requestedOperationId;
  const isCurrent = () => isCurrentProjectIdentity(identity);
  return {
    projectInstanceId: identity.projectInstanceId,
    projectEpoch: identity.epoch,
    publicationRevision: identity.publicationRevision,
    operationId,
    operationPendingKey: `${identity.projectInstanceId}:${operationId}`,
    isCurrent,
    assertCurrent: () => assertCurrentProjectIdentity(identity),
  };
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
