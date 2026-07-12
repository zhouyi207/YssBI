import type { FC } from 'react';
import { useMemo } from 'react';
import {
  ReportLayout,
  ReportSection,
  formatNum,
  InfoRow,
  CoefficientTable,
} from './shared';
import type { Coefficient, DFADFSummaryResultData } from '@/shared/types/report';

export const DFADFComponent: FC<{ data: DFADFSummaryResultData }> = ({ data }) => {
  const {
    title,
    var_name,
    h0,
    test_statistic,
    critical_value_1pct,
    critical_value_5pct,
    critical_value_10pct,
    p_value,
    use_t_distribution,
    num_obs,
    lags,
    regression,
    regression_table,
  } = data;

  const reject_1 = test_statistic < critical_value_1pct;
  const reject_5 = test_statistic < critical_value_5pct;
  const reject_10 = test_statistic < critical_value_10pct;

  const coefficients: Coefficient[] = useMemo(
    () =>
      regression_table.map((row) => ({
        variable: row.variable,
        coef: row.coef,
        std_err: row.std_err,
        t_value: row.t,
        p_value: row.p_value,
        'confidence_interval_0.025': row.ci_lower,
        'confidence_interval_0.975': row.ci_upper,
        is_significant: row.p_value < 0.05,
      })),
    [regression_table],
  );

  return (
    <ReportLayout
      title={title}
      badges={
        <span className="text-xs text-muted-foreground">
          Variable: {var_name} · n={num_obs} · lags={lags} · {regression}
        </span>
      }
      subtitle={<div className="text-xs text-muted-foreground">{h0}</div>}
    >
      <ReportSection title="Test Statistic & Critical Values" icon="test">
        <div className="mb-2 grid grid-cols-2 gap-px overflow-hidden rounded-lg border border-border bg-border">
          <InfoRow label="Z(t)">{formatNum(test_statistic)}</InfoRow>
          <InfoRow label="p-value for Z(t)">
            <span className={p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
              {formatNum(p_value)}
            </span>
            {!use_t_distribution && (
              <span className="ml-1 text-[10px] text-muted-foreground">(MacKinnon approx.)</span>
            )}
          </InfoRow>
          <InfoRow label="1% Critical Value">
            <span className={reject_1 ? 'text-emerald-400' : 'text-foreground'}>
              {formatNum(critical_value_1pct)}
              {reject_1 && ' *'}
            </span>
          </InfoRow>
          <InfoRow label="5% Critical Value">
            <span className={reject_5 ? 'text-emerald-400' : 'text-foreground'}>
              {formatNum(critical_value_5pct)}
              {reject_5 && ' *'}
            </span>
          </InfoRow>
          <InfoRow label="10% Critical Value">
            <span className={reject_10 ? 'text-emerald-400' : 'text-foreground'}>
              {formatNum(critical_value_10pct)}
              {reject_10 && ' *'}
            </span>
          </InfoRow>
          <div className="col-span-2 flex justify-between bg-card px-4 py-2.5">
            <span className="text-xs text-muted-foreground">
              {use_t_distribution ? 't-distribution' : 'Dickey-Fuller'} critical value
            </span>
            <span className="text-[10px] text-muted-foreground">* Reject H0 at this level</span>
          </div>
        </div>
      </ReportSection>

      <ReportSection title="Regression Table" icon="regress">
        <CoefficientTable coefficients={coefficients} hasCategorical={false} />
      </ReportSection>
    </ReportLayout>
  );
};
