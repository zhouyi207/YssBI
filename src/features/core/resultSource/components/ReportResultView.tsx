import { ScrollArea } from '@/components/ui/scroll-area';
import { ReportView } from '@/views/InfoView/ReportView';
import type { ResultDescriptor } from '../types';
import { reportResultValuePayload } from '../resultValuePayload';
import { useResultValue } from '../useResultValue';
import { ResultReadError } from './ResultReadError';

export interface ReportResultViewProps {
  payload: ResultDescriptor;
  /** When provided, skips IPC fetch (presentation windows preload data). */
  data?: unknown;
}

export function ReportResultView({
  payload,
  data: preloadedData,
}: ReportResultViewProps) {
  if (payload.presentation.kind !== 'report') {
    return null;
  }

  const report = payload.presentation.report;
  const { value, loading, error } = useResultValue(
    preloadedData === undefined ? payload.resultId : null,
  );

  if (error) {
    return <ResultReadError error={error} />;
  }
  if (preloadedData === undefined && loading) {
    return <p className="text-sm text-muted-foreground">Loading…</p>;
  }

  const data = preloadedData ?? (value ? reportResultValuePayload(value) : value);
  const content = <ReportView descriptor={payload} report={report} data={data} />;

  return (
    <ScrollArea className="min-h-0 flex-1" orientation="vertical">
      {content}
    </ScrollArea>
  );
}
