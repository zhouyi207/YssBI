import { useCallback, useEffect, useRef, useState } from "react";
import type {
  LinearRegressionReportData,
  LinearSummaryAddition,
} from "@/shared/types/domain/resultReport";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { addLinearSummaryContents } from "./addLinearSummaryContents";
import { useResultViewPresentation } from "./resultViewPresentation";
import { workbenchLayoutControl } from "@/modules/workbench/public";
import { resultReference } from "@/shared/types/domain/result";

export function useLinearSummaryContents(initial: LinearRegressionReportData) {
  const presentation = useResultViewPresentation();
  const [current, setCurrent] = useState<{
    data: LinearRegressionReportData;
    release?: () => void;
  }>({ data: initial });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<ErrorReference | null>(null);
  const generation = useRef(0);
  const running = useRef(false);
  useEffect(() => {
    setCurrent({ data: initial });
    setBusy(false);
    setError(null);
    running.current = false;
    return () => {
      generation.current++;
    };
  }, [initial]);
  useEffect(() => () => current.release?.(), [current]);
  const add = useCallback(
    async (additions: LinearSummaryAddition) => {
      if (running.current) return;
      running.current = true;
      const request = generation.current;
      setBusy(true);
      setError(null);
      try {
        const next = await addLinearSummaryContents(
          current.data.resultRef,
          additions,
          () => generation.current === request,
        );
        if (generation.current !== request) {
          next.release();
          return;
        }
        if (presentation === "embedded") {
          try {
            const panel = await workbenchLayoutControl.replaceResult(current.data.resultRef, {
              reference: resultReference(next.lease.descriptor),
              leaseId: next.lease.leaseId,
              title: next.lease.descriptor.title,
              presentation: next.lease.descriptor.presentation,
            });
            if (!panel || panel.metadata.role !== "result")
              throw { code: "report_summary_changed", incidentId: null };
            if (panel.metadata.leaseId === next.lease.leaseId) next.transferToPanel();
            else next.release();
          } catch (failure) {
            next.release();
            throw failure;
          }
        } else setCurrent(next);
      } catch (failure) {
        if (generation.current === request)
          setError(toErrorReference(failure, "report_summary_failed"));
      } finally {
        if (generation.current === request) {
          running.current = false;
          setBusy(false);
        }
      }
    },
    [current.data, presentation],
  );
  return { data: current.data, busy, error, add };
}
