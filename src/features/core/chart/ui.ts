import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { useChartDocumentStore } from "./chartDocumentStore";

export interface ChartUi {
  updateDraft(
    chartPath: string,
    patch: DeepReadonly<Partial<ChartDocument>>,
  ): DeepReadonly<ChartDocument> | null;
}

export const chartUi: ChartUi = {
  updateDraft: (chartPath, patch) =>
    useChartDocumentStore
      .getState()
      .updateDocument(chartPath, structuredClone(patch) as Partial<ChartDocument>),
};
