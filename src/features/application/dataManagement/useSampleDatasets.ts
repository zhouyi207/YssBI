import { useEffect, useState } from "react";
import { DatabaseService } from "@/services/database/databaseService";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import type { SampleDatasetSummary } from "@/shared/types/domain/database";

export function sampleDatasetFailure(error: unknown, command: string) {
  const failure = normalizeApplicationIpcError(command, error);
  return { code: failure.code, incidentId: failure.incidentId };
}

export function useSampleDatasets() {
  const [samples, setSamples] = useState<SampleDatasetSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<ReturnType<typeof sampleDatasetFailure> | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(null);
    void DatabaseService.listSampleDatasets().then(
      (result) => {
        if (active) {
          setSamples(result);
          setLoading(false);
        }
      },
      (failure: unknown) => {
        if (active) {
          setError(sampleDatasetFailure(failure, "list_sample_datasets"));
          setLoading(false);
        }
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);
  return { samples, loading, error, retry: () => setAttempt((value) => value + 1) };
}
