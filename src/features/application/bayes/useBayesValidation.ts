import { useCallback, useMemo, useState } from 'react';
import type { BayesModelDraftDTO, ValidationReportDTO } from '@/shared/types/bayes';
import { validateBayesDraftLocally } from '@/features/domain/bayes';
import { validateBayesModel } from '@/services/bayes';

export function useBayesValidation(draft: BayesModelDraftDTO, draftHash: string) {
  const [report, setReport] = useState<ValidationReportDTO | null>(null);
  const [validatedHash, setValidatedHash] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const stale = report !== null && validatedHash !== draftHash;

  const validate = useCallback(async () => {
    setLoading(true);
    try {
      const nextReport = await validateBayesModel(draft).catch(() => validateBayesDraftLocally(draft));
      setReport(nextReport);
      setValidatedHash(draftHash);
      return nextReport;
    } finally {
      setLoading(false);
    }
  }, [draft, draftHash]);

  return useMemo(
    () => ({ report, stale, loading, validate }),
    [report, stale, loading, validate],
  );
}
