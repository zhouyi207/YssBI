import { useEffect, useState } from "react";
import { subscribeResultInvalidation } from "@/services/result/resultInvalidationChannel";
import { resetResultQuery, resultQueryCoordinator } from "@/features/application/results/runtime";

/** Detached windows release their payload when the current output is invalidated. */
export function usePresentationWindowLifecycle(resultId: string | null | undefined): boolean {
  const [invalidated, setInvalidated] = useState(false);
  useEffect(() => {
    setInvalidated(false);
    return subscribeResultInvalidation((ids) => {
      if (!resultId || (ids !== null && !ids.includes(resultId))) return;
      resultQueryCoordinator.resetProject();
      resetResultQuery(resultId);
      setInvalidated(true);
    });
  }, [resultId]);
  return !invalidated;
}
