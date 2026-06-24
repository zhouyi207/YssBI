import React from 'react';
import type { VifEntry } from './types';

function formatVifValue(v: number): string {
  if (!Number.isFinite(v)) return 'Inf';
  if (v >= 1e6) return v.toExponential(2);
  return v.toFixed(4);
}

function vifRowKey(row: VifEntry, idx: number): string {
  return row.category != null ? `${row.variable}-${row.category}` : `${row.variable}-${idx}`;
}

export function VifTable({ rows }: { rows: VifEntry[] }) {
  const hasCategory = rows.some((r) => r.category != null);

  return (
    <div className="rounded-lg border border-border bg-muted overflow-hidden">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b border-border">
            <th className="px-4 py-2.5 text-[11px] text-muted-foreground uppercase tracking-wider font-medium">Variable</th>
            {hasCategory && (
              <th className="px-4 py-2.5 text-[11px] text-muted-foreground uppercase tracking-wider font-medium">Category</th>
            )}
            <th className="px-4 py-2.5 text-[11px] text-muted-foreground uppercase tracking-wider font-medium">VIF</th>
            <th className="px-4 py-2.5 text-[11px] text-muted-foreground uppercase tracking-wider font-medium">1/VIF</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row, idx) => (
            <tr key={vifRowKey(row, idx)} className="border-b border-border last:border-b-0 hover:bg-muted/40">
              <td className="px-4 py-2.5 font-mono text-foreground">{row.variable}</td>
              {hasCategory && (
                <td className="px-4 py-2.5">
                  {row.category != null ? (
                    <span className="inline-flex items-center px-2 py-0.5 rounded text-[11px] font-mono bg-indigo-500/15 text-indigo-300 border border-indigo-500/25">
                      {row.category}
                    </span>
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </td>
              )}
              <td className="px-4 py-2.5 font-mono text-foreground">{formatVifValue(row.vif)}</td>
              <td className="px-4 py-2.5 font-mono text-foreground">{formatVifValue(row.tolerance)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function meanFiniteVif(rows: VifEntry[]): number | null {
  const finite = rows.filter((e) => Number.isFinite(e.vif));
  if (finite.length === 0) return null;
  return finite.reduce((s, e) => s + e.vif, 0) / finite.length;
}
