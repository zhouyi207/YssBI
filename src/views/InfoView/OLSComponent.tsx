import React, { Suspense, useMemo } from 'react';
import {
  SectionHeader,
  StatCard,
  RSquaredBadge,
  Chi2TestCards,
  BP_VARIANTS,
  FTestCards,
  OV_VARIANTS,
  formatNum,
  ModelSummaryGrid,
  AnovaTable,
  CoefficientTable,
  CoeffBarChart,
  HypothesisTestBlock,
  ACFPACFBlock,
  SerialTestsBlock,
} from './shared';
import type { OLSResultData } from './shared/types';

const FormulaBlock = React.lazy(() => import('./FormulaBlock'));
const ResidualPlot = React.lazy(() => import('./ResidualPlot'));
const Scatter = React.lazy(() => import('@/views/PlotView/Scatter'));

export type { Coefficient, OLSResultData } from './shared/types';

export const OLSComponent: React.FC<{ data: OLSResultData }> = ({ data }) => {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

  const significantCount = useMemo(
    () => coefficients.filter((c) => c.is_significant).length,
    [coefficients]
  );

  const hasCategorical = useMemo(
    () => coefficients.some((c) => c.category != null),
    [coefficients]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <RSquaredBadge value={info.r_squared} />
          <span className="text-xs text-gray-500">
            {info.method} &middot; n={info.num_observation}
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
        <FormulaBlock endogName={data.endog_name || 'y'} coefficients={coefficients} />
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
      <CoefficientTable coefficients={coefficients} hasCategorical={hasCategorical} />

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

      {/* Diagnostics */}
      <SectionHeader
        title="Diagnostics"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        }
      />

      <div className="mb-4">
        <div className="flex items-center justify-between mb-2 px-1">
          <span className="text-[11px] text-gray-500 uppercase tracking-wider">
            Multicollinearity — Condition Number & VIF (Stata estat vif)
          </span>
        </div>
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-3">
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

      {diag.bp_tests && (
        <div className="mb-4">
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">
              Breusch-Pagan (Heteroscedasticity) — Stata estat hettest 四种变体
            </span>
            {diag.timing?.bp_tests_ms != null && (
              <span className="text-[10px] text-[var(--accent-color)] font-mono">{diag.timing.bp_tests_ms} ms</span>
            )}
          </div>
          <Chi2TestCards
            cards={BP_VARIANTS.filter(({ key }) => diag.bp_tests![key]).map(({ key, label }) => ({
              label,
              chi2: diag.bp_tests![key]!.lm_stat,
              df: diag.bp_tests![key]!.df,
              p_value: diag.bp_tests![key]!.p_value,
            }))}
          />
        </div>
      )}

      {diag.ov_tests && (
        <div className="mb-4">
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">
              Ramsey RESET (Omitted Variables) — Stata estat ovtest 两种变体
            </span>
            {diag.timing?.ov_tests_ms != null && (
              <span className="text-[10px] text-[var(--accent-color)] font-mono">{diag.timing.ov_tests_ms} ms</span>
            )}
          </div>
          <FTestCards
            cards={OV_VARIANTS.filter(({ key }) => diag.ov_tests![key]).map(({ key, label }) => ({
              label,
              f_stat: diag.ov_tests![key]!.f_stat,
              df1: diag.ov_tests![key]!.df1,
              df2: diag.ov_tests![key]!.df2,
              p_value: diag.ov_tests![key]!.p_value,
            }))}
          />
        </div>
      )}

      {diag.im_test ? (
        <div className="mb-4">
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">
              Cameron & Trivedi&apos;s decomposition of IM-test — Stata estat imtest
            </span>
            {diag.timing?.im_test_ms != null && (
              <span className="text-[10px] text-[var(--accent-color)] font-mono">{diag.timing.im_test_ms} ms</span>
            )}
          </div>
          <Chi2TestCards
            cards={[
              { label: 'Heteroskedasticity', ...diag.im_test.heteroskedasticity },
              { label: 'Skewness', ...diag.im_test.skewness },
              { label: 'Kurtosis', ...diag.im_test.kurtosis },
              { label: 'Total', ...diag.im_test.total },
            ]}
          />
        </div>
      ) : null}

      {diag.normality_tests ? (
        <div className="mb-4">
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">
              Residual Normality (Omnibus / Jarque-Bera)
            </span>
          </div>
          <Chi2TestCards
            cards={[
              {
                label: 'Omnibus',
                chi2: diag.normality_tests.omnibus_stat,
                df: 2,
                p_value: diag.normality_tests.omnibus_p_value,
              },
              {
                label: 'Jarque-Bera',
                chi2: diag.normality_tests.jarque_bera_stat,
                df: 2,
                p_value: diag.normality_tests.jarque_bera_p_value,
              },
            ]}
          />
        </div>
      ) : null}

      {diag.fitted_values && diag.residuals && diag.fitted_values.length > 0 && (
        <>
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">Residuals vs Fitted</span>
            {diag.timing?.fitted_residuals_ms != null && (
              <span className="text-[10px] text-[var(--accent-color)] font-mono">{diag.timing.fitted_residuals_ms} ms</span>
            )}
          </div>
          <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
            <ResidualPlot fitted={diag.fitted_values} residuals={diag.residuals} />
          </Suspense>

          {diag.residual_scatter && diag.residual_scatter.e.length > 0 && diag.residual_scatter.e_lag1.length > 0 && (
            <div className="mt-4">
              <div className="flex items-center justify-between mb-2 px-1">
                <span className="text-[11px] text-gray-500 uppercase tracking-wider">Residuals: e vs e_lag1</span>
              </div>
              <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
                <Scatter
                  data={diag.residual_scatter.e_lag1.map((x, i) => ({ x, y: diag.residual_scatter!.e[i] }))}
                  xLabel="e_{t-1}"
                  yLabel="e_t"
                  height={280}
                  symmetricY
                  zeroLine
                />
              </Suspense>
            </div>
          )}

          <ACFPACFBlock residuals={diag.residuals} />
          <SerialTestsBlock residuals={diag.residuals} exog={diag.exog} />

          {diag.normality_tests ? (
            <div className="mt-4">
              <div className="flex items-center justify-between mb-2 px-1">
                <span className="text-[11px] text-gray-500 uppercase tracking-wider">Skew & Kurtosis</span>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                  <div className="text-[11px] text-gray-500 font-mono mb-2">Skew</div>
                  <div className="text-white font-mono text-sm font-medium">{formatNum(diag.normality_tests.skewness)}</div>
                </div>
                <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                  <div className="text-[11px] text-gray-500 font-mono mb-2">Kurtosis</div>
                  <div className="text-white font-mono text-sm font-medium">{formatNum(diag.normality_tests.kurtosis)}</div>
                </div>
              </div>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
};
