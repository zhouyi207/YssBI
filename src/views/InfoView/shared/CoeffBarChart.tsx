import React from 'react';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { formatNum } from './utils';
import type { Coefficient } from './types';

export function CoeffBarChart({ coefficients }: { coefficients: Coefficient[] }) {
  const maxAbs = Math.max(...coefficients.map((c) => Math.abs(c.coef)), 0.001);

  return (
    <div className="rounded-lg border border-border bg-card p-4 space-y-2">
      {coefficients.map((coeff, idx) => {
        const pct = (Math.abs(coeff.coef) / maxAbs) * 100;
        const isPositive = coeff.coef >= 0;
        const label = coeff.category != null
          ? `${coeff.variable}[${coeff.category}]`
          : coeff.variable;

        return (
          <div key={`${coeff.variable}-${coeff.category ?? ''}-${idx}`} className="flex items-center gap-3">
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="text-xs font-mono text-muted-foreground w-28 text-right shrink-0 truncate cursor-default">
                  {label}
                </span>
              </TooltipTrigger>
              <TooltipContent side="top">{label}</TooltipContent>
            </Tooltip>
            <div className="flex-1 flex items-center h-5">
              <div className="w-1/2 flex justify-end">
                {!isPositive && (
                  <div
                    className={`h-4 rounded-l transition-all ${coeff.is_significant ? 'bg-rose-500/70' : 'bg-rose-500/25'}`}
                    style={{ width: `${pct}%`, minWidth: pct > 0 ? '2px' : '0' }}
                  />
                )}
              </div>
              <div className="w-px h-5 bg-border shrink-0" />
              <div className="w-1/2 flex justify-start">
                {isPositive && (
                  <div
                    className={`h-4 rounded-r transition-all ${coeff.is_significant ? 'bg-emerald-500/70' : 'bg-emerald-500/25'}`}
                    style={{ width: `${pct}%`, minWidth: pct > 0 ? '2px' : '0' }}
                  />
                )}
              </div>
            </div>
            <span className="text-[10px] font-mono text-muted-foreground w-20 text-left shrink-0">
              {formatNum(coeff.coef)}
            </span>
          </div>
        );
      })}
    </div>
  );
}
