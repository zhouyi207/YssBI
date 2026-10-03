import {
  createDocumentState,
  withDirtyState,
  useResourceStore,
  type DocumentState,
} from "./resourceStore";
import type { ResourceRef } from "./resourceTypes";
import { resourceKey } from "./resourceTypes";

function updateDocumentState(
  ref: ResourceRef,
  updater: (previous: DocumentState) => DocumentState,
): void {
  const key = resourceKey(ref);
  const previous = useResourceStore.getState().documents[key] ?? createDocumentState(key);
  useResourceStore.getState().upsertDocument(updater(previous));
}

export function markResourceLoaded(ref: ResourceRef, loaded = true): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    loaded,
    missing: useResourceStore.getState().resources[resourceKey(ref)]?.exists === false,
  }));
}

export function markResourceStale(ref: ResourceRef, stale = true): void {
  updateDocumentState(ref, (previous) => ({
    ...previous,
    stale,
    conflict: stale ? previous.conflict : false,
  }));
}

export function markResourceDirty(ref: ResourceRef, dirty: boolean): void {
  updateDocumentState(ref, (previous) => withDirtyState(previous, dirty));
}

export function clearResourceDocumentState(ref: ResourceRef): void {
  const key = resourceKey(ref);
  useResourceStore.getState().removeDocument(key);
}
