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
  CoefficientsBlock,
  HypothesisTestBlock,
  ACFPACFBlock,
  SerialTestsBlock,
  VifTable,
  IvFirstStageSummaryTables,
  meanFiniteVif,
  computeKDE,
} from './shared';
import type { OLSResultData } from './shared/types';

const FormulaBlock = React.lazy(() => import('./FormulaBlock'));
const FormulaBlock2SLS = React.lazy(() => import('./FormulaBlock2SLS'));
const ResidualPlot = React.lazy(() => import('./ResidualPlot'));
const Scatter = React.lazy(() => import('@/views/PlotView/Scatter'));
const KDE = React.lazy(() => import('@/views/PlotView/KDE'));

export const LIMLComponent: React.FC<{ data: OLSResultData }> = ({ data }) => {
  const { model_basic_info: info, coefficients, diagnostic_info: diag } = data;

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
        <h1 className="text-xl font-bold text-foreground mb-2">{data.title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <RSquaredBadge value={info.r_squared} />
          <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-medium bg-violet-500/20 text-violet-400 border border-violet-500/30">
            IV:LIML
          </span>
          {diag.ivliml_kappa != null && (
            <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-medium bg-muted text-foreground border border-border">
              κ = {formatNum(diag.ivliml_kappa, 6)}
            </span>
          )}
          <span className="text-xs text-muted-foreground">
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
      <Suspense fallback={<div className="rounded-lg border border-border bg-card h-24 animate-pulse" />}>
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
                className="rounded-lg border border-border bg-muted overflow-hidden"
              >
                <div className="px-4 py-2.5 border-b border-border flex items-center justify-between">
                  <span className="text-sm font-medium text-foreground">
                    {fs.endog_name} on exog + instruments
                  </span>
                  <span className="text-xs text-muted-foreground">
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

      {/* First Stage Summary (estat firststage) — LIML */}
      {diag.iv2sls_first_stage_summary && (
        <>
          <SectionHeader
            title="First Stage Summary (estat firststage) — LIML"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <IvFirstStageSummaryTables
            summary={diag.iv2sls_first_stage_summary}
            firstStage={diag.iv2sls_first_stage}
            variant="liml"
          />
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
            {diag.ivliml_overid ? (
              <>
                <div className="flex items-center justify-between mb-2 px-1">
                  <span className="text-[11px] text-muted-foreground uppercase tracking-wider">
                    Tests of overidentifying restrictions (df = {diag.ivliml_overid.df})
                  </span>
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                  <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                    <div className="text-[11px] text-muted-foreground font-mono mb-2">
                      Anderson-Rubin chi2({diag.ivliml_overid.df})
                    </div>
                    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                      <span className="text-muted-foreground">
                        chi2 = <span className="font-mono text-foreground">{formatNum(diag.ivliml_overid.anderson_rubin_stat)}</span>
                      </span>
                      <span className="text-muted-foreground">
                        df = <span className="font-mono text-foreground">{diag.ivliml_overid.df}</span>
                      </span>
                      <span className="text-muted-foreground">
                        p = <span className={`font-mono ${diag.ivliml_overid.anderson_rubin_p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}>{formatNum(diag.ivliml_overid.anderson_rubin_p_value)}</span>
                      </span>
                    </div>
                    <div className="mt-1.5 text-[10px]">
                      {diag.ivliml_overid.anderson_rubin_p_value < 0.05 ? (
                        <span className="text-amber-400">Reject H0 — instruments may not be valid</span>
                      ) : (
                        <span className="text-muted-foreground">Do not reject H0 — overidentifying restrictions appear valid</span>
                      )}
                    </div>
                  </div>
                  <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                    <div className="text-[11px] text-muted-foreground font-mono mb-2">
                      Basmann F({diag.ivliml_overid.df},{diag.ivliml_overid.df_denom})
                    </div>
                    <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
                      <span className="text-muted-foreground">
                        F = <span className="font-mono text-foreground">{formatNum(diag.ivliml_overid.basmann_stat)}</span>
                      </span>
                      <span className="text-muted-foreground">
                        p = <span className={`font-mono ${diag.ivliml_overid.basmann_p_value < 0.05 ? 'text-emerald-400' : 'text-muted-foreground'}`}>{formatNum(diag.ivliml_overid.basmann_p_value)}</span>
                      </span>
                    </div>
                    <div className="mt-1.5 text-[10px]">
                      {diag.ivliml_overid.basmann_p_value < 0.05 ? (
                        <span className="text-amber-400">Reject H0 — instruments may not be valid</span>
                      ) : (
                        <span className="text-muted-foreground">Do not reject H0 — overidentifying restrictions appear valid</span>
                      )}
                    </div>
                  </div>
                </div>
                <p className="text-xs text-muted-foreground mt-2 px-1">
                  H0: overidentifying restrictions are valid. Significant p-value suggests instruments may not be valid.
                </p>
              </>
            ) : (
              <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-3">
                <p className="text-sm text-amber-200">
                  Model is {diag.iv2sls_overid_dims.k_iv > diag.iv2sls_overid_dims.k_endog ? 'overidentified' : 'exactly identified'} (k_iv = {diag.iv2sls_overid_dims.k_iv}, k_endog = {diag.iv2sls_overid_dims.k_endog}).
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  {diag.iv2sls_overid_dims.k_iv <= diag.iv2sls_overid_dims.k_endog
                    ? 'Overidentification test requires k_iv &gt; k_endog.'
                    : 'Overidentification test requires nonrobust VCE (homoskedastic errors).'}
                </p>
              </div>
            )}
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
      <CoefficientsBlock
        coefficients={coefficients}
        hasCategorical={hasCategorical}
        useZStat
      />

      {/* Hypothesis Test */}
      <HypothesisTestBlock data={data} />

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
          <span className="text-[11px] text-muted-foreground uppercase tracking-wider">
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
            const meanVif = meanFiniteVif(diag.vif);
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
        {diag.vif && diag.vif.length > 0 && <VifTable rows={diag.vif} />}
      </div>

      {diag.bp_tests && (
        <div className="mb-4">
          <div className="flex items-center justify-between mb-2 px-1">
            <span className="text-[11px] text-muted-foreground uppercase tracking-wider">
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
            <span className="text-[11px] text-muted-foreground uppercase tracking-wider">
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
            <span className="text-[11px] text-muted-foreground uppercase tracking-wider">
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
            <span className="text-[11px] text-muted-foreground uppercase tracking-wider">
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
                <span className="text-[11px] text-muted-foreground uppercase tracking-wider">Leverage KDE (Stata predict lev, leverage)</span>
              </div>
              <Suspense fallback={<div className="rounded-lg border border-border bg-card h-[280px] animate-pulse" />}>
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
            <span className="text-[11px] text-muted-foreground uppercase tracking-wider">Residuals vs Fitted</span>
            {diag.timing?.fitted_residuals_ms != null && (
              <span className="text-[10px] text-[var(--accent-color)] font-mono">{diag.timing.fitted_residuals_ms} ms</span>
            )}
          </div>
          <Suspense fallback={<div className="rounded-lg border border-border bg-card h-[280px] animate-pulse" />}>
            <ResidualPlot
              fitted={diag.fitted_values}
              residuals={diag.residuals}
              leverage={diag.leverage}
            />
          </Suspense>

          {diag.residual_scatter && diag.residual_scatter.e.length > 0 && diag.residual_scatter.e_lag1.length > 0 && (
            <div className="mt-4">
              <div className="flex items-center justify-between mb-2 px-1">
                <span className="text-[11px] text-muted-foreground uppercase tracking-wider">Residuals: e vs e_lag1</span>
              </div>
              <Suspense fallback={<div className="rounded-lg border border-border bg-card h-[280px] animate-pulse" />}>
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
                <span className="text-[11px] text-muted-foreground uppercase tracking-wider">Skew & Kurtosis</span>
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                  <div className="text-[11px] text-muted-foreground font-mono mb-2">Skew</div>
                  <div className="text-foreground font-mono text-sm font-medium">{formatNum(diag.normality_tests.skewness)}</div>
                </div>
                <div className="rounded-lg border border-border bg-muted px-4 py-3 hover:border-border transition-colors">
                  <div className="text-[11px] text-muted-foreground font-mono mb-2">Kurtosis</div>
                  <div className="text-foreground font-mono text-sm font-medium">{formatNum(diag.normality_tests.kurtosis)}</div>
                </div>
              </div>
            </div>
          ) : null}
        </>
      )}
    </div>
  );
};
