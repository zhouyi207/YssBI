import { OverlayScrollbar } from '@/shared/ui/OverlayScrollbar';
import { ReportView } from '@/views/InfoView/ReportView';
import type { SourceDescriptor } from '../types';
import { useSourceValue } from '../useSourceValue';
import type { SourceViewLayout } from './SourceViewShell';

export interface ReportSourceViewProps {
  payload: SourceDescriptor;
  layout?: SourceViewLayout;
  /** When provided, skips IPC fetch (presentation windows preload data). */
  data?: unknown;
}

export function ReportSourceView({
  payload,
  layout = 'embedded',
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

  const data = preloadedData ?? value?.value ?? value?.structured ?? value;
  const content = <ReportView report={report} data={data} />;

  if (layout === 'window') {
    return (
      <OverlayScrollbar className="min-h-0 flex-1" direction="vertical">
        {content}
      </OverlayScrollbar>
    );
  }

  return content;
}
