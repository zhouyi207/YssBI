import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatNum } from './RegressionShared';
import {
  InfoStatsTable,
  infoStatsCellClass,
  infoStatsCellRightClass,
  infoStatsHeadClass,
  infoStatsHeadCompactClass,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
} from './InfoStatsTable';
import type { Iv2slsFirstStageResult, Iv2slsFirstStageSummary } from '@/shared/types/report';

export function IvFirstStageSummaryTables({
  summary,
  firstStage,
  variant,
}: {
  summary: Iv2slsFirstStageSummary;
  firstStage?: Iv2slsFirstStageResult[];
  variant: '2sls' | 'liml';
}) {
  const stockYogoLabel = variant === 'liml' ? 'Stock-Yogo (2005) LIML' : 'Stock-Yogo (2005)';

  return (
    <div className="mb-4 space-y-3">
      <div className="flex flex-wrap gap-x-6 gap-y-1 text-xs text-muted-foreground">
        <span>
          Included instruments: <span className="font-mono text-foreground">{summary.k_included_instruments}</span>
        </span>
        <span>
          Excluded instruments: <span className="font-mono text-foreground">{summary.k_excluded_instruments}</span>
        </span>
        <span>
          Endogenous regressors: <span className="font-mono text-foreground">{summary.k_endogenous_regressors}</span>
        </span>
      </div>

      {summary.r2 != null ? (
        <InfoStatsTable>
          <TableHeader>
            <TableRow className="border-0 hover:bg-transparent">
              <TableHead className={infoStatsHeadClass}>Variable</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>R-sq.</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Adj R-sq.</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Partial R-sq.</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>
                F({summary.f_df1 ?? 0},{summary.f_df2 ?? 0})
              </TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Prob &gt; F</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow className={infoStatsRowEvenClass}>
              <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>
                {firstStage?.[0]?.endog_name ?? '—'}
              </TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{summary.r2 != null ? formatNum(summary.r2, 4) : '—'}</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{summary.r2_adjusted != null ? formatNum(summary.r2_adjusted, 4) : '—'}</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>{summary.partial_r2 != null ? formatNum(summary.partial_r2, 4) : '—'}</TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                {summary.f_stat != null ? formatNum(summary.f_stat, 4) : '—'}
              </TableCell>
              <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                {summary.f_p_value != null ? formatNum(summary.f_p_value, 4) : '—'}
              </TableCell>
            </TableRow>
          </TableBody>
        </InfoStatsTable>
      ) : (
        <InfoStatsTable>
          <TableHeader>
            <TableRow className="border-0 hover:bg-transparent">
              <TableHead className={infoStatsHeadClass}>Variable</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Shea&apos;s partial R-sq.</TableHead>
              <TableHead className={infoStatsHeadCompactClass}>Shea&apos;s adj. partial R-sq.</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {firstStage?.map((fs, i) => (
              <TableRow key={fs.endog_name} className={i % 2 === 0 ? infoStatsRowEvenClass : infoStatsRowOddClass}>
                <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>{fs.endog_name}</TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                  {formatNum(summary.shea_partial_r2[i] ?? 0, 4)}
                </TableCell>
                <TableCell className={`${infoStatsCellRightClass} text-foreground`}>
                  {formatNum(summary.shea_adj_partial_r2[i] ?? 0, 4)}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </InfoStatsTable>
      )}

      {summary.min_eigenvalue_cv_note !== 'robust' && (
        <div className="overflow-hidden rounded-lg border border-border">
          <div className="flex items-center justify-between border-b border-border bg-muted px-4 py-2.5">
            <div>
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">Minimum eigenvalue statistic</span>
              <span className="ml-2 font-mono font-medium text-foreground">{formatNum(summary.min_eigenvalue, 4)}</span>
            </div>
            {summary.min_eigenvalue_cv && <span className="text-[10px] text-muted-foreground">{stockYogoLabel}</span>}
          </div>
          {summary.min_eigenvalue_cv && (
            <div className="overflow-x-auto">
              <Table className="w-full table-fixed text-xs">
                <colgroup>
                  <col className="w-[min(16rem,45%)]" />
                  <col className="w-[4.5rem]" />
                  <col className="w-[4.5rem]" />
                  <col className="w-[4.5rem]" />
                  <col className="w-[4.5rem]" />
                </colgroup>
                {variant === '2sls' ? (
                  <>
                    <TableHeader>
                      <TableRow className="bg-muted/40 hover:bg-transparent">
                        <TableHead className="h-auto px-4 py-2 text-left text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                          Test
                        </TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">5%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">10%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">20%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">30%</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      <TableRow className="border-t border-border bg-card">
                        <TableCell className="px-4 py-2 text-left text-[11px] text-muted-foreground">2SLS relative bias</TableCell>
                        {summary.min_eigenvalue_cv.bias ? (
                          <>
                            <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                              {formatNum(summary.min_eigenvalue_cv.bias.pct_5, 2)}
                            </TableCell>
                            <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                              {formatNum(summary.min_eigenvalue_cv.bias.pct_10, 2)}
                            </TableCell>
                            <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                              {formatNum(summary.min_eigenvalue_cv.bias.pct_20, 2)}
                            </TableCell>
                            <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                              {formatNum(summary.min_eigenvalue_cv.bias.pct_30, 2)}
                            </TableCell>
                          </>
                        ) : (
                          <TableCell colSpan={4} className="px-4 py-2 text-right italic text-muted-foreground">
                            (not available)
                          </TableCell>
                        )}
                      </TableRow>
                    </TableBody>
                    <TableHeader>
                      <TableRow className="border-t border-border bg-muted/40 hover:bg-transparent">
                        <TableHead className="h-auto px-4 py-2 text-left text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                          Test
                        </TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">10%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">15%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">20%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">25%</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      <TableRow className="border-t border-border bg-card">
                        <TableCell className="px-4 py-2 text-left text-[11px] text-muted-foreground">
                          2SLS size of nominal 5% Wald test
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_10, 2)}
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_15, 2)}
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_20, 2)}
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_25, 2)}
                        </TableCell>
                      </TableRow>
                    </TableBody>
                  </>
                ) : (
                  <>
                    <TableHeader>
                      <TableRow className="bg-muted/40 hover:bg-transparent">
                        <TableHead className="h-auto px-4 py-2 text-left text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
                          Test
                        </TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">10%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">15%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">20%</TableHead>
                        <TableHead className="h-auto px-4 py-2 text-right font-medium tabular-nums text-muted-foreground">25%</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      <TableRow className="border-t border-border bg-card">
                        <TableCell className="px-4 py-2 text-left text-[11px] text-muted-foreground">
                          LIML size of nominal 5% Wald test
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_10, 2)}
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_15, 2)}
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_20, 2)}
                        </TableCell>
                        <TableCell className="px-4 py-2 text-right font-mono tabular-nums text-foreground">
                          {formatNum(summary.min_eigenvalue_cv.size.pct_25, 2)}
                        </TableCell>
                      </TableRow>
                    </TableBody>
                  </>
                )}
              </Table>
            </div>
          )}
          {!summary.min_eigenvalue_cv && (
            <div className="bg-card px-4 py-2.5 text-[11px] text-muted-foreground">
              {summary.min_eigenvalue_cv_note === 'k_endog_gt_2'
                ? 'Stock-Yogo critical values not available for 3+ endogenous regressors'
                : 'Stock-Yogo critical values not shown'}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
