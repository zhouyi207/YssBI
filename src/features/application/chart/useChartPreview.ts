import { useEffect, useMemo, useState } from "react";

import { toErrorReference } from "@/features/application/errorReference";
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

  const specKey = useMemo(() => {
    if (!document) return "";
    return JSON.stringify({
      chartPath,
      databaseId: document.databaseId,
      chartType: document.chartType,
      encodings: document.encodings,
    });
  }, [document, chartPath]);

  useEffect(() => {
    if (!specKey || !document) {
      setPreview({ kind: "empty" });
      return;
    }

    const identity = captureProjectIdentity();
    const cached = getCachedChartPreview(identity.projectInstanceId, chartPath, document);
    if (cached) {
      if (!isCurrentProjectIdentity(identity)) return;
      setPreview(cached);
      setLoading(false);
      return;
    }

    const previewIdentity: ChartPreviewProjectIdentity = {
      projectInstanceId: identity.projectInstanceId,
      isCurrent: () => isCurrentProjectIdentity(identity),
      assertCurrent: () => assertCurrentProjectIdentity(identity),
    };
    const timer = window.setTimeout(() => {
      void (async () => {
        if (!isCurrentProjectIdentity(identity)) return;
        setLoading(true);
        try {
          const result = await getChartPreview(
            identity.projectInstanceId,
            chartPath,
            document,
            () => fetchChartPreview(document, previewIdentity),
          );
          if (!isCurrentProjectIdentity(identity)) return;
          setPreview(result);
        } catch (error) {
          if (!isCurrentProjectIdentity(identity)) return;
          setPreview({
            kind: "error",
            ...toErrorReference(error, "chart_preview_read_failed"),
          });
        } finally {
          if (isCurrentProjectIdentity(identity)) setLoading(false);
        }
      })();
    }, 300);
    return () => window.clearTimeout(timer);
  }, [specKey, document, chartPath]);

  return { preview, loading };
}
