import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { toErrorReference, fetchWorksheetPreview, getCachedWorksheetPreview, getWorksheetPreview } from '@/features/application/viewCapabilities';
import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';
import Scatter from '@/views/PlotView/Scatter';
import Line from '@/views/PlotView/Line';
import Histogram from '@/views/PlotView/Histogram';
import { WorksheetEmptyState } from './WorksheetEmptyState';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/application/viewCapabilities';

interface WorksheetChartPreviewProps {
  worksheetPath: string;
  document: WorksheetDocument | null;
}

type WorksheetPreviewErrorPayload = Extract<WorksheetPreviewPayload, { kind: 'error' }>;

function WorksheetPreviewError({ error }: { error: WorksheetPreviewErrorPayload }) {
  const { t } = useTranslation();
  const summary = error.column === undefined
    ? t('worksheet.previewLoadFailed')
    : t('worksheet.previewColumnNotFound', { column: error.column });

  return (
    <div className="flex h-full items-center justify-center p-4">
      <Alert variant="destructive" className="max-w-md">
        <AlertTitle>{summary}</AlertTitle>
        <AlertDescription>
          <p>
            {t('common.errorCode')}: <code>{error.code}</code>
          </p>
          {error.incidentId ? (
            <p>
              {t('common.incidentId')}: <code>{error.incidentId}</code>
            </p>
          ) : null}
        </AlertDescription>
      </Alert>
    </div>
  );
}

export function WorksheetChartPreview({ worksheetPath, document }: WorksheetChartPreviewProps) {
  const [preview, setPreview] = useState<WorksheetPreviewPayload>({ kind: 'empty' });
  const [loading, setLoading] = useState(false);

  const specKey = useMemo(() => {
    if (!document) return '';
    return JSON.stringify({
      worksheetPath,
      databaseId: document.databaseId,
      chartType: document.chartType,
      encodings: document.encodings,
    });
  }, [document, worksheetPath]);

  useEffect(() => {
    if (!specKey || !document) {
      setPreview({ kind: 'empty' });
      return;
    }

    const identity = captureProjectIdentity();
    const cached = getCachedWorksheetPreview(identity.projectInstanceId, worksheetPath, document);
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
            worksheetPath,
            document,
            () => fetchWorksheetPreview(document, previewIdentity),
          );
          if (!isCurrentProjectIdentity(identity)) return;
          setPreview(result);
        } catch (error) {
          if (!isCurrentProjectIdentity(identity)) return;
          setPreview({
            kind: 'error',
            ...toErrorReference(error, 'worksheet_preview_read_failed'),
          });
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
      {preview.kind === 'error' && <WorksheetPreviewError error={preview} />}
      {preview.kind === 'empty' && !loading && (
        <div className="absolute inset-0 flex min-h-0">
          <WorksheetEmptyState />
        </div>
      )}
      {(preview.kind === 'histogram' || preview.kind === 'scatter' || preview.kind === 'line') && (
        <div data-worksheet-chart-region className="h-full w-full select-none">
          {preview.kind === 'histogram' ? (
            <Histogram data={preview.bins} xLabel={preview.xLabel} yLabel={preview.yLabel} embedded />
          ) : preview.kind === 'line' ? (
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
          )}
        </div>
      )}
    </div>
  );
}
