import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useDocumentStateStore, type DocumentState } from './documentStateStore';
import { useResourceStore } from './resourceStore';
import type { ResourceRef } from './resourceTypes';
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
  const next = updateDocumentState(ref, (previous) => ({
    ...previous,
    loaded: true,
    dirty,
    stale: dirty ? previous.stale : false,
    conflict: dirty ? previous.conflict : false,
    missing: false,
    version: previous.version + 1,
    lastSavedAt: dirty ? previous.lastSavedAt : Date.now(),
  }));
  useLayoutStore.getState().setTabDirty(ref.id, next.dirty);
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
  useResourceStore.getState().patchResource(ref, {
    loaded: false,
    exists: true,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
  });
}
