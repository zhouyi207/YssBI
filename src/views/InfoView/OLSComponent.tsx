import React, { Suspense, useMemo, useState } from 'react';
import katex from 'katex';
import 'katex/dist/katex.min.css';
import { hypothesisTest } from '@/services/stats';
import type { HypothesisTestResponse } from '@/services/stats';

const FormulaBlock = React.lazy(() => import('./FormulaBlock'));
const ResidualPlot = React.lazy(() => import('./ResidualPlot'));

interface ModelBasicInfo {
  model_type: string;
  method: string;
  num_observation: number;
  r_squared: number;
  adj_r_squared: number;
  f_statistic: number;
  prob_f_statistic: number;
  df_model: number;
  df_residual: number;
  df_total: number;
  ss_model: number;
  ss_residual: number;
  ss_total: number;
  ms_model: number;
  ms_residual: number;
  ms_total: number;
  covariance_type: string;
  aic?: number;
  bic?: number;
}

export interface Coefficient {
  variable: string;
  category?: string;
  coef: number;
  std_err: number;
  t_value: number;
  p_value: number;
  'confidence_interval_0.025': number;
  'confidence_interval_0.975': number;
  is_significant: boolean;
}

interface BreuschPaganTest {
  lm_stat: number;
  df: number;
  p_value: number;
}

/** 四种 BP 变体（对应 Stata estat hettest） */
interface BreuschPaganTests {
  stata?: BreuschPaganTest;
  koenker?: BreuschPaganTest;
  stata_rhs?: BreuschPaganTest;
  koenker_rhs?: BreuschPaganTest;
}

/** IM-test 各分量的 chi² 检验结果 */
interface ImTestComponent {
  chi2: number;
  df: number;
  p_value: number;
}

/** Cameron & Trivedi (1990) IM-test 分解（estat imtest） */
interface ImTest {
  heteroskedasticity: ImTestComponent;
  skewness: ImTestComponent;
  kurtosis: ImTestComponent;
  total: ImTestComponent;
}

/** 各诊断模块的后端计算耗时（毫秒） */
interface DiagnosticTiming {
  fitted_residuals_ms?: number;
  bp_tests_ms?: number;
  im_test_ms?: number;
}

interface DiagnosticInfo {
  cond_no: number;
  bp_tests?: BreuschPaganTests;
  im_test?: ImTest;
  fitted_values?: number[];
  residuals?: number[];
  timing?: DiagnosticTiming;
}

export interface OLSResultData {
  title: string;
  endog_name?: string;
  model_basic_info: ModelBasicInfo;
  coefficients: Coefficient[];
  diagnostic_info: DiagnosticInfo;
  /** 参数估计，用于假设检验 */
  betas?: number[];
  /** 参数协方差矩阵 (k×k)，用于假设检验 */
  cov_beta?: number[][];
  /** 后端计算耗时（毫秒），用于性能分析 */
  executionTimeMs?: number;
}

function formatNum(value: number, decimals = 4): string {
  if (Math.abs(value) < 0.0001 && value !== 0) {
    return value.toExponential(3);
  }
  return value.toFixed(decimals);
}

function SignificanceStars({ pValue }: { pValue: number }) {
  if (pValue < 0.001) return <span className="text-yellow-400 font-bold ml-1">***</span>;
  if (pValue < 0.01) return <span className="text-yellow-400 font-bold ml-1">**</span>;
  if (pValue < 0.05) return <span className="text-yellow-400 font-bold ml-1">*</span>;
  if (pValue < 0.1) return <span className="text-gray-500 ml-1">.</span>;
  return null;
}

