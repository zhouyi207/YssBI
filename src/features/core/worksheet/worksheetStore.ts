import { create } from 'zustand';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';

import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
} from '@/features/core/resource';

interface WorksheetStore {
  index: WorksheetIndexEntry[];
  documents: Record<string, WorksheetDocument>;
  setIndex(entries: WorksheetIndexEntry[]): void;
  upsertDocument(worksheetPath: string, document: WorksheetDocument): void;
  removeDocument(worksheetPath: string): void;
  clear(): void;
  updateDocument(
    worksheetPath: string,
    patch: Partial<WorksheetDocument>,
  ): WorksheetDocument | null;
  markDirty(worksheetPath: string): void;
}

export const useWorksheetStore = create<WorksheetStore>((set, get) => ({
  index: [],
  documents: {},

  setIndex: (entries) => set({ index: entries }),

  upsertDocument: (worksheetPath, document) =>
    set((state) => {
      markResourceLoaded({ id: worksheetPath, kind: 'worksheet' });
      return { documents: { ...state.documents, [worksheetPath]: document } };
    }),

  removeDocument: (worksheetPath) =>
    set((state) => {
      clearResourceDocumentState({ id: worksheetPath, kind: 'worksheet' });
      const documents = { ...state.documents };
      delete documents[worksheetPath];
      return {
        index: state.index.filter((entry) => entry.worksheetPath !== worksheetPath),
        documents,
      };
    }),

  clear: () => set({ index: [], documents: {} }),

  updateDocument: (worksheetPath, patch) => {
    const current = get().documents[worksheetPath];
    if (!current) return null;
    const next: WorksheetDocument = {
      ...current,
      ...patch,
      encodings: { ...current.encodings, ...patch.encodings },
    };
    get().upsertDocument(worksheetPath, next);
    get().markDirty(worksheetPath);
    return next;
  },

  markDirty: (worksheetPath) => {
    markResourceDirty({ id: worksheetPath, kind: 'worksheet' }, true);
  },

}));
