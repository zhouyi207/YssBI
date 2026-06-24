import React from 'react';
import { formatNum } from './RegressionShared';
import type { ModelBasicInfo } from './types';

export function AnovaTable({ info }: { info: ModelBasicInfo }) {
  return (
    <div className="rounded-lg border border-border overflow-hidden mb-2">
      <table className="w-full text-xs">
        <thead>
          <tr className="bg-muted">
            <th className="text-left px-4 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">Source</th>
            <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">SS</th>
            <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">df</th>
            <th className="text-right px-3 py-2.5 text-muted-foreground font-medium uppercase tracking-wider">MS</th>
          </tr>
        </thead>
        <tbody>
          <tr className="bg-card border-t border-border">
            <td className="px-4 py-2.5 font-mono text-foreground">Model</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground">{formatNum(info.ss_model)}</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground">{info.df_model}</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground">{formatNum(info.ms_model)}</td>
          </tr>
          <tr className="bg-muted/40 border-t border-border">
            <td className="px-4 py-2.5 font-mono text-foreground">Residual</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground">{formatNum(info.ss_residual)}</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground">{info.df_residual}</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground">{formatNum(info.ms_residual)}</td>
          </tr>
          <tr className="bg-card border-t border-border">
            <td className="px-4 py-2.5 font-mono text-foreground font-semibold">Total</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground font-semibold">{formatNum(info.ss_total)}</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground font-semibold">{info.df_total}</td>
            <td className="text-right px-3 py-2.5 font-mono text-foreground font-semibold">{formatNum(info.ms_total)}</td>
          </tr>
        </tbody>
      </table>
    </div>
  );
}
