import type { FC } from 'react';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { ReportLayout, ReportSection, formatNum } from './shared';
import { InfoStatsTable } from './shared/InfoStatsTable';
import type { VecRankResultData } from '@/shared/types/report';

export type { VecRankResultData } from '@/shared/types/report';

function fmt(v: number | null | undefined, d = 4): string {
  if (v == null || Number.isNaN(v)) return '—';
  return formatNum(v, d);
}

export const VecRankComponent: FC<{ data: VecRankResultData }> = ({ data }) => {
  const {
    title,
    var_names,
    num_observation,
    n_lags,
    trend_spec,
    show_max_eigen,
    selected_rank_trace_95,
    selected_rank_trace_99,
    selected_rank_max_95,
    selected_rank_max_99,
    rows,
    note,
  } = data;

  const trendLabel =
    trend_spec === 'none'
      ? 'none'
      : trend_spec === 'constant'
        ? 'constant'
        : trend_spec === 'trend'
          ? 'trend'
          : trend_spec;

  return (
    <ReportLayout
      title={title}
      size="extraWide"
      subtitle={
        <p className="text-xs leading-relaxed text-muted-foreground">
          Variables: {var_names.join(', ')} · Trend: {trendLabel} · Number of obs = {num_observation} · Lags = {n_lags}
          <br />
          Trace @5% / 1% selected rank: {selected_rank_trace_95} / {selected_rank_trace_99} · Max eigen @5% / 1%:{' '}
          {selected_rank_max_95} / {selected_rank_max_99}
        </p>
      }
    >
      <ReportSection title="Johansen tests" icon="chart">
        <InfoStatsTable className="mb-6 overflow-x-auto" tableClassName="font-mono text-xs text-foreground">
          <TableHeader>
            <TableRow className="border-b border-border text-muted-foreground hover:bg-transparent">
              <TableHead className="h-auto px-2 py-2 text-left">rank</TableHead>
              <TableHead className="h-auto px-2 py-2 text-right">LL</TableHead>
              <TableHead className="h-auto px-2 py-2 text-right">Eigen</TableHead>
              <TableHead className="h-auto px-2 py-2 text-right">Trace</TableHead>
              <TableHead className="h-auto px-2 py-2 text-right">cv 10%</TableHead>
              <TableHead className="h-auto px-2 py-2 text-right">cv 5%</TableHead>
              <TableHead className="h-auto px-2 py-2 text-right">cv 1%</TableHead>
              {show_max_eigen && (
                <>
                  <TableHead className="h-auto border-l border-border px-2 py-2 text-right">λ_max</TableHead>
                  <TableHead className="h-auto px-2 py-2 text-right">cv 10%</TableHead>
                  <TableHead className="h-auto px-2 py-2 text-right">cv 5%</TableHead>
                  <TableHead className="h-auto px-2 py-2 text-right">cv 1%</TableHead>
                </>
              )}
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((r) => {
              const starTrace = r.rank === selected_rank_trace_95 && r.trace_statistic != null;
              const starMax = show_max_eigen && r.rank === selected_rank_max_95 && r.max_eigenvalue_statistic != null;
              return (
                <TableRow key={r.rank} className="border-b border-border hover:bg-muted/40">
                  <TableCell className="px-2 py-1.5 text-left text-muted-foreground">{r.rank}</TableCell>
                  <TableCell className="px-2 py-1.5 text-right">{fmt(r.log_likelihood)}</TableCell>
                  <TableCell className="px-2 py-1.5 text-right">{fmt(r.eigenvalue)}</TableCell>
                  <TableCell className="px-2 py-1.5 text-right">
                    {starTrace ? (
                      <span className="font-medium text-emerald-400">
                        {fmt(r.trace_statistic)} <span className="text-muted-foreground">*</span>
                      </span>
                    ) : (
                      fmt(r.trace_statistic)
                    )}
                  </TableCell>
                  <TableCell className="px-2 py-1.5 text-right text-muted-foreground">{fmt(r.trace_crit_10pct)}</TableCell>
                  <TableCell className="px-2 py-1.5 text-right text-muted-foreground">{fmt(r.trace_crit_5pct)}</TableCell>
                  <TableCell className="px-2 py-1.5 text-right text-muted-foreground">{fmt(r.trace_crit_1pct)}</TableCell>
                  {show_max_eigen && (
                    <>
                      <TableCell className="border-l border-border px-2 py-1.5 text-right">
                        {starMax ? (
                          <span className="font-medium text-emerald-400">
                            {fmt(r.max_eigenvalue_statistic)} <span className="text-muted-foreground">*</span>
                          </span>
                        ) : (
                          fmt(r.max_eigenvalue_statistic)
                        )}
                      </TableCell>
                      <TableCell className="px-2 py-1.5 text-right text-muted-foreground">{fmt(r.max_eigen_crit_10pct)}</TableCell>
                      <TableCell className="px-2 py-1.5 text-right text-muted-foreground">{fmt(r.max_eigen_crit_5pct)}</TableCell>
                      <TableCell className="px-2 py-1.5 text-right text-muted-foreground">{fmt(r.max_eigen_crit_1pct)}</TableCell>
                    </>
                  )}
                </TableRow>
              );
            })}
          </TableBody>
        </InfoStatsTable>
      </ReportSection>

      <p className="text-[11px] leading-relaxed text-muted-foreground">{note}</p>
      <p className="mt-2 text-[11px] text-muted-foreground">
        * next to trace (or max eigen) marks the row selected at 5% by Johansen’s sequential rule (Stata vecrank).
      </p>
    </ReportLayout>
  );
};
