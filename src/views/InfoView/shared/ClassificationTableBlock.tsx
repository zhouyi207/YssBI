import React from 'react';
import type { ClassificationTable } from './types';

/** Stata estat classification — classification table and statistics */
export function ClassificationTableBlock({ data }: { data: ClassificationTable }) {
  const totalD = data.tp + data.fn_;
  const totalND = data.tn + data.fp;
  const totalPos = data.tp + data.fp;
  const totalNeg = data.tn + data.fn_;
  const total = data.tp + data.fp + data.tn + data.fn_;

  return (
    <div className="rounded-xl border border-border bg-card overflow-hidden shadow-sm">
      <div className="px-4 py-3 border-b border-border">
        <h3 className="text-sm font-medium text-foreground">Classification Table (estat classification)</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          Classified + if predicted Pr(D) ≥ {data.cutoff}
        </p>
      </div>

      {/* 2×2 Table */}
      <div className="p-4">
        <table className="w-full text-sm border-collapse">
          <thead>
            <tr className="border-b border-border">
              <th className="text-left py-2 px-3 text-muted-foreground font-medium"></th>
              <th className="text-center py-2 px-3 text-muted-foreground font-medium">True D</th>
              <th className="text-center py-2 px-3 text-muted-foreground font-medium">True ~D</th>
              <th className="text-center py-2 px-3 text-muted-foreground font-medium">Total</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b border-border">
              <td className="py-2 px-3 text-muted-foreground font-medium">Classified +</td>
              <td className="py-2 px-3 text-center font-mono text-emerald-400">{data.tp}</td>
              <td className="py-2 px-3 text-center font-mono text-amber-400">{data.fp}</td>
              <td className="py-2 px-3 text-center font-mono text-foreground">{totalPos}</td>
            </tr>
            <tr>
              <td className="py-2 px-3 text-muted-foreground font-medium">Classified −</td>
              <td className="py-2 px-3 text-center font-mono text-amber-400">{data.fn_}</td>
              <td className="py-2 px-3 text-center font-mono text-emerald-400">{data.tn}</td>
              <td className="py-2 px-3 text-center font-mono text-foreground">{totalNeg}</td>
            </tr>
            <tr className="border-t border-border">
              <td className="py-2 px-3 text-muted-foreground font-medium">Total</td>
              <td className="py-2 px-3 text-center font-mono text-foreground">{totalD}</td>
              <td className="py-2 px-3 text-center font-mono text-foreground">{totalND}</td>
              <td className="py-2 px-3 text-center font-mono text-foreground">{total}</td>
            </tr>
          </tbody>
        </table>

        {/* Statistics */}
        <div className="mt-4 space-y-1.5 text-xs">
          {[
            { label: 'Sensitivity', formula: 'Pr(+|D)', value: data.sensitivity },
            { label: 'Specificity', formula: 'Pr(−|~D)', value: data.specificity },
            { label: 'Positive predictive value', formula: 'Pr(D|+)', value: data.ppv },
            { label: 'Negative predictive value', formula: 'Pr(~D|−)', value: data.npv },
            { label: 'False + rate for true ~D', formula: 'Pr(+|~D)', value: data.false_pos_rate },
            { label: 'False − rate for true D', formula: 'Pr(−|D)', value: data.false_neg_rate },
          ].map(({ label, formula, value }) => (
            <div key={label} className="flex justify-between items-center py-1.5 border-b border-border">
              <span className="text-muted-foreground">{label}</span>
              <span className="text-muted-foreground font-mono text-[11px] mr-4">{formula}</span>
              <span className="text-foreground font-mono font-medium">{(value * 100).toFixed(2)}%</span>
            </div>
          ))}
          <div className="flex justify-between py-2 mt-2 bg-muted/40 rounded px-3">
            <span className="text-muted-foreground font-medium">Correctly classified</span>
            <span className="text-[var(--accent-color)] font-mono font-semibold">{data.pct_correct.toFixed(2)}%</span>
          </div>
        </div>
      </div>
    </div>
  );
}
