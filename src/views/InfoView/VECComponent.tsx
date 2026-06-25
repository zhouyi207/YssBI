import React, { useMemo } from 'react';
import {
  SectionHeader,
  formatNum,
  CoefficientsBlock,
  CoefficientTable,
  VARStableChart,
  VarModelTable,
  VarModelRow,
  VarModelCell,
  VarEigenvalueTable,
} from './shared';
import type { Coefficient, VECSummaryResultData } from './shared/types';

function vecCoeffsToOLSFormat(coefficients: VECSummaryResultData['coefficients']): Coefficient[] {
  const eqOrder = [...new Set(coefficients.map((x) => x.eq_name))];
  return coefficients.map((c, idx) => ({
    variable: c.variable,
    category: c.eq_name,
    coef: c.coef,
    std_err: c.std_err,
    t_value: c.z_value,
    p_value: c.p_value,
    'confidence_interval_0.025': c.ci_lower,
    'confidence_interval_0.975': c.ci_upper,
    is_significant: c.p_value < 0.05,
    _sortKey: c.variable === 'const' ? 0 : 1,
    _eqOrder: eqOrder.indexOf(c.eq_name),
    _idx: idx,
  }))
    .sort((a, b) => {
      if (a._eqOrder !== b._eqOrder) return a._eqOrder - b._eqOrder;
      if (a._sortKey !== b._sortKey) return a._sortKey - b._sortKey;
      return a._idx - b._idx;
    })
    .map(({ _sortKey, _eqOrder, _idx, ...rest }) => rest as Coefficient);
}

type BetaCoeffWithSort = Coefficient & { _sortKey: number; _eqOrder: number; _idx: number };

function vecBetaToCoeffs(
  beta: number[][],
  betaVarNames: string[],
  beta_std_err: (number | null)[][],
  beta_z_value: (number | null)[][],
  beta_p_value: (number | null)[][],
  beta_ci_lower: (number | null)[][],
  beta_ci_upper: (number | null)[][]
): Coefficient[] {
  const result: BetaCoeffWithSort[] = [];
  for (let ceIdx = 0; ceIdx < beta.length; ceIdx++) {
    for (let varIdx = 0; varIdx < beta[ceIdx].length; varIdx++) {
      const se = beta_std_err[ceIdx]?.[varIdx];
      const z = beta_z_value[ceIdx]?.[varIdx];
      const p = beta_p_value[ceIdx]?.[varIdx];
      const ciLo = beta_ci_lower[ceIdx]?.[varIdx];
      const ciHi = beta_ci_upper[ceIdx]?.[varIdx];
      const hasStats = se != null && z != null && p != null && ciLo != null && ciHi != null;
      result.push({
        variable: betaVarNames[varIdx] ?? `var${varIdx}`,
        category: `_ce${ceIdx + 1}`,
        coef: beta[ceIdx][varIdx],
        std_err: hasStats ? se! : undefined,
        t_value: hasStats ? z! : undefined,
        p_value: hasStats ? p! : undefined,
        'confidence_interval_0.025': hasStats ? ciLo! : undefined,
        'confidence_interval_0.975': hasStats ? ciHi! : undefined,
        is_significant: hasStats ? (p! < 0.05) : false,
        _sortKey: betaVarNames[varIdx] === 'const' ? 0 : 1,
        _eqOrder: ceIdx,
        _idx: result.length,
      });
    }
  }
  return result
    .sort((a, b) => {
      if (a._eqOrder !== b._eqOrder) return a._eqOrder - b._eqOrder;
      if (a._sortKey !== b._sortKey) return a._sortKey - b._sortKey;
      return a._idx - b._idx;
    })
    .map(({ _sortKey, _eqOrder, _idx, ...rest }) => rest as Coefficient);
}

