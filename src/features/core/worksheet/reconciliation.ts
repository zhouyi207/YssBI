import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import { rebaseWorksheetDraft } from './publication';

export interface WorksheetDraftReconciliation {
  rebaseCommittedDraft(
    worksheetPath: string,
    committed: DeepReadonly<WorksheetDocument>,
    expectedDraftFingerprint: string,
  ): 'rebased' | 'draft-changed';
}

export const worksheetDraftReconciliation: WorksheetDraftReconciliation = {
  rebaseCommittedDraft: rebaseWorksheetDraft,
};
