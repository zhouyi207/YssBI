import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { formatNum, SignificanceStars } from './RegressionShared';
import type { Coefficient } from '@/shared/types/report';
import {
  InfoStatsTable,
  infoStatsCellClass,
  infoStatsCellCompactClass,
  infoStatsCellRightClass,
  infoStatsHeadClass,
  infoStatsHeadCompactClass,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
} from './InfoStatsTable';

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
      <InfoStatsTable>
        <TableHeader>
          <TableRow className="border-0 hover:bg-transparent">
            <TableHead className={infoStatsHeadClass}>Variable</TableHead>
            {hasCategorical && <TableHead className={infoStatsHeadClass}>{catHeader}</TableHead>}
            <TableHead className={infoStatsHeadCompactClass}>Coef</TableHead>
            {showOddsRatio && (
              <TableHead className={`${infoStatsHeadCompactClass} cursor-help`}>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <span>Odds Ratio</span>
                  </TooltipTrigger>
                  <TooltipContent side="top" className="max-w-xs">
                    exp(β)。变量系数：x 每增加 1 单位，几率变为原来的 exp(β) 倍；常数项：当所有 x 为 0 时 y=1 的基准几率
                  </TooltipContent>
                </Tooltip>
              </TableHead>
            )}
            <TableHead className={infoStatsHeadCompactClass}>Std Err</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>{statLabel}</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>{pLabel}</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>[0.025</TableHead>
            <TableHead className={infoStatsHeadCompactClass}>0.975]</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {coefficients.map((coeff, idx) => {
            const oddsTooltip = showOddsRatio
              ? coeff.variable === 'const'
                ? '基准几率：当所有自变量为 0 时，y=1 的几率'
                : coeff.category != null
                  ? `${coeff.variable}=${coeff.category} 相对于参照组，几率变为原来的 ${formatNum(Math.exp(coeff.coef))} 倍`
                  : `${coeff.variable} 每增加 1 单位，几率变为原来的 ${formatNum(Math.exp(coeff.coef))} 倍`
              : null;
            return (
            <TableRow
              key={`${coeff.variable}-${coeff.category ?? ''}-${idx}`}
              className={idx % 2 === 0 ? infoStatsRowEvenClass : infoStatsRowOddClass}
            >
              <TableCell className={infoStatsCellClass}>
                <div className="flex items-center gap-2">
                  <div className={`h-1.5 w-1.5 rounded-full ${coeff.is_significant ? 'bg-emerald-400' : 'bg-muted-foreground/40'}`} />
                  <span className={`font-mono font-medium ${coeff.is_significant ? 'text-foreground' : 'text-muted-foreground'}`}>
                    {coeff.variable}
                  </span>
                </div>
              </TableCell>
              {hasCategorical && (
                <TableCell className={infoStatsCellCompactClass}>
                  {coeff.category != null ? (
                    <span className="inline-flex items-center rounded border border-indigo-500/25 bg-indigo-500/15 px-2 py-0.5 text-[11px] font-mono text-indigo-700 dark:text-indigo-300">
                      {coeff.category}
                    </span>
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </TableCell>
              )}
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{formatNum(coeff.coef)}</TableCell>
              {oddsTooltip != null && (
                <TableCell className={`${infoStatsCellRightClass} text-amber-700/90 dark:text-amber-300/90`}>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <span className="cursor-help">{formatNum(Math.exp(coeff.coef))}</span>
                    </TooltipTrigger>
                    <TooltipContent side="top" className="max-w-xs">
                      {oddsTooltip}
                    </TooltipContent>
                  </Tooltip>
                </TableCell>
              )}
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>
                {coeff.std_err != null ? formatNum(coeff.std_err) : '.'}
              </TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                {coeff.t_value != null ? formatNum(coeff.t_value, 3) : '.'}
              </TableCell>
              <TableCell className={infoStatsCellRightClass}>
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
              </TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>
                {coeff['confidence_interval_0.025'] != null ? formatNum(coeff['confidence_interval_0.025']) : '.'}
              </TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>
                {coeff['confidence_interval_0.975'] != null ? formatNum(coeff['confidence_interval_0.975']) : '.'}
              </TableCell>
            </TableRow>
            );
          })}
          {ar1Rho != null && (
            <TableRow className={infoStatsRowOddClass}>
              <TableCell className={infoStatsCellClass}>
                <div className="flex items-center gap-2">
                  <div className="h-1.5 w-1.5 rounded-full bg-amber-400/80" />
                  <span className="font-mono font-medium text-foreground">rho</span>
                </div>
              </TableCell>
              {hasCategorical && <TableCell className={`${infoStatsCellCompactClass} text-muted-foreground`}>—</TableCell>}
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{formatNum(ar1Rho)}</TableCell>
              {showOddsRatio && <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>—</TableCell>}
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>—</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>—</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>—</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>—</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-muted-foreground`}>—</TableCell>
            </TableRow>
          )}
        </TableBody>
      </InfoStatsTable>

      <div className="mt-2 flex items-center gap-4 px-1 text-[10px] text-muted-foreground">
        <span>
          Significance: <span className="text-yellow-400">***</span> p&lt;0.001, <span className="text-yellow-400">**</span> p&lt;0.01,{' '}
          <span className="text-yellow-400">*</span> p&lt;0.05, <span className="text-muted-foreground">.</span> p&lt;0.1
        </span>
      </div>
    </>
  );
}
