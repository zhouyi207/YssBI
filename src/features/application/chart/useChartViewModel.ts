import { useCallback, useEffect, useState } from "react";

import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import { useChartRead } from "@/features/core/chart/read";
import { chartUi } from "@/features/core/chart/ui";
import type { ChartDocument } from "@/shared/types/domain/chart";
import type {
  ChartDocumentCoordinator,
  ChartLoadOutcome,
  ChartSaveOutcome,
} from "./chartDocumentCoordinator";

export interface ChartViewModel {
  readonly document: DeepReadonly<ChartDocument> | null;
  readonly dirty: boolean;
  readonly loading: boolean;
  readonly saving: boolean;
  readonly issue: ErrorReference | null;
  update(patch: Partial<ChartDocument>): void;
  save(): Promise<ChartSaveOutcome>;
  reload(): Promise<ChartLoadOutcome>;
}

function issueFor(
  operation: "load" | "save",
  status: "failed" | "rejected" | "unknown",
): ErrorReference {
  return {
    code: `chart_${operation}_${status}`,
    incidentId: null,
  };
}

export function useChartViewModel(
  chartPath: string,
  coordinator: ChartDocumentCoordinator,
): ChartViewModel {
  const projection = useChartRead((state) => ({
    document: state.draftsByPath[chartPath] ?? state.documents[chartPath] ?? null,
    dirty: state.dirtyByPath[chartPath] === true,
  }));
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [issue, setIssue] = useState<ErrorReference | null>(null);

  useEffect(() => {
    setLoading(false);
    setSaving(false);
    setIssue(null);
  }, [chartPath]);

  const update = useCallback(
    (patch: Partial<ChartDocument>): void => {
      chartUi.updateDraft(chartPath, patch);
      setIssue(null);
    },
    [chartPath],
  );

  const save = useCallback(async (): Promise<ChartSaveOutcome> => {
    setSaving(true);
    setIssue(null);
    try {
      const outcome = await coordinator.save(chartPath);
      if (
        outcome.status === "failed" ||
        outcome.status === "rejected" ||
        outcome.status === "unknown"
      ) {
        setIssue(issueFor("save", outcome.status));
      }
      return outcome;
    } catch {
      const outcome = { status: "failed" as const };
      setIssue(issueFor("save", outcome.status));
      return outcome;
    } finally {
      setSaving(false);
    }
  }, [coordinator, chartPath]);

  const reload = useCallback(async (): Promise<ChartLoadOutcome> => {
    setLoading(true);
    setIssue(null);
    try {
      const outcome = await coordinator.load(chartPath);
      if (outcome.status === "failed") setIssue(issueFor("load", outcome.status));
      return outcome;
    } catch {
      const outcome = { status: "failed" as const };
      setIssue(issueFor("load", outcome.status));
      return outcome;
    } finally {
      setLoading(false);
    }
  }, [coordinator, chartPath]);

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
