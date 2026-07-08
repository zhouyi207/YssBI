import { isUntitledGraphPath } from '@/shared/types/domain/graphResourcePath';
import { useDocumentStateStore, type DocumentState } from './documentStateStore';
import { useResourceStore } from './resourceStore';
import type { ResourceKind, ResourceRef } from './resourceTypes';
import { resourceKey, type ResourceKey } from './resourceTypes';

function emptyDocumentState(key: ResourceKey): DocumentState {
  return {
    resourceKey: key,
    loaded: false,
    dirty: false,
    stale: false,
    missing: false,
    conflict: false,
    version: 0,
  };
}

function updateDocumentState(ref: ResourceRef, updater: (previous: DocumentState) => DocumentState): DocumentState {
  const key = resourceKey(ref);
  const previous = useDocumentStateStore.getState().documents[key] ?? emptyDocumentState(key);
  const next = updater(previous);
  useDocumentStateStore.getState().upsertDocument(next);
  useResourceStore.getState().patchResource(ref, {
    loaded: next.loaded,
    exists: !next.missing,
    hasDirtyDocument: next.dirty,
    hasStaleDocument: next.stale,
    hasConflictDocument: next.conflict,
  });
  return next;
}

export function markResourceLoaded(ref: ResourceRef, loaded = true): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    loaded,
    missing: false,
    version: previous.version + (loaded ? 1 : 0),
    lastLoadedAt: loaded ? Date.now() : previous.lastLoadedAt,
  }));
}

export function markResourceDirty(ref: ResourceRef, dirty: boolean): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    loaded: true,
    dirty,
    stale: dirty ? previous.stale : false,
    conflict: dirty ? previous.conflict : false,
    missing: false,
    version: previous.version + 1,
    lastSavedAt: dirty ? previous.lastSavedAt : Date.now(),
  }));
}

export function markResourceExternalChanged(ref: ResourceRef): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    loaded: previous.loaded,
    stale: previous.loaded && !previous.dirty,
    conflict: previous.loaded && previous.dirty,
    missing: false,
    diskVersion: (previous.diskVersion ?? previous.version) + 1,
  }));
}

export function markResourceMissing(ref: ResourceRef): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    missing: true,
    stale: false,
    conflict: previous.loaded && previous.dirty,
    diskVersion: (previous.diskVersion ?? previous.version) + 1,
  }));
}

export function clearResourceDocumentState(ref: ResourceRef): void {
  const key = resourceKey(ref);
  useDocumentStateStore.getState().removeDocument(key);
  if (isUntitledGraphPath(ref.id)) {
    useResourceStore.getState().removeResource(ref);
    return;
  }
  useResourceStore.getState().patchResource(ref, {
    loaded: false,
    exists: true,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
}

/** Move document state when a graph resource path changes on disk. */
export function migrateDocumentStatePath(
  from: string,
  to: string,
  kind: Extract<ResourceKind, 'event' | 'function'>,
): void {
  const fromKey = resourceKey({ id: from, kind });
  const toKey = resourceKey({ id: to, kind });
  const store = useDocumentStateStore.getState();
  const previous = store.documents[fromKey];
  if (!previous) return;

  store.removeDocument(fromKey);
  store.upsertDocument({ ...previous, resourceKey: toKey });
  useResourceStore.getState().patchResource({ id: to, kind }, {
    loaded: previous.loaded,
    exists: !previous.missing,
    hasDirtyDocument: previous.dirty,
    hasStaleDocument: previous.stale,
    hasConflictDocument: previous.conflict,
  });
}
