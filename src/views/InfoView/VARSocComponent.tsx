import type { FC } from 'react';
import { useMemo } from 'react';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { ReportLayout, ReportSection, formatNum } from './shared';
import { InfoStatsTable, infoStatsRowEvenClass, infoStatsRowOddClass } from './shared/InfoStatsTable';
import type { VARSocResultData } from '@/shared/types/report';

function fmtCell(v: number | null | undefined, decimals = 4): string {
  if (v == null || Number.isNaN(v)) return '—';
  return formatNum(v, decimals);
}

function EmphasisNumber({ valueText, align = 'left' }: { valueText: string; align?: 'left' | 'right' }) {
  const flex = align === 'right' ? 'flex items-center justify-end gap-2' : 'flex items-center gap-2';
  return (
    <div className={flex}>
      <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-400" aria-hidden />
      <span className="font-medium text-foreground">{valueText}</span>
    </div>
  );
}

function extremumRowIndices(nums: (number | null | undefined)[], mode: 'min' | 'max'): Set<number> {
  const pairs: { v: number; i: number }[] = [];
  nums.forEach((v, i) => {
    if (v != null && !Number.isNaN(v)) pairs.push({ v, i });
  });
  if (pairs.length === 0) return new Set();
  const m = mode === 'max' ? Math.max(...pairs.map((p) => p.v)) : Math.min(...pairs.map((p) => p.v));
  const scale = Math.max(1, Math.abs(m));
  const eps = 1e-9 * scale;
  return new Set(pairs.filter((p) => Math.abs(p.v - m) <= eps).map((p) => p.i));
}

export const VARSocComponent: FC<{ data: VARSocResultData }> = ({ data }) => {
  const { var_names, maxlag, num_observation, rows } = data;

  const highlight = useMemo(
    () => ({
      ll: extremumRowIndices(
        rows.map((r) => r.log_likelihood),
        'max',
      ),
      lr: extremumRowIndices(rows.map((r) => r.lr), 'max'),
      lr_df: extremumRowIndices(rows.map((r) => r.lr_df), 'min'),
      lr_p: extremumRowIndices(rows.map((r) => r.lr_p), 'min'),
      fpe: extremumRowIndices(rows.map((r) => r.fpe), 'min'),
      aic: extremumRowIndices(rows.map((r) => r.aic), 'min'),
      hqic: extremumRowIndices(rows.map((r) => r.hqic), 'min'),
      sbic: extremumRowIndices(rows.map((r) => r.sbic), 'min'),
    }),
    [rows],
  );

  return (
    <ReportLayout
      title={data.title}
      size="wide"
      subtitle={
        <p className="text-xs text-muted-foreground">
          Endogenous: {var_names.join(', ')} · table lag 0…{maxlag} · n={num_observation} (common sample T−maxlag, Stata{' '}
          <code className="text-muted-foreground">varsoc</code>)
        </p>
      }
    >
      <ReportSection title="Lag-order selection" icon="margins">
        <InfoStatsTable className="overflow-x-auto" tableClassName="font-mono text-xs text-foreground">
          <TableHeader>
            <TableRow className="border-b border-border text-muted-foreground hover:bg-transparent">
              <TableHead className="h-auto px-3 py-2 text-left">Lag</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">LL</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">LR</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">df</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">P</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">FPE</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">AIC</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">HQIC</TableHead>
              <TableHead className="h-auto px-3 py-2 text-right">SBIC</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((r, idx) => (
              <TableRow
                key={r.lag}
                className={`border-b border-border transition-colors hover:bg-muted ${idx % 2 === 0 ? infoStatsRowEvenClass : infoStatsRowOddClass}`}
              >
                <TableCell className="px-3 py-2 text-muted-foreground">{r.lag}</TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.ll.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.log_likelihood)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.log_likelihood)}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.lr.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.lr ?? undefined)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.lr ?? undefined)}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {r.lr_df != null && highlight.lr_df.has(idx) ? (
                    <EmphasisNumber valueText={String(r.lr_df)} align="right" />
                  ) : (
                    <span className="text-foreground">{r.lr_df ?? '—'}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.lr_p.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.lr_p ?? undefined, 4)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.lr_p ?? undefined, 4)}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.fpe.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.fpe)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.fpe)}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.aic.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.aic)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.aic)}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.hqic.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.hqic)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.hqic)}</span>
                  )}
                </TableCell>
                <TableCell className="px-3 py-2 text-right">
                  {highlight.sbic.has(idx) ? (
                    <EmphasisNumber valueText={fmtCell(r.sbic)} align="right" />
                  ) : (
                    <span className="text-foreground">{fmtCell(r.sbic)}</span>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </InfoStatsTable>
      </ReportSection>

      <p className="mt-2 px-1 text-[10px] text-muted-foreground">
        <span className="inline-flex items-center gap-1 align-middle">
          <span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
        </span>
        加亮：FPE、AIC、HQIC、SBIC、P 取列内最小（同 Stata varsoc 信息准则 *）；LL、LR 取列内最大；df 取列内最小；Lag
        不标。并列则多行同标。
      </p>
    </ReportLayout>
  );
};
