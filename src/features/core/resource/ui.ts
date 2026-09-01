import { markResourceDirty } from "./documentStateActions";
import { useDocumentStateStore } from "./documentStateStore";
import type { ResourceKey, ResourceRef } from "./resourceTypes";

export interface ResourceUiCapability {
  readonly setDocumentDirty: (resource: ResourceRef, dirty: boolean) => void;
  readonly setDraft: (resourceKey: ResourceKey, draft: unknown) => void;
  readonly clearDraft: (resourceKey: ResourceKey) => void;
}

export const resourceUi: ResourceUiCapability = {
  setDocumentDirty: (resource, dirty) => markResourceDirty(resource, dirty),
  setDraft: (resourceKey, draft) => {
    useDocumentStateStore.getState().patchDocument(resourceKey, { draft });
  },
  clearDraft: (resourceKey) => {
    useDocumentStateStore.getState().patchDocument(resourceKey, { draft: undefined });
  },
};
