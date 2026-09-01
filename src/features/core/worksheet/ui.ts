import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { WorksheetDocument } from "@/shared/types/domain/worksheet";
import { applyWorksheetDraftUpdate, discardWorksheetDraft } from "./publication";

export interface WorksheetUi {
  updateDraft(
    worksheetPath: string,
    patch: DeepReadonly<Partial<WorksheetDocument>>,
  ): DeepReadonly<WorksheetDocument> | null;
  discardDraft(worksheetPath: string): void;
}

export const worksheetUi: WorksheetUi = {
  updateDraft: applyWorksheetDraftUpdate,
  discardDraft: discardWorksheetDraft,
};
