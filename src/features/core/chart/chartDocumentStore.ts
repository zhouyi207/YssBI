import { create } from "zustand";
import type { ChartDocument } from "@/shared/types/domain/chart";

import { markResourceDirty, markResourceLoaded } from "@/features/core/resource";

interface ChartDocumentStore {
  documents: Record<string, ChartDocument>;
  upsertDocument(chartPath: string, document: ChartDocument): void;
  clear(): void;
  updateDocument(chartPath: string, patch: Partial<ChartDocument>): ChartDocument | null;
}

export const useChartDocumentStore = create<ChartDocumentStore>((set, get) => ({
  documents: {},

  upsertDocument: (chartPath, document) =>
    set((state) => {
      markResourceLoaded({ id: chartPath, kind: "chart" });
      return { documents: { ...state.documents, [chartPath]: document } };
    }),

  clear: () => set({ documents: {} }),

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
