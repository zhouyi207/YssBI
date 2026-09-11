import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { ResultAnalysisRequest } from "@/shared/types/domain/resultReport";
import type { ResultReference } from "@/shared/types/domain/result";
import { resultQueryCoordinator, resultQueryRead } from "./runtime";
import { resultAnalysisParameters } from "./resultQueryCoordinator";

export function useResultAnalysis(reference: ResultReference) {
  const generation = useRef(0);
  useEffect(() => {
    const release = resultQueryCoordinator.retainPayload(reference);
    return () => {
      generation.current += 1;
      release();
    };
  }, [reference.executionSessionId, reference.resultId]);

  return useCallback(
    async (analysis: ResultAnalysisRequest) => {
      const current = generation.current;
      const request = { reference, analysis };
      const outcome = await resultQueryCoordinator.loadAnalysis(request);
      if (generation.current !== current || outcome.status === "stale") return null;
      if (outcome.status === "failed")
        throw resultQueryRead.getFailure({ kind: "analysis", ...request });
      return resultQueryRead.getAnalysis(request);
    },
    [reference.executionSessionId, reference.resultId],
  );
}

export function useResultAnalysisQuery(
  reference: ResultReference,
  analysis: ResultAnalysisRequest,
) {
  const run = useResultAnalysis(reference);
  const [loading, setLoading] = useState(true);
  const request = { reference, analysis };
  const parameters = resultAnalysisParameters(request);
  const value = useSyncExternalStore(
    resultQueryRead.subscribe,
    () => resultQueryRead.getAnalysis(request),
    () => resultQueryRead.getAnalysis(request),
  );
  const error = resultQueryRead.getFailure({ kind: "analysis", ...request });
  const reload = useCallback(async () => {
    setLoading(true);
    try {
      await run(analysis);
    } catch {
      /* The coordinator publishes the read error. */
    } finally {
      setLoading(false);
    }
  }, [run, analysis.kind, parameters]);
  useEffect(() => {
    void reload();
  }, [reload]);
  return { value, loading, error, reload };
}
