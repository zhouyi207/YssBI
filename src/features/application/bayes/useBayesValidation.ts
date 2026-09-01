import { useCallback, useMemo, useState } from "react";
import type { BayesModelDraftDTO, ValidationReportDTO } from "@/shared/types/bayes";
import { validateBayesModel } from "@/services/bayes";
import { normalizeBayesApplicationError, type BayesApplicationError } from "./bayesError";

export function useBayesValidation(draft: BayesModelDraftDTO, draftHash: string) {
  const [report, setReport] = useState<ValidationReportDTO | null>(null);
  const [validatedHash, setValidatedHash] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<BayesApplicationError | null>(null);

  const stale = report !== null && validatedHash !== draftHash;

  const validate = useCallback(async (): Promise<ValidationReportDTO | null> => {
    setLoading(true);
    setError(null);
    try {
      const nextReport = await validateBayesModel(draft);
      setReport(nextReport);
      setValidatedHash(draftHash);
      return nextReport;
    } catch (caught) {
      setReport(null);
      setValidatedHash(null);
      setError(normalizeBayesApplicationError(caught, "bayes_validation_request_failed"));
      return null;
    } finally {
      setLoading(false);
    }
  }, [draft, draftHash]);

  return useMemo(
    () => ({ report, stale, loading, error, validate }),
    [report, stale, loading, error, validate],
  );
}
