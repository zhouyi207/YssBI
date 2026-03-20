import React, { Suspense, useMemo } from 'react';
import {
  SectionHeader,
  formatNum,
  CoefficientsBlock,
  VARStableChart,
} from './shared';
import type { Coefficient, VARSummaryResultData } from './shared/types';

const VARFormulaBlock = React.lazy(() => import('./VARFormulaBlock'));

export type { VARSummaryResultData } from './shared/types';

function varCoeffsToOLSFormat(coefficients: VARSummaryResultData['coefficients']): Coefficient[] {
  const eqOrder = [...new Set(coefficients.map((x) => x.eq_name))];
  const mapped = coefficients.map((c, idx) => ({
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
  }));
  mapped.sort((a, b) => {
    if (a._eqOrder !== b._eqOrder) return a._eqOrder - b._eqOrder;
    if (a._sortKey !== b._sortKey) return a._sortKey - b._sortKey;
    return a._idx - b._idx;
  });
  return mapped.map(({ _sortKey, _eqOrder, _idx, ...rest }) => rest as Coefficient);
}

export const VARComponent: React.FC<{ data: VARSummaryResultData }> = ({ data }) => {
  const {
    var_names,
    num_observation,
    complete_sample_rows,
    var_max_lag,
    log_likelihood,
    aic,
    fpe,
    hqic,
    sbic,
    equations,
    coefficients,
    oirf,
    fevd,
    varwle,
    varlmar,
    varstable,
    vargranger,
  } = data;

  const coeffsForTable = useMemo(() => varCoeffsToOLSFormat(coefficients), [coefficients]);
  const varstableSorted = useMemo(
    () => (varstable ? [...varstable].sort((a, b) => b.modulus - a.modulus) : []),
    [varstable]
  );

  return (
    <div className="p-6 max-w-[900px] mx-auto">
      {/* Title */}
      <div className="mb-6">
        <h1 className="text-xl font-bold text-white mb-2">{data.title}</h1>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-xs text-gray-500 leading-relaxed">
            Variables: {var_names.join(', ')}
            {complete_sample_rows != null && var_max_lag != null ? (
              <>
                {' '}
                · T={complete_sample_rows}（时间轴对齐行数）· p={var_max_lag} · n={num_observation}
                （Stata Number of obs；仅内生 listwise 时 n = T − p；有外生 DataFrame 时与 Stata var ex() 相同，仅当期
                exog[t] 须有效）
              </>
            ) : (
              <> · n={num_observation}</>
            )}
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
        <VARFormulaBlock varNames={var_names} coefficients={coefficients} />
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
      <div className="grid grid-cols-2 gap-px bg-gray-800/50 rounded-lg overflow-hidden border border-gray-800/50 mb-6">
        {complete_sample_rows != null && var_max_lag != null && (
          <div className="bg-[#13151a] px-4 py-2.5 flex justify-between col-span-2 border-b border-gray-800/40">
            <span className="text-gray-500 text-xs shrink-0">Observations</span>
            <span className="text-white text-xs font-mono text-right">
              T = {complete_sample_rows}, p = {var_max_lag}, n = {num_observation}{' '}
              <span className="text-gray-500 font-sans">
                （无缺失外生时 n = T − p；首期外生缺失不减少 n）
              </span>
            </span>
          </div>
        )}
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
          <span className="text-gray-500 text-xs">Log likelihood</span>
          <span className="text-white text-xs font-mono font-medium">{formatNum(log_likelihood)}</span>
        </div>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
          <span className="text-gray-500 text-xs">AIC</span>
          <span className="text-white text-xs font-mono font-medium">{formatNum(aic)}</span>
        </div>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
          <span className="text-gray-500 text-xs">FPE</span>
          <span className="text-white text-xs font-mono font-medium">{formatNum(fpe)}</span>
        </div>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
          <span className="text-gray-500 text-xs">HQIC</span>
          <span className="text-white text-xs font-mono font-medium">{formatNum(hqic)}</span>
        </div>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
          <span className="text-gray-500 text-xs">SBIC</span>
          <span className="text-white text-xs font-mono font-medium">{formatNum(sbic)}</span>
        </div>
        <div className="bg-[#13151a] px-4 py-2.5 flex justify-between">
          <span className="text-gray-500 text-xs">Det(Sigma_ml)</span>
          <span className="text-white text-xs font-mono font-medium">{formatNum(data.det_sigma_ml)}</span>
        </div>
      </div>

      {/* Equation Summary */}
      <SectionHeader
        title="Equation Summary"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h18M3 14h18M3 6h18M3 18h18" />
          </svg>
        }
      />
      <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden mb-6">
        <table className="w-full text-left text-sm">
          <thead>
            <tr className="border-b border-gray-800/50">
              <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Equation</th>
              <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Parms</th>
              <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">RMSE</th>
              <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">R-sq</th>
              <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">chi2</th>
              <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">P&gt;chi2</th>
            </tr>
          </thead>
          <tbody>
            {equations.map((eq, i) => (
              <tr key={i} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                <td className="px-4 py-2.5 font-mono text-white">{eq.eq_name}</td>
                <td className="px-4 py-2.5 font-mono text-gray-300">{eq.parms}</td>
                <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(eq.rmse)}</td>
                <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(eq.r_sq)}</td>
                <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(eq.chi2)}</td>
                <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(eq.p_chi2)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Coefficients */}
      <CoefficientsBlock
        coefficients={coeffsForTable}
        hasCategorical={true}
        useZStat={true}
        categoryLabel="Equation"
      />

      {/* Wald lag-exclusion (varwle) */}
      {varwle && varwle.length > 0 && (() => {
        const byEq = varwle.reduce<Record<string, typeof varwle>>((acc, row) => {
          (acc[row.eq_name] ??= []).push(row);
          return acc;
        }, {});
        const eqOrder = [...new Set(varwle.map((r) => r.eq_name))];
        return (
          <>
            <SectionHeader
              title="Wald lag-exclusion statistics (varwle)"
              icon={
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-6 9l2 2 4-4" />
                </svg>
              }
            />
            <div className="space-y-4 mb-6">
              {eqOrder.map((eqName) => {
                const rows = byEq[eqName] ?? [];
                return (
                  <div key={eqName} className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden">
                    <div className="px-4 py-2.5 text-sm font-medium text-gray-300 border-b border-gray-800/50">
                      Equation: {eqName}
                    </div>
                    <table className="w-full text-left text-sm">
                      <thead>
                        <tr className="border-b border-gray-800/50">
                          <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">lag</th>
                          <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">chi2</th>
                          <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">df</th>
                          <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Prob &gt; chi2</th>
                        </tr>
                      </thead>
                      <tbody>
                        {rows.map((row, i) => (
                          <tr key={i} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                            <td className="px-4 py-2.5 font-mono text-white">{row.lag}</td>
                            <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(row.chi2)}</td>
                            <td className="px-4 py-2.5 font-mono text-gray-300">{row.df}</td>
                            <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(row.p_value)}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                );
              })}
            </div>
          </>
        );
      })()}

      {/* Eigenvalue stability condition (varstable, graph) */}
      {varstableSorted.length > 0 && (
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
            <div className="flex flex-col h-full rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden">
              <div className="flex-1 min-h-0 flex flex-col">
                <table className="text-left text-sm min-w-[200px]">
                  <thead>
                    <tr className="border-b border-gray-800/50">
                      <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Eigenvalue</th>
                      <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Modulus</th>
                    </tr>
                  </thead>
                  <tbody>
                    {varstableSorted.map((row, i) => (
                      <tr key={i} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                        <td className="px-4 py-2.5 font-mono text-gray-300">
                          {row.im >= 0
                            ? `${formatNum(row.re)} + ${formatNum(row.im)}i`
                            : `${formatNum(row.re)} - ${formatNum(Math.abs(row.im))}i`}
                        </td>
                        <td className="px-4 py-2.5 font-mono text-white">{formatNum(row.modulus)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <div className="flex-1 min-h-0 bg-[#1a1d23]" />
              </div>
              <div className="px-4 py-2 text-[11px] text-gray-500 border-t border-gray-800/30 shrink-0">
                {varstableSorted.some((r) => r.modulus >= 1)
                  ? 'At least one eigenvalue is at least 1.0. VAR does not satisfy stability condition.'
                  : 'All the eigenvalues lie inside the unit circle.'}
              </div>
            </div>
            <div className="min-w-[240px] min-h-0 flex">
              <VARStableChart data={varstableSorted} />
            </div>
          </div>
        </>
      )}

      {/* Granger causality Wald tests (vargranger) */}
      {vargranger && vargranger.length > 0 && (
        <>
          <SectionHeader
            title="Granger causality Wald tests"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
              </svg>
            }
          />
          <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden mb-6">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-gray-800/50">
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Equation</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Excluded</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">chi2</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">df</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Prob &gt; chi2</th>
                </tr>
              </thead>
              <tbody>
                {vargranger.map((row, i) => (
                  <tr
                    key={i}
                    className={`border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20 ${row.excluded === 'ALL' ? 'border-b-2 border-gray-700/50' : ''}`}
                  >
                    <td className="px-4 py-2.5 font-mono text-white">{row.eq_name}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{row.excluded}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(row.chi2)}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{row.df}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(row.p_value)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {/* LM test for residual autocorrelation (varlmar) */}
      {varlmar && varlmar.length > 0 && (
        <>
          <SectionHeader
            title="Lagrange-multiplier test (varlmar)"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
            }
          />
          <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-hidden mb-6">
            <table className="w-full text-left text-sm">
              <thead>
                <tr className="border-b border-gray-800/50">
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">lag</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">chi2</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">df</th>
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Prob &gt; chi2</th>
                </tr>
              </thead>
              <tbody>
                {varlmar.map((row, i) => (
                  <tr key={i} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                    <td className="px-4 py-2.5 font-mono text-white">{row.lag}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(row.chi2)}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{row.df}</td>
                    <td className="px-4 py-2.5 font-mono text-gray-300">{formatNum(row.p_value)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="px-4 py-2 text-[11px] text-gray-500 border-t border-gray-800/30">
              H0: no autocorrelation at lag order
            </div>
          </div>
        </>
      )}

      {/* Orthogonalized IRF */}
      {oirf && oirf.length > 0 && var_names && var_names.length > 0 && (
        <>
          <SectionHeader
            title="Orthogonalized IRF"
            icon={
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 12l3-3 3 3 4-4M8 21l4-4 4 4M3 4h18M4 4h16v12a1 1 0 01-1 1H5a1 1 0 01-1-1V4z" />
              </svg>
            }
          />
          <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-x-auto mb-6">
            <table className="w-full text-left text-sm min-w-[400px]">
              <thead>
                <tr className="border-b border-gray-800/50">
                  <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">Step</th>
                  {var_names.flatMap((imp) =>
                    var_names.map((resp) => (
                      <th key={`${imp}-${resp}`} className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">
                        {imp}→{resp}
                      </th>
                    ))
                  )}
                </tr>
              </thead>
              <tbody>
                {oirf.map((stepData, s) => (
                  <tr key={s} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                    <td className="px-4 py-2.5 font-mono text-white">{s}</td>
                    {var_names.flatMap((_, impIdx) =>
                      var_names.map((_, respIdx) => (
                        <td key={`${impIdx}-${respIdx}`} className="px-4 py-2.5 font-mono text-gray-300">
                          {formatNum(stepData[respIdx]?.[impIdx] ?? 0)}
                        </td>
                      ))
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {/* FEVD */}
      {fevd && fevd.length > 0 && var_names && var_names.length > 0 && (
        <>
        <SectionHeader
          title="Forecast-error variance decomposition"
          icon={
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
            </svg>
          }
        />
        <div className="rounded-lg border border-gray-800/50 bg-[#1a1d23] overflow-x-auto mb-6">
          <table className="w-full text-left text-sm min-w-[400px]">
            <thead>
              <tr className="border-b border-gray-800/50">
                <th className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">step</th>
                {var_names.flatMap((imp) =>
                  var_names.map((resp) => (
                    <th key={`${imp}-${resp}`} className="px-4 py-2.5 text-[11px] text-gray-500 uppercase tracking-wider font-medium">
                      {imp}→{resp}
                    </th>
                  ))
                )}
              </tr>
            </thead>
            <tbody>
              {fevd.map((stepData, s) => (
                <tr key={s} className="border-b border-gray-800/30 last:border-b-0 hover:bg-gray-800/20">
                  <td className="px-4 py-2.5 font-mono text-white">{s}</td>
                  {var_names.flatMap((_, impIdx) =>
                    var_names.map((_, respIdx) => (
                      <td key={`${impIdx}-${respIdx}`} className="px-4 py-2.5 font-mono text-gray-300">
                        {formatNum(stepData[respIdx]?.[impIdx] ?? 0)}
                      </td>
                    ))
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        </>
      )}
    </div>
  );
};
