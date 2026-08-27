import { useEffect, useMemo, type ReactNode } from 'react';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useResultValue } from '@/features/core/resultSource';
import { reportResultValuePayload } from '@/features/core/resultSource/resultValuePayload';
import { ResultReadError } from '@/features/core/resultSource/components/ResultReadError';
import type { ReportKind, ResultDescriptor } from '@/features/core/resultSource';
import { validateReportPayload } from '@/shared/types/report/reportValidation';
import { logger } from '@/utils/appLogger';
import { resolveReportComponent } from './reportViewResolver';

interface ReportViewProps {
  descriptor: ResultDescriptor;
  report: ReportKind;
  data?: unknown;
}

export function ReportView({ descriptor, report, data }: ReportViewProps) {
  const loaded = useResultValue(data === undefined ? descriptor.resultId : null);
  const resolvedData = data ?? (loaded.value ? reportResultValuePayload(loaded.value) : loaded.value);
  const validation = useMemo(
    () => validateReportPayload(descriptor, report, resolvedData),
    [descriptor, report, resolvedData],
  );

  useEffect(() => {
    if (!validation.ok && (data !== undefined || (!loaded.loading && !loaded.error))) {
      logger.data.error(JSON.stringify(validation.diagnostic), 'ReportValidation');
    }
  }, [data, loaded.error, loaded.loading, validation]);

  if (data === undefined && loaded.error) {
    return <ResultReadError error={loaded.error} />;
  }
  if (data === undefined && loaded.loading) {
    return <p className="text-sm text-muted-foreground">Loading…</p>;
  }

  let content: ReactNode;
  if (!validation.ok) {
    const label = report === 'olsSummary' ? 'OLS report' : 'report';
    content = (
      <Alert variant="destructive" className="m-4 w-auto">
        <AlertDescription className="text-destructive">
          Unable to render {label}: {validation.diagnostic.fieldPath} {validation.diagnostic.reason}.
        </AlertDescription>
      </Alert>
    );
  } else {
    const Component = resolveReportComponent(report);
    content = <Component data={validation.value} />;
  }

  return (
    <ScrollArea className="min-h-0 flex-1" orientation="vertical">
      {content}
    </ScrollArea>
  );
}