function RSquaredBadge({ value }: { value: number }) {
  let color = 'bg-red-500/20 text-red-400 border-red-500/30';
  if (value >= 0.7) color = 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
  else if (value >= 0.4) color = 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30';

  return (
    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-semibold border ${color}`}>
      R² = {value.toFixed(3)}
    </span>
  );
}

function StatCard({ label, value, sub }: { label: string; value: string | number; sub?: string }) {
  return (
    <div className="bg-[#1a1d23] rounded-lg px-4 py-3 border border-gray-800/50">
      <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-1">{label}</div>
      <div className="text-white font-mono text-sm font-medium">{value}</div>
      {sub && <div className="text-[10px] text-gray-600 mt-0.5">{sub}</div>}
    </div>
  );
}

const BP_VARIANTS: { key: keyof BreuschPaganTests; label: string }[] = [
  { key: 'stata', label: 'estat hettest' },
  { key: 'koenker', label: 'estat hettest, iid' },
  { key: 'stata_rhs', label: 'estat hettest, rhs' },
  { key: 'koenker_rhs', label: 'estat hettest, rhs iid' },
];

/** 统一的 chi² 检验卡片（BP 四种变体 / IM-test 分解） */
interface Chi2TestCard {
  label: string;
  chi2: number;
  df: number;
  p_value: number;
}

function Chi2TestCards({ cards }: { cards: Chi2TestCard[] }) {
  return (
    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
      {cards.map((c) => {
        const reject = c.p_value < 0.05;
        return (
          <div
            key={c.label}
            className="rounded-lg border border-gray-800/50 bg-[#1a1d23] px-4 py-3 hover:border-gray-700/50 transition-colors"
          >
            <div className="text-[11px] text-gray-500 font-mono mb-2">{c.label}</div>
            <div className="flex flex-wrap items-baseline gap-x-4 gap-y-1 text-xs">
              <span className="text-gray-400">
                chi2 = <span className="font-mono text-white">{formatNum(c.chi2)}</span>
              </span>
              <span className="text-gray-400">
                df = <span className="font-mono text-gray-300">{c.df}</span>
              </span>
              <span className="text-gray-400">
                p = <span className={`font-mono ${reject ? 'text-emerald-400' : 'text-gray-400'}`}>{formatNum(c.p_value)}</span>
              </span>
            </div>
            <div className="mt-1.5 text-[10px]">
              {reject ? (
                <span className="text-amber-400">拒绝 H0</span>
              ) : (
                <span className="text-gray-500">不拒绝 H0</span>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

function SectionHeader({ title, icon }: { title: string; icon: React.ReactNode }) {
  return (
    <div className="flex items-center gap-2 mb-3 mt-6 first:mt-0">
      <div className="text-[var(--accent-color)]">{icon}</div>
      <h3 className="text-sm font-semibold text-gray-300 uppercase tracking-wider">{title}</h3>
      <div className="flex-1 h-px bg-gray-800 ml-2"></div>
    </div>
  );
}

function InfoRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
      <span className="text-gray-500 text-xs">{label}</span>
      <span className="text-white text-xs font-mono font-medium">{children}</span>
    </div>
  );
}

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

      {/* Equation Section */}
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

      {/* Model Info Section */}
      <SectionHeader
        title="Model Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
        }
      />

      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50 mb-2">
        <InfoRow label="Model">{info.model_type}</InfoRow>
        <InfoRow label="Method">{info.method}</InfoRow>
        <InfoRow label="R-squared">{info.r_squared.toFixed(4)}</InfoRow>
        <InfoRow label="Adj. R-squared">{info.adj_r_squared.toFixed(4)}</InfoRow>
        <InfoRow label="F-statistic">{formatNum(info.f_statistic)}</InfoRow>
        <InfoRow label="Prob (F-statistic)">
          <span className={info.prob_f_statistic < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>
            {formatNum(info.prob_f_statistic)}
          </span>
        </InfoRow>
        <InfoRow label="No. Observations">{info.num_observation}</InfoRow>
        <InfoRow label="Covariance Type">{info.covariance_type}</InfoRow>
        <InfoRow label="Df Model">{info.df_model}</InfoRow>
        <InfoRow label="Df Residual">{info.df_residual}</InfoRow>
        {info.aic != null && <InfoRow label="AIC">{formatNum(info.aic)}</InfoRow>}
        {info.bic != null && <InfoRow label="BIC">{formatNum(info.bic)}</InfoRow>}
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2">
          <span className="text-gray-500 text-xs">Df Total</span>
          <span className="text-white text-xs font-mono font-medium">{info.df_total}</span>
        </div>
        {data.executionTimeMs != null && (
          <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2 border-t border-gray-800/30">
            <span className="text-gray-500 text-xs">后端计算耗时</span>
            <span className="text-[var(--accent-color)] text-xs font-mono font-medium">{data.executionTimeMs} ms</span>
          </div>
        )}
      </div>

      {/* Sum of Squares & Mean Squares */}
      <SectionHeader
        title="ANOVA"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
          </svg>
        }
      />

      <div className="rounded-lg border border-gray-800/50 overflow-hidden mb-2">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-[#1a1d23]">
              <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Source</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">SS</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">df</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">MS</th>
            </tr>
          </thead>
          <tbody>
            <tr className="bg-[#13151a] border-t border-gray-800/30">
              <td className="px-4 py-2.5 font-mono text-white">Model</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ss_model)}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{info.df_model}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ms_model)}</td>
            </tr>
            <tr className="bg-[#15171d] border-t border-gray-800/30">
              <td className="px-4 py-2.5 font-mono text-white">Residual</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ss_residual)}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{info.df_residual}</td>
              <td className="text-right px-3 py-2.5 font-mono text-gray-300">{formatNum(info.ms_residual)}</td>
            </tr>
            <tr className="bg-[#13151a] border-t border-gray-800/30">
              <td className="px-4 py-2.5 font-mono text-white font-semibold">Total</td>
              <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{formatNum(info.ss_total)}</td>
              <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{info.df_total}</td>
              <td className="text-right px-3 py-2.5 font-mono text-white font-semibold">{formatNum(info.ms_total)}</td>
            </tr>
          </tbody>
        </table>
      </div>

      {/* Coefficients Section */}
      <SectionHeader
        title={`Coefficients (${significantCount}/${coefficients.length} significant)`}
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 7h16M4 12h10M4 17h6" />
          </svg>
        }
      />

      <div className="rounded-lg border border-gray-800/50 overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-[#1a1d23]">
              <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Variable</th>
              {hasCategorical && (
                <th className="text-left px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Category</th>
              )}
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Coef</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Std Err</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">t</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">P&gt;|t|</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">[0.025</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">0.975]</th>
            </tr>
          </thead>
          <tbody>
            {coefficients.map((coeff, idx) => (
              <tr
                key={`${coeff.variable}-${coeff.category ?? idx}`}
                className={`
                  border-t border-gray-800/30 transition-colors hover:bg-[#1e2128]
                  ${idx % 2 === 0 ? 'bg-[#13151a]' : 'bg-[#15171d]'}
                `}
              >
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <div className={`w-1.5 h-1.5 rounded-full ${coeff.is_significant ? 'bg-emerald-400' : 'bg-gray-600'}`} />
                    <span className={`font-mono font-medium ${coeff.is_significant ? 'text-white' : 'text-gray-400'}`}>
                      {coeff.variable}
                    </span>
                  </div>
                </td>
                {hasCategorical && (
                  <td className="px-3 py-2.5">
                    {coeff.category != null ? (
                      <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-mono bg-indigo-500/15 text-indigo-300 border border-indigo-500/25">
                        {coeff.category}
                      </span>
                    ) : (
                      <span className="text-gray-600">—</span>
                    )}
                  </td>
                )}
                <td className="text-right px-3 py-2.5 font-mono text-white">
                  {formatNum(coeff.coef)}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-400">
                  {formatNum(coeff.std_err)}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-300">
                  {formatNum(coeff.t_value, 3)}
                </td>
                <td className="text-right px-3 py-2.5 font-mono">
                  <span className={coeff.is_significant ? 'text-emerald-400' : 'text-gray-500'}>
                    {formatNum(coeff.p_value, 3)}
                  </span>
                  <SignificanceStars pValue={coeff.p_value} />
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-500">
                  {formatNum(coeff['confidence_interval_0.025'])}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-500">
                  {formatNum(coeff['confidence_interval_0.975'])}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <div className="flex items-center gap-4 mt-2 text-[10px] text-gray-600 px-1">
        <span>Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01, <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-gray-500">.</span> p&lt;0.1</span>
      </div>

      {/* Hypothesis Test */}
      <HypothesisTestBlock data={data} />

      {/* Coefficient Bar Visualization */}
      <SectionHeader
        title="Coefficient Magnitude"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 8v8m-4-5v5m-4-2v2m-2 4h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
          </svg>
        }
      />
      <CoeffBarChart coefficients={coefficients} />

      {/* Diagnostic Section */}
      <SectionHeader
        title="Diagnostics"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        }
      />

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-4">
        <StatCard
          label="Condition Number"
          value={formatNum(diag.cond_no)}
          sub={diag.cond_no > 1000 ? 'Possible multicollinearity' : 'Acceptable'}
        />
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
        </>
      )}
    </div>
  );
};

/** 从系数表构建 param_names（与 OLS exog 列序一致） */
function buildParamNames(coefficients: Coefficient[]): string[] {
  return coefficients.map((c) =>
    c.category != null ? `${c.variable}_${c.category}` : c.variable
  );
}

/** 将线性形式字符串转为 LaTeX（变量名 → β 系数） */
function linearFormToLatex(form: string, paramNames: string[]): string {
  const sorted = [...paramNames].sort((a, b) => b.length - a.length);
  let latex = form;
  for (const p of sorted) {
    const escaped = p.replace(/_/g, '\\_');
    const replacement = `\\beta_{\\text{${escaped}}}`;
    const regex = new RegExp(`\\b${p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'g');
    latex = latex.replace(regex, replacement);
  }
  latex = latex.replace(/\*/g, ' \\cdot ');
  latex = latex.replace(/ ≠ /g, ' \\neq ');
  latex = latex.replace(/ ≤ /g, ' \\leq ');
  latex = latex.replace(/ ≥ /g, ' \\geq ');
  return latex;
}

function renderHypothesisLatex(latex: string): string | null {
  try {
    return katex.renderToString(latex, { displayMode: true, throwOnError: false });
  } catch {
    return null;
  }
}

/** 按约束拆分并渲染，每个约束一行 */
function HypothesisFormulas({
  form,
  paramNames,
  className = '',
}: {
  form: string;
  paramNames: string[];
  className?: string;
}) {
  const parts = form.split(' ; ').map((s) => s.trim()).filter(Boolean);
  return (
    <div className={`flex flex-col gap-2 ${className}`}>
      {parts.map((part, i) => {
        const html = renderHypothesisLatex(linearFormToLatex(part, paramNames));
        return (
          <div
            key={i}
            className="[&_.katex]:text-gray-200 [&_.katex]:text-xs [&_.katex]:block"
            dangerouslySetInnerHTML={{ __html: html ?? part }}
          />
        );
      })}
    </div>
  );
}

function HypothesisTestBlock({ data }: { data: OLSResultData }) {
  const [hypothesis, setHypothesis] = useState('');
  const [result, setResult] = useState<HypothesisTestResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const paramNames = useMemo(() => buildParamNames(data.coefficients), [data.coefficients]);
  const canRun =
    data.betas != null &&
    data.cov_beta != null &&
    data.model_basic_info.df_residual != null &&
    hypothesis.trim().length > 0;

  const handleRun = async () => {
    if (!canRun || !data.betas || !data.cov_beta) return;
    setError(null);
    setResult(null);
    setLoading(true);
    try {
      const res = await hypothesisTest({
        betas: data.betas,
        cov_beta: data.cov_beta,
        df_residual: data.model_basic_info.df_residual,
        param_names: paramNames,
        hypothesis: hypothesis.trim(),
      });
      setResult(res);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="mt-6">
      <SectionHeader
        title="Hypothesis Test (t / Wald)"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 7h6m0 10v-3m-3 3h.01M9 17h.01M9 14h.01M12 14h.01M15 11h.01M12 11h.01M9 11h.01M7 21h10a2 2 0 002-2V5a2 2 0 00-2-2H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
          </svg>
        }
      />
      <div className="rounded-lg border border-gray-800/50 bg-[#13151a] p-4 space-y-3">
        <div className="flex gap-2">
          <input
            type="text"
            value={hypothesis}
            onChange={(e) => setHypothesis(e.target.value)}
            placeholder="e.g. x1 = 0 或 petal_width = -0.5626, petal_length = 0.7（逗号分隔多约束）"
            className="flex-1 px-3 py-2 rounded-md bg-[#1a1d23] border border-gray-700/50 text-sm font-mono text-white placeholder-gray-500 focus:outline-none focus:border-[var(--accent-color)]/50"
            onKeyDown={(e) => e.key === 'Enter' && handleRun()}
          />
          <button
            onClick={handleRun}
            disabled={!canRun || loading}
            className="px-4 py-2 rounded-md bg-[var(--accent-color)]/20 text-[var(--accent-color)] border border-[var(--accent-color)]/40 hover:bg-[var(--accent-color)]/30 disabled:opacity-50 disabled:cursor-not-allowed text-sm font-medium transition-colors"
          >
            {loading ? '...' : 'Run'}
          </button>
        </div>
        <div className="text-[10px] text-gray-500">
          Param names: {paramNames.join(', ')}
        </div>
        {error && (
          <div className="text-xs text-red-400 font-mono">{error}</div>
        )}
        {result && (
          <div className="rounded-md bg-[#1a1d23] border border-gray-800/50 overflow-hidden">
            {/* H0 | H1 公式区域，每个约束一行 */}
            <div className="grid grid-cols-2 divide-x divide-gray-800/50">
              <div className="p-4 min-w-0">
                <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-2">H₀ 原假设</div>
                <HypothesisFormulas form={result.h0_form} paramNames={paramNames} />
              </div>
              <div className="p-4 min-w-0">
                <div className="text-[10px] text-gray-500 uppercase tracking-wider mb-2">H₁ 备择假设</div>
                <HypothesisFormulas form={result.h1_form} paramNames={paramNames} />
              </div>
            </div>
            {/* 检验信息 */}
            <div className="border-t border-gray-800/50 px-4 py-3 space-y-1.5 text-xs">
              <div className="flex justify-between">
                <span className="text-gray-500">
                  {result.test_type === "t" ? "t-statistic" : "F-statistic"}
                </span>
                <span className="font-mono text-white">{formatNum(result.stat, 4)}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500">df</span>
                <span className="font-mono text-gray-400">{result.df1}, {result.df2}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-gray-500">p-value</span>
                <span className={`font-mono font-medium ${result.p_value < 0.05 ? 'text-emerald-400' : 'text-gray-400'}`}>
                  {formatNum(result.p_value, 4)}
                </span>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function CoeffBarChart({ coefficients }: { coefficients: Coefficient[] }) {
  const maxAbs = Math.max(...coefficients.map((c) => Math.abs(c.coef)), 0.001);

  return (
    <div className="rounded-lg border border-gray-800/50 bg-[#13151a] p-4 space-y-2">
      {coefficients.map((coeff, idx) => {
        const pct = (Math.abs(coeff.coef) / maxAbs) * 100;
        const isPositive = coeff.coef >= 0;
        const label = coeff.category != null
          ? `${coeff.variable}[${coeff.category}]`
          : coeff.variable;

        return (
          <div key={`${coeff.variable}-${coeff.category ?? idx}`} className="flex items-center gap-3">
            <span className="text-xs font-mono text-gray-400 w-28 text-right shrink-0 truncate" title={label}>
              {label}
            </span>
            <div className="flex-1 flex items-center h-5">
              <div className="w-1/2 flex justify-end">
                {!isPositive && (
                  <div
                    className={`h-4 rounded-l transition-all ${coeff.is_significant ? 'bg-rose-500/70' : 'bg-rose-500/25'}`}
                    style={{ width: `${pct}%`, minWidth: pct > 0 ? '2px' : '0' }}
                  />
                )}
              </div>
              <div className="w-px h-5 bg-gray-700 shrink-0" />
              <div className="w-1/2 flex justify-start">
                {isPositive && (
                  <div
                    className={`h-4 rounded-r transition-all ${coeff.is_significant ? 'bg-emerald-500/70' : 'bg-emerald-500/25'}`}
                    style={{ width: `${pct}%`, minWidth: pct > 0 ? '2px' : '0' }}
                  />
                )}
              </div>
            </div>
            <span className="text-[10px] font-mono text-gray-500 w-20 text-left shrink-0">
              {formatNum(coeff.coef)}
            </span>
          </div>
        );
      })}
    </div>
  );
}
