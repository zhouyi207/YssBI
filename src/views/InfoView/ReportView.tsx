import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import type { ReportKind } from '@/features/core/resultSource';
import { parseReportPayload } from '@/shared/types/report/parseReportPayload';
import { resolveReportComponent } from './reportViewResolver';

interface ReportViewProps {
  report: ReportKind;
  data: unknown;
}

export function ReportView({ report, data }: ReportViewProps) {
  const { t } = useTranslation();
  const parsed = useMemo(() => parseReportPayload(report, data), [report, data]);

  if (!parsed) {
    return (
      <p className="px-4 py-6 text-sm text-destructive">
        {t('info.invalidReportPayload', { defaultValue: '报告数据格式无效，无法渲染。' })}
      </p>
    );
  }

  const Component = resolveReportComponent(report);
  return <Component data={parsed} />;
}
