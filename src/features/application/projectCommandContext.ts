import { getGraphProjectionBasis } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';

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

export function captureProjectCommandContext(): ProjectCommandContext {
  const projectInstanceId = useProjectIOStore.getState().projectInstanceId;
  if (!projectInstanceId) {
    throw new Error('No active project identity');
  }
  const lifecycle = projectPublicationCoordinator.captureCommandLifecycle();
  if (lifecycle.projectInstanceId !== projectInstanceId) {
    throw new Error('Project identity does not match publication lifecycle');
  }
  const operationId = crypto.randomUUID();
  const isCurrent = () => (
    useProjectIOStore.getState().projectInstanceId === projectInstanceId
    && projectPublicationCoordinator.ownsCommandLifecycle(projectInstanceId, lifecycle.epoch)
  );
  return {
    projectInstanceId,
    projectEpoch: lifecycle.epoch,
    publicationRevision: lifecycle.publicationRevision,
    operationId,
    operationPendingKey: `${projectInstanceId}:${operationId}`,
    isCurrent,
    assertCurrent: () => {
      if (useProjectIOStore.getState().projectInstanceId !== projectInstanceId) {
        throw new Error('Project command completion is stale');
      }
      projectPublicationCoordinator.assertCommandLifecycle(projectInstanceId, lifecycle.epoch);
    },
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
