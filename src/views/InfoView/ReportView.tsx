import { useEffect, useMemo } from 'react';
import type { ReportKind, ResultDescriptor } from '@/features/core/resultSource';
import { validateReportPayload } from '@/shared/types/report/reportValidation';
import { logger } from '@/utils/appLogger';
import { resolveReportComponent } from './reportViewResolver';

interface ReportViewProps {
  descriptor: ResultDescriptor;
  report: ReportKind;
  data: unknown;
}

export function ReportView({ descriptor, report, data }: ReportViewProps) {
  const validation = useMemo(
    () => validateReportPayload(descriptor, report, data),
    [data, descriptor, report],
  );

  useEffect(() => {
    if (!validation.ok) {
      logger.notify.error(JSON.stringify(validation.diagnostic), 'ReportValidation');
    }
  }, [validation]);

  if (!validation.ok) {
    const label = report === 'olsSummary' ? 'OLS report' : 'report';
    return (
      <p className="px-4 py-6 text-sm text-destructive">
        Unable to render {label}: {validation.diagnostic.fieldPath} {validation.diagnostic.reason}.
      </p>
    );
  }

  const Component = resolveReportComponent(report);
  return <Component data={validation.value} />;
}
