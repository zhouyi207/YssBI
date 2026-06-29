import { create } from 'zustand';
import type { GraphFolderMeta } from './resourceSnapshotReconcile';
import type { ProjectResourceMeta, ResourceKey, ResourceRef } from './resourceTypes';
import { resourceKey } from './resourceTypes';

interface ResourceStore {
  resources: Record<ResourceKey, ProjectResourceMeta>;
  graphFolders: GraphFolderMeta[];
  graphOrder: string[];
  setSnapshot(snapshot: {
    resources: ProjectResourceMeta[];
    graphFolders?: GraphFolderMeta[];
    graphOrder?: string[];
  }): void;
  setResources(resources: ProjectResourceMeta[]): void;
  upsertResource(resource: ProjectResourceMeta): void;
  patchResource(ref: ResourceRef, patch: Partial<ProjectResourceMeta>): void;
  removeResource(ref: ResourceRef): void;
  clear(): void;
}

export const useResourceStore = create<ResourceStore>((set) => ({
  resources: {},
  graphFolders: [],
  graphOrder: [],

  setSnapshot: ({ resources, graphFolders, graphOrder }) =>
    set({
      resources: Object.fromEntries(
        resources.map((resource) => [resourceKey(resource), resource]),
      ) as Record<ResourceKey, ProjectResourceMeta>,
      graphFolders: graphFolders ?? [],
      graphOrder: graphOrder ?? resources
        .filter((resource) => resource.kind === 'event' || resource.kind === 'function')
        .map((resource) => resource.id),
    }),

  setResources: (resources) =>
    set({
      resources: Object.fromEntries(
        resources.map((resource) => [resourceKey(resource), resource]),
      ) as Record<ResourceKey, ProjectResourceMeta>,
    }),

  upsertResource: (resource) =>
    set((state) => ({
      resources: {
        ...state.resources,
        [resourceKey(resource)]: resource,
      },
      graphOrder: state.graphOrder.includes(resource.id)
        ? state.graphOrder
        : resource.kind === 'event' || resource.kind === 'function'
          ? [...state.graphOrder, resource.id]
          : state.graphOrder,
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

  removeResource: (ref) =>
    set((state) => {
      const key = resourceKey(ref);
      if (!state.resources[key]) return state;
      const next = { ...state.resources };
      delete next[key];
      return {
        resources: next,
        graphOrder: state.graphOrder.filter((id) => id !== ref.id),
      };
    }),

  clear: () => set({ resources: {}, graphFolders: [], graphOrder: [] }),
}));
