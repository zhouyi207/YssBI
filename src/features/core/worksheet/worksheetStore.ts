import { create } from 'zustand';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
  updateOpenResourceLabels,
  useResourceStore,
} from '@/features/core/resource';

interface WorksheetStore {
  index: WorksheetIndexEntry[];
  documents: Record<string, WorksheetDocument>;
  setIndex: (entries: WorksheetIndexEntry[]) => void;
  upsertDocument: (document: WorksheetDocument) => void;
  removeDocument: (worksheetId: string) => void;
  clear: () => void;
  updateDocument: (worksheetId: string, patch: Partial<WorksheetDocument>) => WorksheetDocument | null;
  markDirty: (worksheetId: string) => void;
  saveDocument: (worksheetId: string) => Promise<void>;
}

export const useWorksheetStore = create<WorksheetStore>((set, get) => ({
  index: [],
  documents: {},

  setIndex: (entries) => set({ index: entries }),

  upsertDocument: (document) =>
    set((state) => {
      markResourceLoaded({ id: document.id, kind: 'worksheet' });
      const indexEntry: WorksheetIndexEntry = {
        id: document.id,
        name: document.name,
        databaseId: document.databaseId,
        chartType: document.chartType,
      };
      return {
        documents: { ...state.documents, [document.id]: document },
        index: state.index.some((e) => e.id === document.id)
          ? state.index.map((e) => (e.id === document.id ? indexEntry : e))
          : [...state.index, indexEntry],
      };
    }),

  removeDocument: (worksheetId) =>
    set((state) => {
      clearResourceDocumentState({ id: worksheetId, kind: 'worksheet' });
      return {
        index: state.index.filter((e) => e.id !== worksheetId),
        documents: Object.fromEntries(
          Object.entries(state.documents).filter(([id]) => id !== worksheetId),
        ),
      };
    }),

  clear: () => set({ index: [], documents: {} }),

  updateDocument: (worksheetId, patch) => {
    const current = get().documents[worksheetId];
    if (!current) return null;
    const next: WorksheetDocument = {
      ...current,
      ...patch,
      encodings: { ...current.encodings, ...patch.encodings },
    };
    get().upsertDocument(next);
    get().markDirty(worksheetId);

    if (patch.name) {
      useResourceStore.getState().patchResource(
        { id: worksheetId, kind: 'worksheet' },
        { name: patch.name },
      );
      updateOpenResourceLabels({ id: worksheetId, kind: 'worksheet' }, patch.name);
    }

    return next;
  },

  markDirty: (worksheetId) => {
    markResourceDirty({ id: worksheetId, kind: 'worksheet' }, true);
  },

  saveDocument: async (worksheetId) => {
    const document = get().documents[worksheetId];
    if (!document) return;
    await WorksheetService.saveWorksheet(document);
    markResourceDirty({ id: worksheetId, kind: 'worksheet' }, false);
  },
}));

export function worksheetIndexFromDocuments(
  documents: Record<string, WorksheetDocument>,
): WorksheetIndexEntry[] {
  return Object.values(documents).map((doc) => ({
    id: doc.id,
    name: doc.name,
    databaseId: doc.databaseId,
    chartType: doc.chartType,
  }));
}
