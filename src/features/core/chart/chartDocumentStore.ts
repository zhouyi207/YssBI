import { create } from "zustand";
import type { ChartDocument, ChartIndexEntry } from "@/shared/types/domain/chart";

import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
} from "@/features/core/resource";

interface ChartDocumentStore {
  index: ChartIndexEntry[];
  documents: Record<string, ChartDocument>;
  setIndex(entries: ChartIndexEntry[]): void;
  upsertDocument(chartPath: string, document: ChartDocument): void;
  removeDocument(chartPath: string): void;
  clear(): void;
  updateDocument(chartPath: string, patch: Partial<ChartDocument>): ChartDocument | null;
  markDirty(chartPath: string): void;
}

export const useChartDocumentStore = create<ChartDocumentStore>((set, get) => ({
  index: [],
  documents: {},

  setIndex: (entries) => set({ index: entries }),

  upsertDocument: (chartPath, document) =>
    set((state) => {
      markResourceLoaded({ id: chartPath, kind: "chart" });
      return { documents: { ...state.documents, [chartPath]: document } };
    }),

  removeDocument: (chartPath) =>
    set((state) => {
      clearResourceDocumentState({ id: chartPath, kind: "chart" });
      const documents = { ...state.documents };
      delete documents[chartPath];
      return {
        index: state.index.filter((entry) => entry.chartPath !== chartPath),
        documents,
      };
    }),

  clear: () => set({ index: [], documents: {} }),

  updateDocument: (chartPath, patch) => {
    const current = get().documents[chartPath];
    if (!current) return null;
    const next: ChartDocument = {
      ...current,
      ...patch,
      encodings: { ...current.encodings, ...patch.encodings },
    };
    get().upsertDocument(chartPath, next);
    get().markDirty(chartPath);
    return next;
  },

  markDirty: (chartPath) => {
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
  },
}));
