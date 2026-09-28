import { useDocumentStateStore } from "./documentStateStore";
import type { ResourceRef, ResourceKey, ProjectResourceMeta } from "./resourceTypes";
import { resourceKey } from "./resourceTypes";
import { useResourceStore } from "./resourceStore";

export function getDocumentState(ref: ResourceRef) {
  return useDocumentStateStore.getState().documents[resourceKey(ref)];
}

export function isResourceDocumentDirty(ref: ResourceRef): boolean {
  return getDocumentState(ref)?.dirty ?? false;
}

/** Missing files keep their editor while either published or live state is dirty. */
export function shouldRetainResourceEditor(
  ref: ResourceRef,
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>> = useResourceStore.getState()
    .resources,
): boolean {
  const resource = resources[resourceKey(ref)];
  return (
    resource?.exists === true || resource?.hasDirtyDocument === true || isResourceDocumentDirty(ref)
  );
}

export function isGraphResourceDirty(graphPath: string): boolean {
  return (
    isResourceDocumentDirty({ id: graphPath, kind: "event_graph" }) ||
    isResourceDocumentDirty({ id: graphPath, kind: "function_graph" })
  );
}
