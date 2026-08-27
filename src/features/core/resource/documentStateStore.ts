import { create } from 'zustand';
import type { ResourceKey } from './resourceTypes';

export interface DocumentState {
  resourceKey: ResourceKey;
  loaded: boolean;
  dirty: boolean;
  draft?: unknown;
  stale: boolean;
  missing: boolean;
  conflict: boolean;
  version: number;
  diskVersion?: number;
  lastLoadedAt?: number;
  lastSavedAt?: number;
}

interface DocumentStateStore {
  documents: Record<ResourceKey, DocumentState>;
  upsertDocument(document: DocumentState): void;
  patchDocument(resourceKey: ResourceKey, patch: Partial<DocumentState>): void;
  removeDocument(resourceKey: ResourceKey): void;
  clear(): void;
}

export const useDocumentStateStore = create<DocumentStateStore>((set) => ({
  documents: {},

  upsertDocument: (document) =>
    set((state) => ({
      documents: {
        ...state.documents,
        [document.resourceKey]: document,
      },
    })),

  patchDocument: (resourceKey, patch) =>
    set((state) => {
      const previous = state.documents[resourceKey];
      if (!previous) return state;
      return {
        documents: {
          ...state.documents,
          [resourceKey]: { ...previous, ...patch },
        },
      };
    }),

  removeDocument: (resourceKey) =>
    set((state) => {
      if (!state.documents[resourceKey]) return state;
      const next = { ...state.documents };
      delete next[resourceKey];
      return { documents: next };
    }),

  clear: () => set({ documents: {} }),
}));
