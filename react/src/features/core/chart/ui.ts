import { useResourceStore } from "@/features/core/resource/resourceStore";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument } from "@/shared/types/domain/chart";

export interface ChartUi {
  updateDraft(
    chartPath: string,
    patch: DeepReadonly<Partial<ChartDocument>>,
  ): DeepReadonly<ChartDocument> | null;
}

export const chartUi: ChartUi = {
  updateDraft: (chartPath, patch) =>
    useResourceStore
      .getState()
      .updateChartDocument(chartPath, structuredClone(patch) as Partial<ChartDocument>),
};
