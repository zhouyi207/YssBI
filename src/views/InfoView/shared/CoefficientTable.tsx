import React from 'react';
import { formatNum, SignificanceStars } from './RegressionShared';
import type { Coefficient } from './types';

export function CoefficientTable({
  coefficients,
  hasCategorical,
  ar1Rho,
  useZStat,
  showOddsRatio,
  categoryLabel,
}: {
  coefficients: Coefficient[];
  hasCategorical: boolean;
  /** AR(1) 自相关参数 ρ，Prais 时传入，在表末追加一行 */
  ar1Rho?: number;
  /** IV:2SLS uses z (asymptotic normal), not t */
  useZStat?: boolean;
  /** Logit: show odds ratio exp(β) to the right of Coef */
  showOddsRatio?: boolean;
  /** Override "Category" column label (e.g. "Equation" for VAR) */
  categoryLabel?: string;
}) {
  const statLabel = useZStat ? 'z' : 't';
  const pLabel = useZStat ? 'P>|z|' : 'P>|t|';
  const catHeader = categoryLabel ?? 'Category';
  return (
    <>
      <div className="rounded-lg border border-gray-800/50 overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-[#1a1d23]">
              <th className="text-left px-4 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Variable</th>
              {hasCategorical && (
                <th className="text-left px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">{catHeader}</th>
              )}
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Coef</th>
              {showOddsRatio && (
                <th
                  className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider cursor-help"
                  title="exp(β)。变量系数：x 每增加 1 单位，几率变为原来的 exp(β) 倍；常数项：当所有 x 为 0 时 y=1 的基准几率"
                >
                  Odds Ratio
                </th>
              )}
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">Std Err</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">{statLabel}</th>
              <th className="text-right px-3 py-2.5 text-gray-500 font-medium uppercase tracking-wider">{pLabel}</th>
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
                {showOddsRatio && (
                  <td
                    className="text-right px-3 py-2.5 font-mono text-amber-300/90 cursor-help"
                    title={
                      coeff.variable === 'const'
                        ? '基准几率：当所有自变量为 0 时，y=1 的几率'
                        : coeff.category != null
                          ? `${coeff.variable}=${coeff.category} 相对于参照组，几率变为原来的 ${formatNum(Math.exp(coeff.coef))} 倍`
                          : `${coeff.variable} 每增加 1 单位，几率变为原来的 ${formatNum(Math.exp(coeff.coef))} 倍`
                    }
                  >
                    {formatNum(Math.exp(coeff.coef))}
                  </td>
                )}
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
            {ar1Rho != null && (
              <tr className="border-t border-gray-800/30 bg-[#15171d] hover:bg-[#1e2128]">
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-amber-400/80" />
                    <span className="font-mono font-medium text-white">rho</span>
                  </div>
                </td>
                {hasCategorical && <td className="px-3 py-2.5 text-gray-600">—</td>}
                <td className="text-right px-3 py-2.5 font-mono text-white">{formatNum(ar1Rho)}</td>
                {showOddsRatio && <td className="text-right px-3 py-2.5 font-mono text-gray-600">—</td>}
                <td className="text-right px-3 py-2.5 font-mono text-gray-600">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-600">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-600">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-600">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-gray-600">—</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center gap-4 mt-2 text-[10px] text-gray-600 px-1">
        <span>Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01, <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-gray-500">.</span> p&lt;0.1</span>
      </div>
    </>
  );
}
