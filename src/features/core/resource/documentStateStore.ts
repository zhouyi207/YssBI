import { create } from "zustand";
import type { ResourceKey } from "./resourceTypes";

export interface DocumentState {
  resourceKey: ResourceKey;
  loaded: boolean;
  dirty: boolean;
  stale: boolean;
  missing: boolean;
  conflict: boolean;
}

interface DocumentStateStore {
  documents: Record<ResourceKey, DocumentState>;
  upsertDocument(document: DocumentState): void;
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

  removeDocument: (resourceKey) =>
    set((state) => {
      if (!state.documents[resourceKey]) return state;
      const next = { ...state.documents };
      delete next[resourceKey];
      return { documents: next };
    }),

  clear: () => set({ documents: {} }),
}));
