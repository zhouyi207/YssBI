import { create } from "zustand";
import type { ProjectResourceMeta, ResourceKey, ResourceRef } from "./resourceTypes";
import { resourceKey } from "./resourceTypes";

interface ResourceStore {
  resources: Record<ResourceKey, ProjectResourceMeta>;
  graphOrder: string[];
  indexRevision: number;
  setSnapshot(snapshot: {
    resources: ProjectResourceMeta[];
    graphOrder?: string[];
    publicationRevision?: number;
  }): void;
  patchResource(ref: ResourceRef, patch: Partial<ProjectResourceMeta>): void;
  clear(): void;
}

export const useResourceStore = create<ResourceStore>((set) => ({
  resources: {},
  graphOrder: [],
  indexRevision: 0,

  setSnapshot: ({ resources, graphOrder, publicationRevision }) =>
    set((state) => ({
      indexRevision: publicationRevision ?? state.indexRevision,
      resources: Object.fromEntries(
        resources.map((resource) => [resourceKey(resource), resource]),
      ) as Record<ResourceKey, ProjectResourceMeta>,
      graphOrder:
        graphOrder ??
        resources
          .filter(
            (resource) => resource.kind === "event_graph" || resource.kind === "function_graph",
          )
          .map((resource) => resource.id),
    })),

  patchResource: (ref, patch) =>
    set((state) => {
      const key = resourceKey(ref);
      const previous = state.resources[key];
      if (!previous) return state;
      return {
        resources: {
          ...state.resources,
          [key]: { ...previous, ...patch },
        },
      };
    }),

  clear: () =>
    set({
      resources: {},
      graphOrder: [],
      indexRevision: 0,
    }),
}));
