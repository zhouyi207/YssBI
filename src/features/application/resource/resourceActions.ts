import { useGraphDataStore, useProjectIOStore, useDatabaseStore } from '@/features/core/dataStore';
import {
  graphResourceRef,
  normalizeBackendResourceMeta,
  updateOpenResourceLabels,
  useResourceStore,
  type ResourceRef,
} from '@/features/core/resource';
import { DatabaseService } from '@/services/database/databaseService';
import { GraphService } from '@/services/graph/graphService';
import { closeEditorTab } from '@/features/application/editor/closeEditorTab';import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import { deleteVariableAction, renameVariableAction } from '@/features/application/dataManagement/variableActions';

export type GraphResourceKind = 'event' | 'function';

async function refreshResourceIndex(): Promise<void> {
  await useProjectIOStore.getState().refreshResourceIndex();
}

export async function renameResource(ref: ResourceRef, nextName: string): Promise<void> {
  const name = nextName.trim();
  if (!name) return;

  if (ref.kind === 'event' || ref.kind === 'function') {
    const backendMeta = await GraphService.renameGraphResource(ref.id, name);
    const meta = normalizeBackendResourceMeta(backendMeta);
    useResourceStore.getState().upsertResource(meta);
    updateOpenResourceLabels(graphResourceRef(ref.id, meta.kind === 'function' ? 'function' : 'event'), meta.name);
    return;
  }

  if (ref.kind === 'database') {
    await DatabaseService.renameDatabase(ref.id, name);
    useDatabaseStore.getState().updateDatabase(ref.id, { name });
    useResourceStore.getState().patchResource(ref, { name });
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
  const id = kind === 'event'
    ? await GraphService.createEvent(graphName)
    : await GraphService.createFunction(graphName);
  await refreshResourceIndex();
  return id;
}

export async function duplicateGraphResource(graphId: string): Promise<void> {
  await GraphService.duplicateGraph(graphId);
  await refreshResourceIndex();
}

export async function deleteResource(ref: ResourceRef): Promise<void> {
  if (ref.kind === 'event' || ref.kind === 'function') {
    await closeEditorTab(ref.id, undefined, true);
    await GraphService.removeGraph(ref.id);
    useGraphDataStore.getState().clearGraph(ref.id);
    useResourceStore.getState().removeResource(ref);
    await refreshResourceIndex();
    return;
  }

  if (ref.kind === 'database') {
    await DatabaseService.deleteDatabase(ref.id);
    useDatabaseStore.getState().deleteDatabase(ref.id);
    useResourceStore.getState().removeResource(ref);
    return;
  }

  if (ref.kind === 'variable') {
    await deleteVariableAction(ref.id);
    return;
  }
}
