import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { ReportView } from '@/views/InfoView/ReportView';
import type { SourceDescriptor } from '../types';
import { reportSourceValuePayload } from '../sourceValuePayload';
import { useSourceValue } from '../useSourceValue';

export interface ReportSourceViewProps {
  payload: SourceDescriptor;
  /** When provided, skips IPC fetch (presentation windows preload data). */
  data?: unknown;
}

export function ReportSourceView({
  payload,
  data: preloadedData,
}: ReportSourceViewProps) {
  if (payload.presentation.kind !== 'report') {
    return null;
  }

  const report = payload.presentation.report;
  const { value, loading, error } = useSourceValue(
    preloadedData === undefined ? payload.sourceId : null,
  );

  if (error) {
    return <p className="text-sm text-destructive">{error}</p>;
  }
  if (preloadedData === undefined && loading) {
    return <p className="text-sm text-muted-foreground">Loading…</p>;
  }

  const data = preloadedData ?? (value ? reportSourceValuePayload(value) : value);
  const content = <ReportView report={report} data={data} />;

  return (
    <OverlayScrollbar className="min-h-0 flex-1" direction="vertical">
      {content}
    </OverlayScrollbar>
  );
}
