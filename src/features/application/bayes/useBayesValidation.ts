import { useMemo, useState } from 'react';
import type { BayesModelDraftDTO, ValidationReportDTO } from '@/shared/types/bayes';
import { hashBayesDraft, validateBayesDraftLocally } from '@/features/domain/bayes';
import { validateBayesModel } from '@/services/bayes';

export function useBayesValidation(draft: BayesModelDraftDTO, draftHash: string) {
  const [report, setReport] = useState<ValidationReportDTO | null>(null);
  const [validatedHash, setValidatedHash] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const stale = report !== null && validatedHash !== draftHash;

  const validate = async () => {
    setLoading(true);
    try {
      const nextReport = await validateBayesModel(draft).catch(() => validateBayesDraftLocally(draft));
      setReport(nextReport);
      setValidatedHash(hashBayesDraft(draft));
      return nextReport;
    } finally {
      setLoading(false);
    }
  };

  return useMemo(
    () => ({ report, stale, loading, validate }),
    [report, stale, loading],
  );
}
