import { useDocumentStateStore } from './documentStateStore';
import type { ResourceRef } from './resourceTypes';
import { resourceKey } from './resourceTypes';

export function getDocumentState(ref: ResourceRef) {
  return useDocumentStateStore.getState().documents[resourceKey(ref)];
}

export function isResourceDocumentDirty(ref: ResourceRef): boolean {
  return getDocumentState(ref)?.dirty ?? false;
}

export type PathDocumentResourceKind = 'event' | 'function' | 'worksheet';

export function isPathResourceDirty(
  resourcePath: string,
  kind: PathDocumentResourceKind,
): boolean {
  return isResourceDocumentDirty({ id: resourcePath, kind });
}

export function isGraphResourceDirty(
  graphPath: string,
  kind?: PathDocumentResourceKind,
): boolean {
  if (kind) return isPathResourceDirty(graphPath, kind);
  return isPathResourceDirty(graphPath, 'event')
    || isPathResourceDirty(graphPath, 'function');
}
