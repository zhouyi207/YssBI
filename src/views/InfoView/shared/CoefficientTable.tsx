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
      <div className="rounded-lg border border-border overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="bg-muted">
              <th className="text-left px-4 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">Variable</th>
              {hasCategorical && (
                <th className="text-left px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">{catHeader}</th>
              )}
              <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">Coef</th>
              {showOddsRatio && (
                <th
                  className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider cursor-help"
                  title="exp(β)。变量系数：x 每增加 1 单位，几率变为原来的 exp(β) 倍；常数项：当所有 x 为 0 时 y=1 的基准几率"
                >
                  Odds Ratio
                </th>
              )}
              <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">Std Err</th>
              <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">{statLabel}</th>
              <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">{pLabel}</th>
              <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">[0.025</th>
              <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">0.975]</th>
            </tr>
          </thead>
          <tbody>
            {coefficients.map((coeff, idx) => (
              <tr
                key={`${coeff.variable}-${coeff.category ?? ''}-${idx}`}
                className={`
                  border-t border-border transition-colors hover:bg-muted
                  ${idx % 2 === 0 ? 'bg-card' : 'bg-muted/40'}
                `}
              >
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <div className={`w-1.5 h-1.5 rounded-full ${coeff.is_significant ? 'bg-emerald-400' : 'bg-muted-foreground/40'}`} />
                    <span className={`font-mono font-medium ${coeff.is_significant ? 'text-foreground' : 'text-muted-foreground'}`}>
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
                      <span className="text-muted-foreground">—</span>
                    )}
                  </td>
                )}
                <td className="text-right px-3 py-2.5 font-mono text-foreground">
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
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">
                  {coeff.std_err != null ? formatNum(coeff.std_err) : '.'}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-foreground">
                  {coeff.t_value != null ? formatNum(coeff.t_value, 3) : '.'}
                </td>
                <td className="text-right px-3 py-2.5 font-mono">
                  {coeff.p_value != null ? (
                    <>
                      <span className={coeff.is_significant ? 'text-emerald-400' : 'text-muted-foreground'}>
                        {formatNum(coeff.p_value, 3)}
                      </span>
                      <SignificanceStars pValue={coeff.p_value} />
                    </>
                  ) : (
                    <span className="text-muted-foreground">.</span>
                  )}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">
                  {coeff['confidence_interval_0.025'] != null ? formatNum(coeff['confidence_interval_0.025']) : '.'}
                </td>
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">
                  {coeff['confidence_interval_0.975'] != null ? formatNum(coeff['confidence_interval_0.975']) : '.'}
                </td>
              </tr>
            ))}
            {ar1Rho != null && (
              <tr className="border-t border-border bg-muted/40 hover:bg-muted">
                <td className="px-4 py-2.5">
                  <div className="flex items-center gap-2">
                    <div className="w-1.5 h-1.5 rounded-full bg-amber-400/80" />
                    <span className="font-mono font-medium text-foreground">rho</span>
                  </div>
                </td>
                {hasCategorical && <td className="px-3 py-2.5 text-muted-foreground">—</td>}
                <td className="text-right px-3 py-2.5 font-mono text-foreground">{formatNum(ar1Rho)}</td>
                {showOddsRatio && <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">—</td>}
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">—</td>
                <td className="text-right px-3 py-2.5 font-mono text-muted-foreground">—</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center gap-4 mt-2 text-[10px] text-muted-foreground px-1">
        <span>Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01, <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-muted-foreground">.</span> p&lt;0.1</span>
      </div>
    </>
  );
}
