import type { FC } from 'react';
import { useRegressionReport } from '@/features/application/stats/useRegressionReport';
import {
  ReportLayout,
  RSquaredBadge,
  RegressionModelCoreSections,
  OlsStyleDiagnosticsSection,
  IvReportSections,
} from './shared';
import type { OLSResultData } from './shared/types';

export type { OLSResultData };

export const TwoSLSComponent: FC<{ data: OLSResultData }> = ({ data }) => {
  const { info, coefficients, diag, hasCategorical, leverageKdeData } = useRegressionReport(data);

  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <RSquaredBadge value={info.r_squared} />
          <span className="inline-flex items-center rounded border border-amber-500/30 bg-amber-500/20 px-2 py-0.5 text-[10px] font-medium text-amber-400">
            IV:2SLS
          </span>
          <span className="text-xs text-muted-foreground">
            {info.method} &middot; n={info.num_observation}
          </span>
        </>
      }
    >
      <IvReportSections
        variant="2sls"
        endogName={data.endog_name || 'y'}
        coefficients={coefficients}
        diag={diag}
      />

      <RegressionModelCoreSections
        data={data}
        hasCategorical={hasCategorical}
        coefficientsProps={{ useZStat: true }}
      />

      <OlsStyleDiagnosticsSection diag={diag} leverageKdeData={leverageKdeData} />
    </ReportLayout>
  );
};
