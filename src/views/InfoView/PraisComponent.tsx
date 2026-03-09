import React, { Suspense, useMemo } from 'react';
import {
  SectionHeader,
  StatCard,
  RSquaredBadge,
  formatNum,
  ModelSummaryGrid,
  AnovaTable,
  CoefficientTable,
  CoeffBarChart,
  HypothesisTestBlock,
  ACFPACFBlock,
  SerialTestsBlock,
  computeKDE,
} from './shared';
import type { RegressionResultData } from './shared/types';

const FormulaBlock = React.lazy(() => import('./FormulaBlock'));
const ResidualPlot = React.lazy(() => import('./ResidualPlot'));
const Scatter = React.lazy(() => import('@/views/PlotView/Scatter'));
const KDE = React.lazy(() => import('@/views/PlotView/KDE'));

export interface PraisResultData extends RegressionResultData {
  diagnostic_info: RegressionResultData['diagnostic_info'] & {
    prais_info: {
      rho: number;
      dw_original: number;
      dw_transformed: number;
      iterations: number;
      iteration_log?: string[];
    };
  };
}

export const PraisComponent: React.FC<{ data: PraisResultData }> = ({ data }) => {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;
  const praisInfo = diag.prais_info!;
  const significantCount = coefficients.filter((c) => c.is_significant).length;
  const hasCategorical = coefficients.some((c) => c.category != null);
  const leverageKdeData = useMemo(
    () => (diag.leverage && diag.leverage.length > 0 ? computeKDE(diag.leverage, 256, 0) : []),
    [diag.leverage]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <RSquaredBadge value={info.r_squared} />
          <span className="text-xs text-gray-500">
            {info.method} &middot; n={info.num_observation} &middot; ρ={formatNum(praisInfo.rho)}
          </span>
        </div>
      </div>

      {/* Equation */}
      <SectionHeader
        title="Equation"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4.745 3A23.933 23.933 0 003 12c0 3.183.62 6.22 1.745 9M19.5 3c.967 2.78 1.5 5.817 1.5 9s-.533 6.22-1.5 9M8.25 8.885l1.444-.89a.75.75 0 011.105.402l2.402 7.206a.75.75 0 001.104.401l1.445-.889" />
          </svg>
        }
      />
      <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-24 animate-pulse" />}>
        <FormulaBlock endogName={data.endog_name || 'y'} coefficients={coefficients} ar1Rho={praisInfo.rho} />
      </Suspense>

      {/* Model Summary */}
      <SectionHeader
        title="Model Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        }
      />
      <ModelSummaryGrid info={info} executionTimeMs={data.executionTimeMs} />

      {/* ANOVA */}
      <SectionHeader
        title="ANOVA"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
          </svg>
        }
      />
      <AnovaTable info={info} />

      {/* Coefficients */}
      <SectionHeader
        title={`Coefficients (${significantCount}/${coefficients.length} significant)`}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
          </svg>
        }
      />
      <CoefficientTable coefficients={coefficients} hasCategorical={hasCategorical} ar1Rho={praisInfo.rho} />

      {/* Hypothesis Test */}
      <HypothesisTestBlock data={data} />

      {/* Coefficient Bar */}
      <SectionHeader
        title="Coefficient Magnitude"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 8v8m-4-5v5m-4-2v2m-2 4h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
        }
      />
      <CoeffBarChart coefficients={coefficients} />

      {/* Prais Diagnostics */}
      <SectionHeader
        title="AR(1) Diagnostics"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        }
      />
      <div className="space-y-3 mb-4">
        <div className="grid grid-cols-2 gap-3">
          <StatCard
            label="DW (original)"
            value={formatNum(praisInfo.dw_original)}
            sub="检验对象: 初始 OLS 残差 u_t"
          />
          <StatCard
            label="DW (transformed)"
            value={formatNum(praisInfo.dw_transformed)}
            sub="检验对象: 变换后残差 e_t"
          />
        </div>
        <StatCard label="Iterations" value={praisInfo.iterations} sub="Convergence" />
        <div className="flex items-center justify-between mb-2 px-1">
          <span className="text-[11px] text-gray-500 uppercase tracking-wider">
            Multicollinearity — Condition Number & VIF (Stata estat vif)
          </span>
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-2">
          <StatCard
            label="Condition Number"
            value={formatNum(diag.cond_no)}
            sub={diag.cond_no > 1000 ? 'Possible multicollinearity' : 'Acceptable'}
          />
          {diag.vif && diag.vif.length > 0 && (() => {
            const finite = diag.vif.filter((e) => Number.isFinite(e.vif));
            const meanVif = finite.length > 0 ? finite.reduce((s, e) => s + e.vif, 0) / finite.length : null;
            const fmt = (v: number) => (!Number.isFinite(v) ? 'Inf' : v >= 1e6 ? v.toExponential(2) : v.toFixed(4));
            return meanVif != null ? (
              <StatCard
                label="Mean VIF"
                value={fmt(meanVif)}
                sub={meanVif > 10 ? 'High multicollinearity' : meanVif > 5 ? 'Moderate' : 'Low'}
              />
            ) : null;
          })()}
        </div>
        {diag.vif && diag.vif.length > 0 && (
          <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-gray-800/50">
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Variable</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">VIF</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">1/VIF</th>
                </tr>
              </thead>
              <tbody>
                {diag.vif.map((row) => {
                  const fmt = (v: number) => (!Number.isFinite(v) ? 'Inf' : v >= 1e6 ? v.toExponential(2) : v.toFixed(4));
                  return (
                    <tr key={row.variable} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                      <td className="px-4 py-2.5 font-mono text-white">{row.variable}</td>
                      <td className="px-4 py-2.5 font-mono text-gray-300">{fmt(row.vif)}</td>
                      <td className="px-4 py-2.5 font-mono text-gray-300">{fmt(row.tolerance)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {praisInfo.iteration_log && praisInfo.iteration_log.length > 0 && (
        <div className="rounded-lg border border-gray-800/50 bg-[#13151a] overflow-hidden mb-4">
          <div className="text-[11px] text-gray-500 uppercase tracking-wider px-4 py-2 bg-[#1a1d23] border-b border-gray-800/40">
            Iteration Log
          </div>
          <pre className="px-4 py-3 text-xs font-mono text-gray-300 overflow-x-auto max-h-40 overflow-y-auto">
            {praisInfo.iteration_log.join('\n')}
          </pre>
        </div>
      )}

      {/* Residuals vs Fitted */}
      {diag.fitted_values && diag.residuals && diag.fitted_values.length > 0 && (
        <>
          {diag.leverage && diag.leverage.length > 0 && (
            <div className="mb-4">
              <div className="flex items-center justify-between mb-2 px-1">
                <span className="text-[11px] text-gray-500 uppercase tracking-wider">Leverage KDE (Stata predict lev, leverage)</span>
              </div>
              <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
                <KDE data={leverageKdeData} xLabel="Leverage" yLabel="Density" height={280} xMin={0} />
              </Suspense>
            </div>
          )}
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">Residuals vs Fitted</span>
            <span className="text-[10px] text-gray-500">检验对象: u_t (Prais 收敛后)</span>
          </div>
          <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
            <ResidualPlot
              fitted={diag.fitted_values}
              residuals={diag.residuals}
              leverage={diag.leverage}
            />
          </Suspense>

          {diag.residual_scatter && diag.residual_scatter.e.length > 0 && diag.residual_scatter.e_lag1.length > 0 && (
            <div className="mt-4">
              <div className="flex items-center justify-between mb-2 px-1">
                <span className="text-[11px] text-gray-500 uppercase tracking-wider">Residuals: u_t vs u_{'{t-1}'}</span>
              </div>
              <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
                <Scatter
                  data={diag.residual_scatter.e_lag1.map((x, i) => ({ x, y: diag.residual_scatter!.e[i] }))}
                  xLabel="u_{t-1}"
                  yLabel="u_t"
                  height={280}
                  symmetricY
                  zeroLine
                />
              </Suspense>
            </div>
          )}

          <ACFPACFBlock residuals={diag.residuals} residualLabel="u_t (Prais 收敛后)" />
          <SerialTestsBlock residuals={diag.residuals} exog={diag.exog} residualLabel="u_t (Prais 收敛后)" />
        </>
      )}
    </div>
  );
};
