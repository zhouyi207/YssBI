import React, { useMemo } from 'react';
import {
  ReportLayout,
  ReportSection,
  formatNum,
  CoefficientsBlock,
  CoefficientTable,
  VarModelTable,
  VarModelRow,
  VarModelCell,
} from './shared';
import { VarEigenvalueStabilityPanel } from './shared/VarEigenvalueStabilityPanel';
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
    <ReportLayout
      title={data.title}
      badges={
        <div className="flex flex-wrap gap-3 text-xs text-muted-foreground">
          <span>Variables: {var_names.join(', ')}</span>
          <span>n = {num_observation}</span>
          <span>rank = {rank}</span>
          <span>lags = {lags}</span>
          <span>trend: {trend_spec}</span>
        </div>
      }
    >
      <ReportSection title="Model Summary" icon="modelSummary">
        <div className="mb-6 grid grid-cols-2 gap-px overflow-hidden rounded-lg border border-border bg-border">
          <div className="flex justify-between bg-card px-4 py-2.5">
            <span className="text-xs text-muted-foreground">Log likelihood</span>
            <span className="font-mono text-xs font-medium text-foreground">{formatNum(log_likelihood)}</span>
          </div>
          <div className="flex justify-between bg-card px-4 py-2.5">
            <span className="text-xs text-muted-foreground">AIC</span>
            <span className="font-mono text-xs font-medium text-foreground">{formatNum(aic)}</span>
          </div>
          <div className="flex justify-between bg-card px-4 py-2.5">
            <span className="text-xs text-muted-foreground">HQIC</span>
            <span className="font-mono text-xs font-medium text-foreground">{formatNum(hqic)}</span>
          </div>
          <div className="flex justify-between bg-card px-4 py-2.5">
            <span className="text-xs text-muted-foreground">SBIC</span>
            <span className="font-mono text-xs font-medium text-foreground">{formatNum(sbic)}</span>
          </div>
          <div className="col-span-2 flex justify-between bg-card px-4 py-2.5">
            <span className="text-xs text-muted-foreground">Det(Sigma_ml)</span>
            <span className="font-mono text-xs font-medium text-foreground">{formatNum(det_sigma_ml)}</span>
          </div>
        </div>
      </ReportSection>
      {equations.length > 0 && (
        <ReportSection title="Equation Summary" icon="anova">
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
        </ReportSection>
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
        <ReportSection title="Cointegrating equations" icon="classification">
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
          <div className="mb-4 mt-2 text-xs text-muted-foreground">Identification: beta is exactly identified</div>
        </ReportSection>
      )}

      {betaCoeffs.length > 0 && (
        <>
          <div className="mb-2 text-xs text-muted-foreground">Johansen normalization restriction imposed</div>
          <ReportSection title="beta" icon="firstStage">
            <CoefficientTable
              coefficients={betaCoeffs}
              hasCategorical={true}
              useZStat={true}
              categoryLabel="Equation"
            />
          </ReportSection>
        </>
      )}

      {vecstableSorted.length > 0 && (
        <ReportSection title="Eigenvalue stability condition" icon="margins">
          <VarEigenvalueStabilityPanel
            rows={vecstableSorted}
            unstableMessage="At least one eigenvalue is at least 1.0. VEC does not satisfy stability condition."
          />
        </ReportSection>
      )}
      {veclmar.length > 0 && (
        <ReportSection title="Lagrange-multiplier test (veclmar)" icon="margins">
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
        </ReportSection>
      )}

      {equations.length === 0 && coefficients.length === 0 && beta.length === 0 && (
        <div className="mb-6 rounded-lg border border-amber-800/50 bg-amber-900/10 px-4 py-3 text-sm text-amber-200/80">
          VEC 协整估计尚未实现，当前为占位结果。待实现 Johansen  procedure 后显示完整输出。
        </div>
      )}
    </ReportLayout>
  );
};
