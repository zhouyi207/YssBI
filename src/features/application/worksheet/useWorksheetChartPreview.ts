import { useEffect, useMemo, useState } from 'react';

import { toErrorReference } from '@/features/application/errorReference';
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import {
  fetchWorksheetPreview,
  type WorksheetPreviewProjectIdentity,
} from '@/services/worksheet/worksheetDataService';
import {
  getCachedWorksheetPreview,
  getWorksheetPreview,
} from '@/services/worksheet/worksheetPreviewCache';
import type { WorksheetDocument, WorksheetPreviewPayload } from '@/shared/types/domain';

export interface WorksheetChartPreviewState {
  readonly preview: WorksheetPreviewPayload;
  readonly loading: boolean;
}

/** Own the asynchronous worksheet preview lifecycle for Views. */
export function useWorksheetChartPreview(
  worksheetPath: string,
  document: WorksheetDocument | null,
): WorksheetChartPreviewState {
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

    const previewIdentity: WorksheetPreviewProjectIdentity = {
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
  }, [specKey, document, worksheetPath]);

  return { preview, loading };
}
