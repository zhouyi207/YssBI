import type { FC } from 'react';
import { useRegressionReport } from '@/features/application/stats/useRegressionReport';
import {
  ReportLayout,
  RSquaredBadge,
  formatNum,
  RegressionModelCoreSections,
  OlsStyleDiagnosticsSection,
  IvReportSections,
} from './shared';
import type { OLSResultData } from '@/shared/types/report';

export const LIMLComponent: FC<{ data: OLSResultData }> = ({ data }) => {
  const { info, coefficients, diag, hasCategorical, leverageKdeData } = useRegressionReport(data);

  return (
    <ReportLayout
      title={data.title}
      badges={
        <>
          <RSquaredBadge value={info.r_squared} />
          <span className="inline-flex items-center rounded border border-violet-500/30 bg-violet-500/20 px-2 py-0.5 text-[10px] font-medium text-violet-400">
            IV:LIML
          </span>
          {diag.ivliml_kappa != null ? (
            <span className="inline-flex items-center rounded border border-border bg-muted px-2 py-0.5 text-[10px] font-medium text-foreground">
              κ = {formatNum(diag.ivliml_kappa, 6)}
            </span>
          ) : null}
          <span className="text-xs text-muted-foreground">
            {info.method} &middot; n={info.num_observation}
          </span>
        </>
      }
    >
      <IvReportSections
        variant="liml"
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
