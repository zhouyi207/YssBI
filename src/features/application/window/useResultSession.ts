import { useEffect, useState } from "react";
import { subscribeResultSessionEnd } from "@/services/result/resultSessionChannel";
import { resetResultQuery } from "@/features/application/results/runtime";
import type { ResultReference } from "@/shared/types/domain/result";

/** Output changes do not end a retained snapshot's lifetime. */
export function useResultSession(reference: ResultReference | null): boolean {
  const [invalidated, setInvalidated] = useState(false);
  useEffect(() => {
    setInvalidated(false);
    return subscribeResultSessionEnd((session) => {
      if (!reference || (session !== null && session !== reference.executionSessionId)) return;
      resetResultQuery(reference);
      setInvalidated(true);
    });
  }, [reference?.executionSessionId, reference?.resultId]);
  return !invalidated;
}
