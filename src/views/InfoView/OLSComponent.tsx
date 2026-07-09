import type { FC } from 'react';
import { useRegressionReport } from '@/features/application/stats/useRegressionReport';
import {
  ReportLayout,
  ReportLazyBoundary,
  ReportSection,
  RSquaredBadge,
  LazyFormulaBlock,
  RegressionModelCoreSections,
  OlsStyleDiagnosticsSection,
} from './shared';
import type { OLSResultData } from '@/shared/types/report';

export type { Coefficient, OLSResultData } from '@/shared/types/report';

export const OLSComponent: FC<{ data: OLSResultData }> = ({ data }) => {
  const { info, coefficients, diag, hasCategorical, leverageKdeData } = useRegressionReport(data);

  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <RSquaredBadge value={info.r_squared} />
          <span className="text-xs text-muted-foreground">
            {info.method} &middot; n={info.num_observation}
          </span>
        </>
      }
    >
      <ReportSection title="Equation" icon="equation">
        <ReportLazyBoundary variant="formula">
          <LazyFormulaBlock endogName={data.endog_name || 'y'} coefficients={coefficients} />
        </ReportLazyBoundary>
      </ReportSection>

      <RegressionModelCoreSections data={data} hasCategorical={hasCategorical} showOmittedVariables />

      <OlsStyleDiagnosticsSection diag={diag} leverageKdeData={leverageKdeData} />
    </ReportLayout>
  );
};
