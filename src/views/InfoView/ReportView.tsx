import type { ReportKind } from '@/features/core/resultSource';
import { resolveReportComponent } from './reportViewResolver';

interface ReportViewProps {
  report: ReportKind;
  data: unknown;
}

export function ReportView({ report, data }: ReportViewProps) {
  const Component = resolveReportComponent(report);
  return <Component data={data} />;
}
