import { useEffect, useMemo, useState } from 'react';
import { fetchWorksheetPreview } from '@/services/worksheet/worksheetDataService';
import { getCachedWorksheetPreview, getWorksheetPreview } from '@/services/worksheet/worksheetPreviewCache';
import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';
import Scatter from '@/views/PlotView/Scatter';
import Line from '@/views/PlotView/Line';
import Histogram from '@/views/PlotView/Histogram';
import { WorksheetEmptyState } from './WorksheetEmptyState';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';

interface WorksheetChartPreviewProps {
  document: WorksheetDocument | null;
}

export function WorksheetChartPreview({ document }: WorksheetChartPreviewProps) {
  const [preview, setPreview] = useState<WorksheetPreviewPayload>({ kind: 'empty' });
  const [loading, setLoading] = useState(false);

  const specKey = useMemo(() => {
    if (!document) return '';
    return JSON.stringify({
      id: document.id,
      databaseId: document.databaseId,
      chartType: document.chartType,
      encodings: document.encodings,
    });
  }, [document]);

  useEffect(() => {
    if (!specKey || !document) {
      setPreview({ kind: 'empty' });
      return;
    }

    const identity = captureProjectIdentity();
    const cached = getCachedWorksheetPreview(identity.projectInstanceId, document);
    if (cached) {
      if (!isCurrentProjectIdentity(identity)) return;
      setPreview(cached);
      setLoading(false);
      return;
    }

    const previewIdentity = {
      projectInstanceId: identity.projectInstanceId,
      isCurrent: () => isCurrentProjectIdentity(identity),
      assertCurrent: () => assertCurrentProjectIdentity(identity),
    };
    const timer = window.setTimeout(() => {
      void (async () => {
        if (!isCurrentProjectIdentity(identity)) return;
        setLoading(true);
        try {
          const result = await getWorksheetPreview(
            identity.projectInstanceId,
            document,
            () => fetchWorksheetPreview(document, previewIdentity),
          );
          if (!isCurrentProjectIdentity(identity)) return;
          setPreview(result);
        } catch {
          if (!isCurrentProjectIdentity(identity)) return;
        } finally {
          if (isCurrentProjectIdentity(identity)) setLoading(false);
        }
      })();
    }, 300);
    return () => window.clearTimeout(timer);
  }, [specKey, document]);

  if (!document) {
    return <WorksheetEmptyState messageKey="worksheet.noActiveWorksheet" />;
  }

  return (
    <div className="relative h-full w-full min-h-0 overflow-hidden bg-[var(--workbench-bg)]">
      {loading && preview.kind !== 'empty' && (
        <div className="pointer-events-none absolute inset-0 z-10 bg-[var(--workbench-bg)]/40" />
      )}
      {preview.kind === 'error' && (
        <div className="flex h-full items-center justify-center p-4 text-sm text-red-400">{preview.message}</div>
      )}
      {preview.kind === 'empty' && !loading && (
        <div className="absolute inset-0 flex min-h-0">
          <WorksheetEmptyState />
        </div>
      )}
      {preview.kind === 'histogram' && (
        <Histogram data={preview.bins} xLabel={preview.xLabel} yLabel={preview.yLabel} embedded />
      )}
      {(preview.kind === 'scatter' || preview.kind === 'line') &&
        (preview.kind === 'line' ? (
          <Line
            data={preview.pair.data}
            xLabel={preview.pair.xLabel}
            yLabel={preview.pair.yLabel}
            xFormat={preview.pair.xFormat}
            yFormat={preview.pair.yFormat}
            embedded
          />
        ) : (
          <Scatter
            data={preview.pair.data}
            xLabel={preview.pair.xLabel}
            yLabel={preview.pair.yLabel}
            xFormat={preview.pair.xFormat}
            yFormat={preview.pair.yFormat}
            embedded
          />
        ))}
    </div>
  );
}
