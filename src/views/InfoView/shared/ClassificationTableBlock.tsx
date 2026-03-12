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
    <div className="rounded-xl border border-gray-700/40 bg-[#13151a] overflow-hidden shadow-sm">
      <div className="px-4 py-3 border-b border-gray-800/50">
        <h3 className="text-sm font-medium text-gray-300">Classification Table (estat classification)</h3>
        <p className="text-xs text-gray-500 mt-0.5">
          Classified + if predicted Pr(D) ≥ {data.cutoff}
        </p>
      </div>

      {/* 2×2 Table */}
      <div className="p-4">
        <table className="w-full text-sm border-collapse">
          <thead>
            <tr className="border-b border-gray-700/50">
              <th className="text-left py-2 px-3 text-gray-500 font-medium"></th>
              <th className="text-center py-2 px-3 text-gray-400 font-medium">True D</th>
              <th className="text-center py-2 px-3 text-gray-400 font-medium">True ~D</th>
              <th className="text-center py-2 px-3 text-gray-400 font-medium">Total</th>
            </tr>
          </thead>
          <tbody>
            <tr className="border-b border-gray-800/40">
              <td className="py-2 px-3 text-gray-400 font-medium">Classified +</td>
              <td className="py-2 px-3 text-center font-mono text-emerald-400">{data.tp}</td>
              <td className="py-2 px-3 text-center font-mono text-amber-400">{data.fp}</td>
              <td className="py-2 px-3 text-center font-mono text-gray-300">{totalPos}</td>
            </tr>
            <tr>
              <td className="py-2 px-3 text-gray-400 font-medium">Classified −</td>
              <td className="py-2 px-3 text-center font-mono text-amber-400">{data.fn_}</td>
              <td className="py-2 px-3 text-center font-mono text-emerald-400">{data.tn}</td>
              <td className="py-2 px-3 text-center font-mono text-gray-300">{totalNeg}</td>
            </tr>
            <tr className="border-t border-gray-700/50">
              <td className="py-2 px-3 text-gray-400 font-medium">Total</td>
              <td className="py-2 px-3 text-center font-mono text-gray-300">{totalD}</td>
              <td className="py-2 px-3 text-center font-mono text-gray-300">{totalND}</td>
              <td className="py-2 px-3 text-center font-mono text-gray-300">{total}</td>
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
            <div key={label} className="flex justify-between items-center py-1.5 border-b border-gray-800/30">
              <span className="text-gray-500">{label}</span>
              <span className="text-gray-400 font-mono text-[11px] mr-4">{formula}</span>
              <span className="text-white font-mono font-medium">{(value * 100).toFixed(2)}%</span>
            </div>
          ))}
          <div className="flex justify-between py-2 mt-2 bg-gray-800/20 rounded px-3">
            <span className="text-gray-400 font-medium">Correctly classified</span>
            <span className="text-[var(--accent-color)] font-mono font-semibold">{data.pct_correct.toFixed(2)}%</span>
          </div>
        </div>
      </div>
    </div>
  );
}
