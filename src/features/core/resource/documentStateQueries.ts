import { useDocumentStateStore } from './documentStateStore';
import type { ResourceRef } from './resourceTypes';
import { resourceKey } from './resourceTypes';

export function getDocumentState(ref: ResourceRef) {
  return useDocumentStateStore.getState().documents[resourceKey(ref)];
}

export function isResourceDocumentDirty(ref: ResourceRef): boolean {
  return getDocumentState(ref)?.dirty ?? false;
}

export function isGraphResourceDirty(
  graphPath: string,
  kind?: 'event' | 'function' | 'worksheet',
): boolean {
  if (kind) {
    return isResourceDocumentDirty({ id: graphPath, kind });
  }
  return (
    isResourceDocumentDirty({ id: graphPath, kind: 'event' }) ||
    isResourceDocumentDirty({ id: graphPath, kind: 'function' }) ||
    isResourceDocumentDirty({ id: graphPath, kind: 'worksheet' })
  );
}
