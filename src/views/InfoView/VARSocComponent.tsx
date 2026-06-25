import React, { useMemo } from 'react';
import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { SectionHeader, formatNum } from './shared';
import {
  InfoStatsTable,
  infoStatsRowEvenClass,
  infoStatsRowOddClass,
} from './shared/InfoStatsTable';
import type { VARSocResultData } from './shared/types';

export type { VARSocResultData } from './shared/types';

function fmtCell(v: number | null | undefined, decimals = 4): string {
  if (v == null || Number.isNaN(v)) return '—';
  return formatNum(v, decimals);
}

/** 与 CoefficientTable 显著行一致：左侧 emerald 圆点 + 加亮文字 */
function EmphasisNumber({
  valueText,
  align = 'left',
}: {
  valueText: string;
  align?: 'left' | 'right';
}) {
  const flex = align === 'right' ? 'flex items-center justify-end gap-2' : 'flex items-center gap-2';
  return (
    <div className={flex}>
      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 shrink-0" aria-hidden />
      <span className="font-medium text-foreground">{valueText}</span>
    </div>
  );
}

function extremumRowIndices(
  nums: (number | null | undefined)[],
  mode: 'min' | 'max'
): Set<number> {
  const pairs: { v: number; i: number }[] = [];
  nums.forEach((v, i) => {
    if (v != null && !Number.isNaN(v)) pairs.push({ v, i });
  });
  if (pairs.length === 0) return new Set();
  const m =
    mode === 'max'
      ? Math.max(...pairs.map((p) => p.v))
      : Math.min(...pairs.map((p) => p.v));
  const scale = Math.max(1, Math.abs(m));
  const eps = 1e-9 * scale;
  return new Set(pairs.filter((p) => Math.abs(p.v - m) <= eps).map((p) => p.i));
}

export const VARSocComponent: React.FC<{ data: VARSocResultData }> = ({ data }) => {
  const { var_names, maxlag, num_observation, rows } = data;

  const highlight = useMemo(() => {
    // Stata varsoc：信息准则 * 在列内最小；LL/LR 越大越好；P 越小越显著；Lag 不标（最小恒为 0）
    return {
      ll: extremumRowIndices(
        rows.map((r) => r.log_likelihood),
        'max'
      ),
      lr: extremumRowIndices(rows.map((r) => r.lr), 'max'),
      lr_df: extremumRowIndices(rows.map((r) => r.lr_df), 'min'),
      lr_p: extremumRowIndices(rows.map((r) => r.lr_p), 'min'),
      fpe: extremumRowIndices(rows.map((r) => r.fpe), 'min'),
      aic: extremumRowIndices(rows.map((r) => r.aic), 'min'),
      hqic: extremumRowIndices(rows.map((r) => r.hqic), 'min'),
      sbic: extremumRowIndices(rows.map((r) => r.sbic), 'min'),
    };
  }, [rows]);

  return (
    <div className="p-6 max-w-[980px] mx-auto">
      <div className="mb-6">
        <h1 className="text-xl font-bold text-foreground mb-2">{data.title}</h1>
        <p className="text-xs text-muted-foreground">
          Endogenous: {var_names.join(', ')} · table lag 0…{maxlag} · n={num_observation} (common sample T−maxlag, Stata{' '}
          <code className="text-muted-foreground">varsoc</code>)
        </p>
      </div>

      <SectionHeader
        title="Lag-order selection"
        icon={
          <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
          </svg>
        }
      />

      <InfoStatsTable className="overflow-x-auto" tableClassName="text-xs font-mono text-foreground">
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

      <p className="mt-2 text-[10px] text-muted-foreground px-1">
        <span className="inline-flex items-center gap-1 align-middle">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
        </span>
        加亮：FPE、AIC、HQIC、SBIC、P 取列内最小（同 Stata varsoc 信息准则 *）；LL、LR 取列内最大；df 取列内最小；Lag
        不标。并列则多行同标。
      </p>
    </div>
  );
};
