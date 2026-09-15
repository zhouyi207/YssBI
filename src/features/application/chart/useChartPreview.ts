import { useEffect, useState } from "react";

import { toErrorReference } from "@/features/application/errorReference";
import { getDatabaseSnapshot, useDatabaseRead } from "@/features/core/database/read";
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  fetchChartPreview,
  type ChartPreviewProjectIdentity,
} from "@/services/chart/chartPreviewDataService";
import { getCachedChartPreview, getChartPreview } from "@/services/chart/chartPreviewCache";
import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain";

export interface ChartPreviewState {
  readonly preview: ChartPreviewPayload;
  readonly loading: boolean;
}

/** Own the asynchronous chart preview lifecycle for Views. */
export function useChartPreview(
  chartPath: string,
  document: ChartDocument | null,
): ChartPreviewState {
  const [preview, setPreview] = useState<ChartPreviewPayload>({ kind: "empty" });
  const [loading, setLoading] = useState(false);

  const databaseRevision = useDatabaseRead((snapshot) =>
    document?.databaseId ? (snapshot.revisions[document.databaseId] ?? null) : null,
  );

  useEffect(() => {
    if (!document) {
      setPreview({ kind: "empty" });
      setLoading(false);
      return;
    }

    const identity = captureProjectIdentity();
    let active = true;
    const sourceIsCurrent = () =>
      isCurrentProjectIdentity(identity) &&
      (document.databaseId
        ? (getDatabaseSnapshot().revisions[document.databaseId] ?? null)
        : null) === databaseRevision;
    const isCurrent = () => active && sourceIsCurrent();
    const cached = getCachedChartPreview(
      identity.projectInstanceId,
      chartPath,
      document,
      databaseRevision,
    );
    if (cached) {
      if (!isCurrent()) return;
      setPreview(cached);
      setLoading(false);
      return;
    }

    const previewIdentity: ChartPreviewProjectIdentity = {
      projectInstanceId: identity.projectInstanceId,
      isCurrent: sourceIsCurrent,
      assertCurrent: () => {
        assertCurrentProjectIdentity(identity);
        if (!sourceIsCurrent()) throw new Error("chart preview source changed");
      },
    };
    const timer = window.setTimeout(() => {
      void (async () => {
        if (!isCurrent()) return;
        setLoading(true);
        try {
          const result = await getChartPreview(
            identity.projectInstanceId,
            chartPath,
            document,
            databaseRevision,
            () => fetchChartPreview(document, previewIdentity),
          );
          if (!isCurrent()) return;
          setPreview(result);
        } catch (error) {
          if (!isCurrent()) return;
          setPreview({
            kind: "error",
            ...toErrorReference(error, "chart_preview_read_failed"),
          });
        } finally {
          if (isCurrent()) setLoading(false);
        }
      })();
    }, 300);
    setPreview({ kind: "empty" });
    setLoading(true);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [document, chartPath, databaseRevision]);

  return { preview, loading };
}
