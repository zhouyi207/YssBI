import { create } from 'zustand';
import type { WorksheetDocument, WorksheetIndexEntry } from '@/shared/types/domain/worksheet';
import { WorksheetService } from '@/services/worksheet/worksheetService';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

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
    set((state) => ({
      documents: { ...state.documents, [document.id]: document },
      index: state.index.some((e) => e.id === document.id)
        ? state.index.map((e) =>
            e.id === document.id
              ? {
                  id: document.id,
                  name: document.name,
                  databaseId: document.databaseId,
                  chartType: document.chartType,
                  folderPath: document.folderPath ?? '',
                }
              : e,
          )
        : [
            ...state.index,
            {
              id: document.id,
              name: document.name,
              databaseId: document.databaseId,
              chartType: document.chartType,
              folderPath: document.folderPath ?? '',
            },
          ],
    })),

  removeDocument: (worksheetId) =>
    set((state) => ({
      index: state.index.filter((e) => e.id !== worksheetId),
      documents: Object.fromEntries(
        Object.entries(state.documents).filter(([id]) => id !== worksheetId),
      ),
    })),

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
      useLayoutStore.setState((state) => {
        for (const node of Object.values(state.nodes)) {
          const tab = node.data?.tabs?.find((item) => item.id === worksheetId);
          if (tab) tab.title = patch.name;
        }
      });
    }

    return next;
  },

  markDirty: (worksheetId) => {
    useLayoutStore.getState().setTabDirty(worksheetId, true);
  },

  saveDocument: async (worksheetId) => {
    const document = get().documents[worksheetId];
    if (!document) return;
    await WorksheetService.saveWorksheet(document);
    useLayoutStore.getState().setTabDirty(worksheetId, false);
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
    folderPath: doc.folderPath ?? '',
  }));
}
