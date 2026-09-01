import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { applyChartDraftUpdate, discardChartDraft } from "./publication";

export interface ChartUi {
  updateDraft(
    chartPath: string,
    patch: DeepReadonly<Partial<ChartDocument>>,
  ): DeepReadonly<ChartDocument> | null;
  discardDraft(chartPath: string): void;
}

export const chartUi: ChartUi = {
  updateDraft: applyChartDraftUpdate,
  discardDraft: discardChartDraft,
};
