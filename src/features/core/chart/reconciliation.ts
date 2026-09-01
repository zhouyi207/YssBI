import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { rebaseChartDraft } from "./publication";

export interface ChartDraftReconciliation {
  rebaseCommittedDraft(
    chartPath: string,
    committed: DeepReadonly<ChartDocument>,
    expectedDraftFingerprint: string,
  ): "rebased" | "draft-changed";
}

export const chartDraftReconciliation: ChartDraftReconciliation = {
  rebaseCommittedDraft: rebaseChartDraft,
};
