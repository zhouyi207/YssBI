import { useDocumentStateStore, type DocumentState } from "./documentStateStore";
import { useResourceStore } from "./resourceStore";
import type { ResourceRef } from "./resourceTypes";
import { resourceKey, type ResourceKey } from "./resourceTypes";

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

function updateDocumentState(
  ref: ResourceRef,
  updater: (previous: DocumentState) => DocumentState,
): DocumentState {
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

export function markResourceStale(ref: ResourceRef, stale = true): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    stale,
    conflict: stale ? previous.conflict : false,
    missing: false,
    version: previous.version + 1,
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
