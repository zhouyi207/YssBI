import { useCallback, useEffect, useState } from 'react';

import type { ErrorReference } from '@/features/application/errorReference';
import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { useWorksheetRead } from '@/features/core/worksheet/read';
import { worksheetUi } from '@/features/core/worksheet/ui';
import type { WorksheetDocument } from '@/shared/types/domain/worksheet';
import type {
  WorksheetCoordinator,
  WorksheetLoadOutcome,
  WorksheetSaveOutcome,
} from './worksheetCoordinator';

export interface WorksheetViewModel {
  readonly document: DeepReadonly<WorksheetDocument> | null;
  readonly dirty: boolean;
  readonly loading: boolean;
  readonly saving: boolean;
  readonly issue: ErrorReference | null;
  update(patch: Partial<WorksheetDocument>): void;
  save(): Promise<WorksheetSaveOutcome>;
  reload(): Promise<WorksheetLoadOutcome>;
}

function issueFor(
  operation: 'load' | 'save',
  status: 'failed' | 'rejected' | 'unknown',
): ErrorReference {
  return {
    code: `worksheet_${operation}_${status}`,
    incidentId: null,
  };
}

export function useWorksheetViewModel(
  worksheetPath: string,
  coordinator: WorksheetCoordinator,
): WorksheetViewModel {
  const projection = useWorksheetRead((state) => ({
    document: state.draftsByPath[worksheetPath]
      ?? state.documents[worksheetPath]
      ?? null,
    dirty: state.dirtyByPath[worksheetPath] === true,
  }));
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [issue, setIssue] = useState<ErrorReference | null>(null);

  useEffect(() => {
    setLoading(false);
    setSaving(false);
    setIssue(null);
  }, [worksheetPath]);

  const update = useCallback((patch: Partial<WorksheetDocument>): void => {
    worksheetUi.updateDraft(worksheetPath, patch);
    setIssue(null);
  }, [worksheetPath]);

  const save = useCallback(async (): Promise<WorksheetSaveOutcome> => {
    setSaving(true);
    setIssue(null);
    try {
      const outcome = await coordinator.save(worksheetPath);
      if (outcome.status === 'failed'
        || outcome.status === 'rejected'
        || outcome.status === 'unknown') {
        setIssue(issueFor('save', outcome.status));
      }
      return outcome;
    } catch {
      const outcome = { status: 'failed' as const };
      setIssue(issueFor('save', outcome.status));
      return outcome;
    } finally {
      setSaving(false);
    }
  }, [coordinator, worksheetPath]);

  const reload = useCallback(async (): Promise<WorksheetLoadOutcome> => {
    setLoading(true);
    setIssue(null);
    try {
      const outcome = await coordinator.load(worksheetPath);
      if (outcome.status === 'failed') setIssue(issueFor('load', outcome.status));
      return outcome;
    } catch {
      const outcome = { status: 'failed' as const };
      setIssue(issueFor('load', outcome.status));
      return outcome;
    } finally {
      setLoading(false);
    }
  }, [coordinator, worksheetPath]);

  return {
    document: projection.document,
    dirty: projection.dirty,
    loading,
    saving,
    issue,
    update,
    save,
    reload,
  };
}