export const VECComponent: React.FC<{ data: VECSummaryResultData }> = ({ data }) => {
  const {
    var_names,
    num_observation,
    log_likelihood,
    aic,
    hqic,
    sbic,
    det_sigma_ml,
    rank,
    lags,
    trend_spec,
    equations,
    coefficients,
    beta,
    beta_var_names,
    cointegrating_equations = [],
    beta_std_err = [],
    beta_z_value = [],
    beta_p_value = [],
    beta_ci_lower = [],
    beta_ci_upper = [],
    veclmar = [],
    vecstable = [],
  } = data;

  const vecstableSorted = useMemo(
    () => (vecstable.length > 0 ? [...vecstable].sort((a, b) => b.modulus - a.modulus) : []),
    [vecstable]
  );

  const coeffsForTable = vecCoeffsToOLSFormat(coefficients);
  const betaVarNames = beta_var_names?.length
    ? beta_var_names
    : (beta[0]?.length ?? 0) > var_names.length
      ? [...var_names, 'const']
      : var_names;

  const betaCoeffs = useMemo(
    () =>
      beta.length > 0 && betaVarNames.length > 0
        ? vecBetaToCoeffs(
            beta,
            betaVarNames,
            beta_std_err,
            beta_z_value,
            beta_p_value,
            beta_ci_lower,
            beta_ci_upper
          )
        : [],
    [beta, betaVarNames, beta_std_err, beta_z_value, beta_p_value, beta_ci_lower, beta_ci_upper]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-foreground mb-2">{data.title}</h1>
        <div className="flex flex-wrap gap-3 text-xs text-muted-foreground">
          <span>Variables: {var_names.join(', ')}</span>
          <span>n = {num_observation}</span>
          <span>rank = {rank}</span>
          <span>lags = {lags}</span>
          <span>trend: {trend_spec}</span>
        </div>
      </div>

      {/* Model Summary */}
      <SectionHeader
        title="Model Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        }
      />
      <div className="grid grid-cols-2 gap-px bg-border rounded-lg overflow-hidden border border-border mb-6">
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">Log likelihood</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(log_likelihood)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">AIC</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(aic)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">HQIC</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(hqic)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between">
          <span className="text-muted-foreground text-xs">SBIC</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(sbic)}</span>
        </div>
        <div className="bg-card px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-muted-foreground text-xs">Det(Sigma_ml)</span>
          <span className="text-foreground text-xs font-mono font-medium">{formatNum(det_sigma_ml)}</span>
        </div>
      </div>

      {/* Equation Summary */}
      {equations.length > 0 && (
        <>
          <SectionHeader
            title="Equation Summary"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
              </svg>
            }
          />
          <VarModelTable className="mb-6" columns={['Equation', 'Parms', 'RMSE', 'R-sq', 'chi2', 'P>chi2']}>
            {equations.map((eq, i) => (
              <VarModelRow key={i}>
                <VarModelCell>{eq.eq_name}</VarModelCell>
                <VarModelCell>{eq.parms}</VarModelCell>
                <VarModelCell>{formatNum(eq.rmse)}</VarModelCell>
                <VarModelCell>{formatNum(eq.r_sq)}</VarModelCell>
                <VarModelCell>{formatNum(eq.chi2)}</VarModelCell>
                <VarModelCell>{formatNum(eq.p_chi2)}</VarModelCell>
              </VarModelRow>
            ))}
          </VarModelTable>
        </>
      )}

      {/* Coefficients */}
      {coeffsForTable.length > 0 && (
        <CoefficientsBlock
          coefficients={coeffsForTable}
          hasCategorical={true}
          useZStat={true}
          categoryLabel="Equation"
        />
      )}

      {/* Cointegrating equations (Stata style) */}
      {cointegrating_equations.length > 0 && (
        <>
          <SectionHeader
            title="Cointegrating equations"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2" />
              </svg>
            }
          />
          <VarModelTable className="mb-6" columns={['Equation', 'Parms', 'chi2', 'P>chi2']}>
            {cointegrating_equations.map((ce, i) => (
              <VarModelRow key={i}>
                <VarModelCell>{ce.eq_name}</VarModelCell>
                <VarModelCell>{ce.parms}</VarModelCell>
                <VarModelCell>{formatNum(ce.chi2)}</VarModelCell>
                <VarModelCell>{formatNum(ce.p_chi2)}</VarModelCell>
              </VarModelRow>
            ))}
          </VarModelTable>
          <div className="text-xs text-muted-foreground mt-2 mb-4">Identification: beta is exactly identified</div>
        </>
      )}

      {/* Cointegration vectors (beta) - 使用 CoefficientTable 展示 */}
      {betaCoeffs.length > 0 && (
        <>
          <div className="text-xs text-muted-foreground mb-2">Johansen normalization restriction imposed</div>
          <SectionHeader
            title="beta"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <div className="mb-6">
            <CoefficientTable
              coefficients={betaCoeffs}
              hasCategorical={true}
              useZStat={true}
              categoryLabel="Equation"
            />
          </div>
        </>
      )}

      {/* Eigenvalue stability condition (vecstable) */}
      {vecstableSorted.length > 0 && (
        <>
          <SectionHeader
            title="Eigenvalue stability condition"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002-2V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <div className="grid grid-cols-[auto_1fr] gap-4 mb-6 items-stretch min-h-[360px]">
            <div className="flex flex-col h-full rounded-lg border border-border bg-muted overflow-hidden">
              <div className="flex-1 min-h-0 flex flex-col">
                <VarEigenvalueTable rows={vecstableSorted} />
                <div className="flex-1 min-h-0 bg-muted" />
              </div>
              <div className="px-4 py-2 text-[11px] text-muted-foreground border-t border-border shrink-0">
                {vecstableSorted.some((r) => r.modulus >= 1)
                  ? 'At least one eigenvalue is at least 1.0. VEC does not satisfy stability condition.'
                  : 'All the eigenvalues lie inside the unit circle.'}
              </div>
            </div>
            <div className="min-w-[240px] min-h-0 flex">
              <VARStableChart data={vecstableSorted} />
            </div>
          </div>
        </>
      )}

      {/* Lagrange-multiplier test (veclmar) */}
      {veclmar.length > 0 && (
        <>
          <SectionHeader
            title="Lagrange-multiplier test (veclmar)"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <VarModelTable
            className="mb-6"
            columns={['lag', 'chi2', 'df', 'Prob > chi2']}
            footer={
              <div className="border-t border-border px-4 py-2 text-[11px] text-muted-foreground">
                H0: no autocorrelation at lag order
              </div>
            }
          >
            {veclmar.map((row, i) => (
              <VarModelRow key={i}>
                <VarModelCell>{row.lag}</VarModelCell>
                <VarModelCell>{formatNum(row.chi2)}</VarModelCell>
                <VarModelCell>{row.df}</VarModelCell>
                <VarModelCell>{formatNum(row.p_value)}</VarModelCell>
              </VarModelRow>
            ))}
          </VarModelTable>
        </>
      )}

      {/* Placeholder when no estimation results */}
      {equations.length === 0 && coefficients.length === 0 && beta.length === 0 && (
        <div className="rounded-lg border border-amber-800/50 bg-amber-900/10 px-4 py-3 text-sm text-amber-200/80 mb-6">
          VEC 协整估计尚未实现，当前为占位结果。待实现 Johansen  procedure 后显示完整输出。
        </div>
      )}
    </div>
  );
};
