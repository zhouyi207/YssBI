import { useGraphDataStore, useGraphMetaStore, useProjectIOStore, useDatabaseStore } from '@/features/core/dataStore';
import {
  graphResourceRef,
  normalizeBackendResourceMeta,
  useResourceStore,
  type ResourceRef,
} from '@/features/core/resource';
import { DatabaseService } from '@/services/database/databaseService';
import { GraphService } from '@/services/graph/graphService';
import { closeEditorTab } from '@/features/application/editor/closeEditorTab';
import { DEFAULT_EVENT_NAME, DEFAULT_FUNCTION_NAME } from '@/shared/constants/defaultResourceNames';
import type { GraphResourceKind } from '@/shared/types/domain/graphResourcePath';
import { deleteVariableAction, renameVariableAction } from '@/features/application/dataManagement/variableActions';

export type { GraphResourceKind };

export async function commitFileFirstResourceIndex(): Promise<void> {
  await useProjectIOStore.getState().refreshResourceIndex();
}

export async function renameResource(ref: ResourceRef, nextName: string): Promise<void> {
  const name = nextName.trim();
  if (!name) return;

  if (ref.kind === 'event' || ref.kind === 'function') {
    const backendMeta = await GraphService.renameGraphResource(ref.id, name);
    const meta = normalizeBackendResourceMeta(backendMeta);
    const targetRef = graphResourceRef(meta.id, ref.kind);

    // Path migration is owned by GraphResourceMoved; patch name/uri only to preserve document state.
    useResourceStore.getState().patchResource(targetRef, {
      name: meta.name,
      uri: meta.uri,
      exists: meta.exists,
      loaded: meta.loaded,
    });
    useGraphMetaStore.getState().updateGraph(meta.id, { name: meta.name });
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
  const path = kind === 'event'
    ? await GraphService.createEvent(graphName)
    : await GraphService.createFunction(graphName);
  await commitFileFirstResourceIndex();
  return path;
}

export async function duplicateGraphResource(graphPath: string): Promise<string> {
  const newPath = await GraphService.duplicateGraph(graphPath);
  await commitFileFirstResourceIndex();
  return newPath;
}

export async function deleteResource(ref: ResourceRef): Promise<void> {
  if (ref.kind === 'event' || ref.kind === 'function') {
    await closeEditorTab(ref.id, undefined, true);
    await GraphService.removeGraph(ref.id);
    useGraphDataStore.getState().clearGraph(ref.id);
    useGraphMetaStore.getState().deleteGraph(ref.id);
    useResourceStore.getState().removeResource(ref);
    await commitFileFirstResourceIndex();
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
