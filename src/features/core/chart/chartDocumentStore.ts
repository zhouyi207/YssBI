import { create } from "zustand";
import type { ChartDocument } from "@/shared/types/domain/chart";

import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
} from "@/features/core/resource";

interface ChartDocumentStore {
  documents: Record<string, ChartDocument>;
  upsertDocument(chartPath: string, document: ChartDocument): void;
  beginRead(chartPath: string): () => boolean;
  removeDocument(chartPath: string): void;
  clear(): void;
  updateDocument(chartPath: string, patch: Partial<ChartDocument>): ChartDocument | null;
}

const reads = new Map<string, object>();

export const useChartDocumentStore = create<ChartDocumentStore>((set, get) => ({
  documents: {},

  beginRead: (chartPath) => {
    const token = {};
    reads.set(chartPath, token);
    return () => reads.get(chartPath) === token;
  },

  upsertDocument: (chartPath, document) => {
    reads.delete(chartPath);
    set((state) => {
      markResourceLoaded({ id: chartPath, kind: "chart" });
      return { documents: { ...state.documents, [chartPath]: document } };
    });
  },

  removeDocument: (chartPath) => {
    reads.delete(chartPath);
    set((state) => {
      const documents = { ...state.documents };
      delete documents[chartPath];
      return { documents };
    });
    clearResourceDocumentState({ id: chartPath, kind: "chart" });
  },

  clear: () => {
    reads.clear();
    set({ documents: {} });
  },

  updateDocument: (chartPath, patch) => {
    const current = get().documents[chartPath];
    if (!current) return null;
    const next: ChartDocument = {
      ...current,
      ...patch,
      encodings: { ...current.encodings, ...patch.encodings },
    };
    get().upsertDocument(chartPath, next);
    markResourceDirty({ id: chartPath, kind: "chart" }, true);
    return next;
  },
}));
