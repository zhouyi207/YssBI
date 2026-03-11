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
  computeKDE,
} from './shared';
import type { OLSResultData } from './shared/types';

const FormulaBlock = React.lazy(() => import('./FormulaBlock'));
const FormulaBlock2SLS = React.lazy(() => import('./FormulaBlock2SLS'));
const ResidualPlot = React.lazy(() => import('./ResidualPlot'));
const Scatter = React.lazy(() => import('@/views/PlotView/Scatter'));
const KDE = React.lazy(() => import('@/views/PlotView/KDE'));

export type { OLSResultData };

export const TwoSLSComponent: React.FC<{ data: OLSResultData }> = ({ data }) => {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

  const significantCount = useMemo(
    () => coefficients.filter((c) => c.is_significant).length,
    [coefficients]
  );

  const hasCategorical = useMemo(
    () => coefficients.some((c) => c.category != null),
    [coefficients]
  );

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
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-medium bg-amber-500/20 text-amber-400 border border-amber-500/30">
            IV:2SLS
          </span>
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
        {diag.iv2sls_first_stage && diag.iv2sls_first_stage.length > 0 ? (
          <FormulaBlock2SLS
            endogName={data.endog_name || 'y'}
            coefficients={coefficients}
            firstStage={diag.iv2sls_first_stage}
          />
        ) : (
          <FormulaBlock endogName={data.endog_name || 'y'} coefficients={coefficients} />
        )}
      </Suspense>

      {/* First Stage Regression Results */}
      {diag.iv2sls_first_stage && diag.iv2sls_first_stage.length > 0 && (
        <>
          <SectionHeader
            title="First Stage Regression Results"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
              </svg>
            }
          />
          <div className="space-y-4">
            {diag.iv2sls_first_stage.map((fs) => (
              <div
                key={fs.endog_name}
                className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden"
              >
                <div className="px-4 py-2.5 border-b border-gray-800/50 flex items-center justify-between">
                  <span className="text-sm font-medium text-white">
                    {fs.endog_name} on exog + instruments
                  </span>
                  <span className="text-xs text-gray-500">
                    R² = {fs.r_squared.toFixed(4)} &middot; Adj R² = {fs.adj_r_squared.toFixed(4)}
                  </span>
                </div>
                <CoefficientTable
                  coefficients={fs.coefficients}
                  hasCategorical={false}
                />
              </div>
            ))}
          </div>
        </>
      )}

      {/* First Stage Summary (estat firststage) */}
      {diag.iv2sls_first_stage_summary && (
        <>
          <SectionHeader
            title="First Stage Summary (estat firststage)"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <div className="space-y-3 mb-4">
            {/* Instrument counts */}
            <div className="flex flex-wrap gap-x-6 gap-y-1 text-xs text-gray-400">
              <span>Included instruments: <span className="font-mono text-gray-300">{diag.iv2sls_first_stage_summary.k_included_instruments}</span></span>
              <span>Excluded instruments: <span className="font-mono text-gray-300">{diag.iv2sls_first_stage_summary.k_excluded_instruments}</span></span>
              <span>Endogenous regressors: <span className="font-mono text-gray-300">{diag.iv2sls_first_stage_summary.k_endogenous_regressors}</span></span>
            </div>
            {/* Single endog: R², Adj R², Partial R², F table */}
            {diag.iv2sls_first_stage_summary.r2 != null ? (
              <div className="rounded-lg border border-gray-800/50 overflow-hidden">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="bg-[#1a1d23]">
                      <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Variable</th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">R-sq.</th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Adj R-sq.</th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Partial R-sq.</th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">
                        F({diag.iv2sls_first_stage_summary.f_df1 ?? 0},{diag.iv2sls_first_stage_summary.f_df2 ?? 0})
                      </th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Prob &gt; F</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr className="bg-[#13151a] border-t border-gray-800/30 hover:bg-[#1e2128]">
                      <td className="px-4 py-2.5 font-mono text-white">
                        {diag.iv2sls_first_stage?.[0]?.endog_name ?? '—'}
                      </td>
                      <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(diag.iv2sls_first_stage_summary.r2, 4)}</td>
                      <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(diag.iv2sls_first_stage_summary.r2_adjusted, 4)}</td>
                      <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(diag.iv2sls_first_stage_summary.partial_r2, 4)}</td>
                      <td className="text-right px-3 py-2.5 font-mono text-gray-300">
                        {diag.iv2sls_first_stage_summary.f_stat != null ? formatNum(diag.iv2sls_first_stage_summary.f_stat, 4) : '—'}
                      </td>
                      <td className="text-right px-3 py-2.5 font-mono text-gray-300">
                        {diag.iv2sls_first_stage_summary.f_p_value != null ? formatNum(diag.iv2sls_first_stage_summary.f_p_value, 4) : '—'}
                      </td>
                    </tr>
                  </tbody>
                </table>
              </div>
            ) : (
              /* Multi endog: Shea's partial R² table */
              <div className="rounded-lg border border-gray-800/50 overflow-hidden">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="bg-[#1a1d23]">
                      <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Variable</th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Shea&apos;s partial R-sq.</th>
                      <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Shea&apos;s adj. partial R-sq.</th>
                    </tr>
                  </thead>
                  <tbody>
                    {diag.iv2sls_first_stage.map((fs, i) => (
                      <tr
                        key={fs.endog_name}
                        className={`border-t border-gray-800/30 hover:bg-[#1e2128] ${i % 2 === 0 ? 'bg-[#13151a]' : 'bg-[#15171d]'}`}
                      >
                        <td className="px-4 py-2.5 font-mono text-white">{fs.endog_name}</td>
                        <td className="text-right px-3 py-2.5 font-mono text-gray-300">
                          {formatNum(diag.iv2sls_first_stage_summary.shea_partial_r2[i] ?? 0, 4)}
                        </td>
                        <td className="text-right px-3 py-2.5 font-mono text-gray-300">
                          {formatNum(diag.iv2sls_first_stage_summary.shea_adj_partial_r2[i] ?? 0, 4)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {/* Minimum eigenvalue + Stock-Yogo critical values（robust 时整块隐藏） */}
            {diag.iv2sls_first_stage_summary.min_eigenvalue_cv_note !== 'robust' && (
            <div className="rounded-lg border border-gray-800/50 overflow-hidden">
              <div className="px-4 py-2.5 bg-[#1a1d23] border-b border-gray-800/50 flex items-center justify-between">
                <div>
                  <span className="text-[11px] text-gray-500 uppercase tracking-wider">Minimum eigenvalue statistic</span>
                  <span className="ml-2 font-mono text-white font-medium">{formatNum(diag.iv2sls_first_stage_summary.min_eigenvalue, 4)}</span>
                </div>
                {diag.iv2sls_first_stage_summary.min_eigenvalue_cv && (
                  <span className="text-[10px] text-gray-500">Stock-Yogo (2005)</span>
                )}
              </div>
              {diag.iv2sls_first_stage_summary.min_eigenvalue_cv && (() => {
                const cv = diag.iv2sls_first_stage_summary.min_eigenvalue_cv;
                const cellClass = "text-right px-4 py-2 font-mono text-gray-300 tabular-nums";
                const labelClass = "text-left px-4 py-2 text-[11px] text-gray-400";
                const thClass = "text-right px-4 py-2 text-gray-500 font-medium tabular-nums";
                return (
                  <div className="overflow-x-auto">
                    <table className="w-full text-xs table-fixed">
                      <colgroup>
                        <col className="w-[min(16rem,45%)]" />
                        <col className="w-[4.5rem]" />
                        <col className="w-[4.5rem]" />
                        <col className="w-[4.5rem]" />
                        <col className="w-[4.5rem]" />
                      </colgroup>
                      <thead>
                        <tr className="bg-[#15171d]">
                          <th className="text-left px-4 py-2 text-gray-500 font-medium uppercase tracking-wider text-[10px]">Test</th>
                          <th className={thClass}>5%</th>
                          <th className={thClass}>10%</th>
                          <th className={thClass}>20%</th>
                          <th className={thClass}>30%</th>
                        </tr>
                      </thead>
                      <tbody>
                        <tr className="bg-[#13151a] border-t border-gray-800/30">
                          <td className={labelClass}>2SLS relative bias</td>
                          {cv.bias ? (
                            <>
                              <td className={cellClass}>{cv.bias.pct_5.toFixed(2)}</td>
                              <td className={cellClass}>{cv.bias.pct_10.toFixed(2)}</td>
                              <td className={cellClass}>{cv.bias.pct_20.toFixed(2)}</td>
                              <td className={cellClass}>{cv.bias.pct_30.toFixed(2)}</td>
                            </>
                          ) : (
                            <td colSpan={4} className="text-right px-4 py-2 text-gray-500 italic">(not available)</td>
                          )}
                        </tr>
                      </tbody>
                      <thead>
                        <tr className="bg-[#15171d] border-t border-gray-800/50">
                          <th className="text-left px-4 py-2 text-gray-500 font-medium uppercase tracking-wider text-[10px]">Test</th>
                          <th className={thClass}>10%</th>
                          <th className={thClass}>15%</th>
                          <th className={thClass}>20%</th>
                          <th className={thClass}>25%</th>
                        </tr>
                      </thead>
                      <tbody>
                        <tr className="bg-[#13151a] border-t border-gray-800/30">
                          <td className={labelClass}>2SLS size of nominal 5% Wald test</td>
                          <td className={cellClass}>{cv.size.pct_10.toFixed(2)}</td>
                          <td className={cellClass}>{cv.size.pct_15.toFixed(2)}</td>
                          <td className={cellClass}>{cv.size.pct_20.toFixed(2)}</td>
                          <td className={cellClass}>{cv.size.pct_25.toFixed(2)}</td>
                        </tr>
                      </tbody>
                    </table>
                  </div>
                );
              })()}
              {!diag.iv2sls_first_stage_summary.min_eigenvalue_cv && (
                <div className="px-4 py-2.5 bg-[#13151a] text-[11px] text-gray-500">
                  {diag.iv2sls_first_stage_summary.min_eigenvalue_cv_note === 'k_endog_gt_2'
                    ? 'Stock-Yogo critical values not available for 3+ endogenous regressors'
                    : 'Stock-Yogo critical values not shown'}
                </div>
              )}
            </div>
            )}
          </div>
        </>
      )}

      {/* Overidentification Test (estat overid) */}
      {diag.iv2sls_overid_dims && (
        <>
          <SectionHeader
            title="Overidentification Test (estat overid)"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
            }
          />
          <div className="mb-4">
            {diag.iv2sls_overid ? (
              <>
                <div className="flex items-center justify-between mb-2 px-1">
                  <span className="text-[11px] text-gray-500 uppercase tracking-wider">
                    Tests of overidentifying restrictions (df = {diag.iv2sls_overid.df})
                    {diag.iv2sls_overid.test_type === 'wooldridge' && ' — Wooldridge score (robust)'}
                  </span>
                </div>
                <Chi2TestCards
                  cards={
                    diag.iv2sls_overid.test_type === 'wooldridge'
                      ? [
                          {
                            label: 'Wooldridge score',
                            chi2: diag.iv2sls_overid.wooldridge_stat ?? 0,
                            df: diag.iv2sls_overid.df,
                            p_value: diag.iv2sls_overid.wooldridge_p_value ?? 0,
                          },
                        ]
                      : [
                          {
                            label: 'Sargan',
                            chi2: diag.iv2sls_overid.sargan_stat ?? 0,
                            df: diag.iv2sls_overid.df,
                            p_value: diag.iv2sls_overid.sargan_p_value ?? 0,
                          },
                          {
                            label: 'Basmann',
                            chi2: diag.iv2sls_overid.basmann_stat ?? 0,
                            df: diag.iv2sls_overid.df,
                            p_value: diag.iv2sls_overid.basmann_p_value ?? 0,
                          },
                        ]
                  }
                />
                <p className="text-xs text-gray-500 mt-2 px-1">
                  H0: overidentifying restrictions are valid. Significant p-value suggests instruments may not be valid.
                  {diag.iv2sls_overid.test_type === 'wooldridge' &&
                    ' Wooldridge (1995) score test is used with robust VCE (Sargan/Basmann assume homoskedasticity).'}
                </p>
              </>
            ) : (
              <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3">
                <p className="text-sm text-amber-200">
                  Model is exactly identified (k_iv = {diag.iv2sls_overid_dims.k_iv}, k_endog = {diag.iv2sls_overid_dims.k_endog}).
                </p>
                <p className="text-xs text-gray-400 mt-1">
                  The overidentification test requires k_iv &gt; k_endog (excluded instruments &gt; endogenous variables). Exogenous variables are not counted as instruments.
                </p>
              </div>
            )}
          </div>
        </>
      )}

      {/* Tests of Endogeneity (estat endogenous) + Hausman (hausman iv ols, constant sigmamore) */}
      {(diag.iv2sls_endogenous || diag.iv2sls_hausman) && (
        <>
          <SectionHeader
            title="Tests of Endogeneity"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
              </svg>
            }
          />
          <div className="mb-4">
            <div className="flex items-center justify-between mb-2 px-1">
              <span className="text-[11px] text-gray-500 uppercase tracking-wider">
                H0: variables are exogenous
              </span>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
              {diag.iv2sls_endogenous && (
                <>
                  <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                    <div className="text-[11px] text-gray-500 font-mono mb-2">Durbin (score)</div>
                    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                      <span className="text-gray-400">
                        chi2({diag.iv2sls_endogenous.df}) = <span className="font-mono text-white">{formatNum(diag.iv2sls_endogenous.durbin_stat)}</span>
                      </span>
                      <span className="text-gray-400">
                        p = <span className={`font-mono ${diag.iv2sls_endogenous.durbin_p_value < 0.05 ? 'text-emerald-400' : 'text-gray-400'}`}>{formatNum(diag.iv2sls_endogenous.durbin_p_value)}</span>
                      </span>
                    </div>
                    <div className="mt-1.5 text-[10px]">
                      {diag.iv2sls_endogenous.durbin_p_value < 0.05 ? (
                        <span className="text-amber-400">拒绝 H0</span>
                      ) : (
                        <span className="text-gray-500">不拒绝 H0</span>
                      )}
                    </div>
                  </div>
                  <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                    <div className="text-[11px] text-gray-500 font-mono mb-2">Wu-Hausman</div>
                    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                      <span className="text-gray-400">
                        F({diag.iv2sls_endogenous.df},{diag.iv2sls_endogenous.wu_df_denom}) = <span className="font-mono text-white">{formatNum(diag.iv2sls_endogenous.wu_stat)}</span>
                      </span>
                      <span className="text-gray-400">
                        p = <span className={`font-mono ${diag.iv2sls_endogenous.wu_p_value < 0.05 ? 'text-emerald-400' : 'text-gray-400'}`}>{formatNum(diag.iv2sls_endogenous.wu_p_value)}</span>
                      </span>
                    </div>
                    <div className="mt-1.5 text-[10px]">
                      {diag.iv2sls_endogenous.wu_p_value < 0.05 ? (
                        <span className="text-amber-400">拒绝 H0</span>
                      ) : (
                        <span className="text-gray-500">不拒绝 H0</span>
                      )}
                    </div>
                  </div>
                </>
              )}
              {diag.iv2sls_hausman && (
                <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors">
                  <div className="text-[11px] text-gray-500 font-mono mb-2">Hausman (sigmamore)</div>
                  <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                    <span className="text-gray-400">
                      chi2({diag.iv2sls_hausman.df}) = <span className="font-mono text-white">{formatNum(diag.iv2sls_hausman.stat)}</span>
                    </span>
                    <span className="text-gray-400">
                      p = <span className={`font-mono ${diag.iv2sls_hausman.p_value < 0.05 ? 'text-emerald-400' : 'text-gray-400'}`}>{formatNum(diag.iv2sls_hausman.p_value)}</span>
                    </span>
                  </div>
                  <div className="mt-1.5 text-[10px]">
                    {diag.iv2sls_hausman.p_value < 0.05 ? (
                      <span className="text-amber-400">拒绝 H0</span>
                    ) : (
                      <span className="text-gray-500">不拒绝 H0</span>
                    )}
                  </div>
                </div>
              )}
            </div>
            <p className="text-xs text-gray-500 mt-2 px-1">
              estat endogenous: Durbin &amp; Wu-Hausman. hausman iv ols, constant sigmamore: traditional Hausman. Significant p-value favors IV.
            </p>
          </div>
        </>
      )}

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
      <CoefficientTable coefficients={coefficients} hasCategorical={hasCategorical} useZStat />

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
          {diag.leverage && diag.leverage.length > 0 && (
            <div className="mb-4">
              <div className="flex items-center justify-between mb-2 px-1">
                <span className="text-[11px] text-gray-500 uppercase tracking-wider">Leverage KDE (Stata predict lev, leverage)</span>
              </div>
              <Suspense fallback={<div className="rounded-lg border border-gray-800/50 bg-[#13151a] h-[280px] animate-pulse" />}>
                <KDE
                  data={leverageKdeData}
                  xLabel="Leverage"
                  yLabel="Density"
                  height={280}
                  xMin={0}
                />
              </Suspense>
            </div>
          )}
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-gray-500 uppercase tracking-wider">Residuals vs Fitted</span>
            {diag.timing?.fitted_residuals_ms != null && (
              <span className="text-[10px] text-[var(--accent-color)] font-mono">{diag.timing.fitted_residuals_ms} ms</span>
            )}
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
