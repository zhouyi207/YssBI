import {
  useResourceStore,
  type ResourceRef,
} from '@/features/core/resource';
import { commitAfterCommand } from './resourceIndexCoordinator';
import { getGraphProjectionBasis } from '@/features/core/dataStore/graphEntityAccess';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { DatabaseService } from '@/services/database/databaseService';
import { GraphService } from '@/services/graph/graphService';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { captureProjectCommandContext } from '@/features/application/projectCommandContext';
import { beginGraphRenameLifecycle } from '@/features/application/editorProjection/graphProjectionCoordinator';
import {
  beginWorksheetRenameLifecycle,
  isWorksheetLifecycleCurrent,
} from '@/features/application/editor/worksheetLifecycleCoordinator';
import type { ResourceMutationResultDto } from '@/shared/types/domain/editorMutation';

import type { GraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { deleteVariableAction, renameVariableAction } from '@/features/application/dataManagement/variableActions';
import { executeDatabaseMutation } from '@/features/application/dataManagement/databaseMutation';

export type { GraphResourceKind };

function graphRevision(graphPath: string): number {
  const basis = getGraphProjectionBasis(useGraphDataStore.getState(), graphPath);
  if (basis) return basis.graphRevision;

  const resource = Object.values(useResourceStore.getState().resources)
    .find((candidate) => candidate.id === graphPath
      && (candidate.kind === 'event' || candidate.kind === 'function'));
  if (resource?.revision == null) {
    throw new Error(`Graph resource '${graphPath}' has no authoritative revision`);
  }
  return resource.revision;
}

function worksheetRevision(worksheetPath: string): number {
  const resource = Object.values(useResourceStore.getState().resources)
    .find((candidate) => candidate.id === worksheetPath && candidate.kind === 'worksheet');
  if (resource?.revision == null) {
    throw new Error(`Worksheet resource '${worksheetPath}' has no authoritative revision`);
  }
  return resource.revision;
}

function mutationGraphPath(result: ResourceMutationResultDto): string {
  const paths = result.projectionStatus.status === 'complete'
    ? result.projectionStatus.expectedGraphPaths
    : result.projectionStatus.invalidatedGraphPaths;
  const path = paths.find((candidate) =>
    candidate.startsWith('events/') || candidate.startsWith('functions/'));
  if (!path) throw new Error('Resource mutation result omitted its graph path');
  return path;
}

async function submitCurrentResult(
  context: ReturnType<typeof captureProjectCommandContext>,
  result: ResourceMutationResultDto,
): Promise<void> {
  context.assertCurrent();
  if (result.projectInstanceId !== context.projectInstanceId) {
    throw new Error('stale project lifecycle for resource mutation');
  }
  await projectPublicationCoordinator.submit({ result });
  context.assertCurrent();
}

export async function commitFileFirstResourceIndex(): Promise<boolean> {
  return commitAfterCommand();
}

export async function renameResource(ref: ResourceRef, nextName: string): Promise<void> {
  if (!nextName) return;
  const name = nextName;

  if (ref.kind === 'event' || ref.kind === 'function') {
    const context = captureProjectCommandContext();
    const expectedRevision = graphRevision(ref.id);
    const lifecycleToken = beginGraphRenameLifecycle(ref.id);
    const result = await GraphService.renameGraphResource(
      context.projectInstanceId,
      ref.id,
      expectedRevision,
      name,
      lifecycleToken,
      context.operationId,
    );
    await submitCurrentResult(context, result);
    return;
  }

  if (ref.kind === 'worksheet') {
    const context = captureProjectCommandContext();
    const expectedRevision = worksheetRevision(ref.id);
    const lifecycleToken = beginWorksheetRenameLifecycle(context.projectInstanceId, ref.id);
    const result = await WorksheetService.renameWorksheet(
      context.projectInstanceId,
      context.operationId,
      ref.id,
      expectedRevision,
      name,
      lifecycleToken,
    );
    context.assertCurrent();
    if (!isWorksheetLifecycleCurrent(context.projectInstanceId, ref.id, lifecycleToken)) {
      throw Object.assign(
        new Error(`stale resource lifecycle for worksheet '${ref.id}'`),
        { code: 'stale_resource_lifecycle' },
      );
    }
    await submitCurrentResult(context, result);
    return;
  }

  if (ref.kind === 'database') {
    await executeDatabaseMutation(ref.id, (authority) => DatabaseService.renameDatabase(
      authority.projectInstanceId,
      authority.operationId,
      authority.expectedRevision,
      ref.id,
      name,
    ));
    return;
  }

  if (ref.kind === 'variable') {
    await renameVariableAction(ref.id, name);
    return;
  }

  useResourceStore.getState().patchResource({ id: ref.id, kind: ref.kind }, { name });
}

export async function createGraphResource(kind: GraphResourceKind, name?: string): Promise<string> {
  const graphName = name?.trim() || (kind === 'event' ? DEFAULT_EVENT_NAME : DEFAULT_FUNCTION_NAME);
  const context = captureProjectCommandContext();
  const result = kind === 'event'
    ? await GraphService.createEvent(context.projectInstanceId, graphName, context.operationId)
    : await GraphService.createFunction(context.projectInstanceId, graphName, context.operationId);
  await submitCurrentResult(context, result);
  await commitFileFirstResourceIndex();
  context.assertCurrent();
  return mutationGraphPath(result);
}

export async function duplicateGraphResource(graphPath: string): Promise<string> {
  const context = captureProjectCommandContext();
  const result = await GraphService.duplicateGraph(
    context.projectInstanceId,
    graphPath,
    graphRevision(graphPath),
    context.operationId,
  );
  await submitCurrentResult(context, result);
  await commitFileFirstResourceIndex();
  context.assertCurrent();
  return mutationGraphPath(result);
}

export async function deleteResource(ref: ResourceRef): Promise<void> {
  if (ref.kind === 'event' || ref.kind === 'function') {
    const context = captureProjectCommandContext();
    const expectedRevision = graphRevision(ref.id);
    const result = await GraphService.removeGraph(
      context.projectInstanceId,
      ref.id,
      expectedRevision,
      context.operationId,
    );
    await submitCurrentResult(context, result);
    await commitFileFirstResourceIndex();
    return;
  }

  if (ref.kind === 'database') {
    await executeDatabaseMutation(ref.id, (authority) => DatabaseService.deleteDatabase(
      authority.projectInstanceId,
      authority.operationId,
      authority.expectedRevision,
      ref.id,
    ));
    return;
  }

  if (ref.kind === 'variable') {
    await deleteVariableAction(ref.id);
    return;
  }
}
