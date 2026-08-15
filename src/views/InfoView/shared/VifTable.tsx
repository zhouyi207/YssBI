import { TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { formatNum } from './RegressionShared';
import type { VifEntry } from '@/shared/types/report';
import {
  InfoStatsTable,
  infoStatsCellClass,
  infoStatsHeadClass,
} from './InfoStatsTable';

function vifRowKey(row: VifEntry, idx: number): string {
  return row.category != null ? `${row.variable}-${row.category}` : `${row.variable}-${idx}`;
}

export function VifTable({ rows }: { rows: VifEntry[] }) {
  const hasCategory = rows.some((r) => r.category != null);

  return (
    <InfoStatsTable className="bg-muted" tableClassName="text-sm">
      <TableHeader>
        <TableRow className="border-b border-border hover:bg-transparent">
          <TableHead className={infoStatsHeadClass}>Variable</TableHead>
          {hasCategory && <TableHead className={infoStatsHeadClass}>Category</TableHead>}
          <TableHead className={infoStatsHeadClass}>VIF</TableHead>
          <TableHead className={infoStatsHeadClass}>1/VIF</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((row, idx) => (
          <TableRow key={vifRowKey(row, idx)} className="border-b border-border last:border-b-0 hover:bg-muted/40">
            <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>{row.variable}</TableCell>
            {hasCategory && (
              <TableCell className={infoStatsCellClass}>
                {row.category != null ? (
                  <span className="inline-flex items-center rounded border border-indigo-500/25 bg-indigo-500/15 px-2 py-0.5 text-[11px] font-mono text-indigo-700 dark:text-indigo-300">
                    {row.category}
                  </span>
                ) : (
                  <span className="text-muted-foreground">—</span>
                )}
              </TableCell>
            )}
            <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>{formatNum(row.vif)}</TableCell>
            <TableCell className={`${infoStatsCellClass} font-mono text-foreground`}>{formatNum(row.tolerance)}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </InfoStatsTable>
  );
}

export function meanFiniteVif(rows: VifEntry[]): number | null {
  const finite = rows.filter((e) => Number.isFinite(e.vif));
  if (finite.length === 0) return null;
  return finite.reduce((s, e) => s + e.vif, 0) / finite.length;
}
