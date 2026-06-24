import React, { useMemo } from 'react';
import {
  SectionHeader,
  formatNum,
  InfoRow,
  CoefficientTable,
} from './shared';
import type { Coefficient, DFADFSummaryResultData } from './shared/types';

const TEST_ICON = (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
  </svg>
);

const REGRESS_ICON = (
  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
  </svg>
);

export const DFADFComponent: React.FC<{ data: DFADFSummaryResultData }> = ({ data }) => {
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
    [regression_table]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-foreground mb-2">{title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-xs text-muted-foreground">
            Variable: {var_name} &middot; n={num_obs} &middot; lags={lags} &middot; {regression}
          </span>
        </div>
        <div className="text-xs text-muted-foreground mt-1">{h0}</div>
      </div>

      {/* Test Statistic & Critical Values */}
      <SectionHeader
        title="Test Statistic & Critical Values"
        icon={TEST_ICON}
      />
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border mb-2">
        <InfoRow label="Z(t)">{formatNum(test_statistic)}</InfoRow>
        <InfoRow label="p-value for Z(t)">
          <span className={p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}>
            {formatNum(p_value)}
          </span>
          {!use_t_distribution && (
            <span className="text-muted-foreground ml-1 text-[10px]">(MacKinnon approx.)</span>
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
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-muted-foreground text-xs">
            {use_t_distribution ? 't-distribution' : 'Dickey-Fuller'} critical value
          </span>
          <span className="text-muted-foreground text-[10px]">* Reject H0 at this level</span>
        </div>
      </div>

      {/* Regression Table */}
      <SectionHeader
        title="Regression Table"
        icon={REGRESS_ICON}
      />
      <CoefficientTable coefficients={coefficients} hasCategorical={false} />
    </div>
  );
};
