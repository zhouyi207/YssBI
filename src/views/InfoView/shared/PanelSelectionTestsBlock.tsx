import React from 'react';
import { formatNum } from './RegressionShared';
import type { PanelSelectionTest } from './types';

function DecisionBadge({ decision }: { decision: string }) {
  if (decision === 'significant') {
    return <span className="text-[10px] px-2 py-0.5 rounded border border-amber-500/40 bg-amber-500/10 text-amber-300">Significant</span>;
  }
  if (decision === 'not_significant') {
    return <span className="text-[10px] px-2 py-0.5 rounded border border-gray-700 bg-gray-800/70 text-gray-300">Not Significant</span>;
  }
  return <span className="text-[10px] px-2 py-0.5 rounded border border-gray-700 bg-gray-900/60 text-gray-400">Unavailable</span>;
}

function TestCard({ t }: { t: PanelSelectionTest }) {
  return (
    <div className="rounded-lg border border-gray-800/50 bg-[#15171d] px-4 py-3">
      <div className="flex items-center justify-between gap-3 mb-1.5">
        <div className="text-xs text-white font-medium">{t.label}</div>
        <DecisionBadge decision={t.decision} />
      </div>
      <div className="text-[11px] text-gray-500 mb-2">{t.h0}</div>
      {t.stat != null && t.p_value != null ? (
        <div className="text-[11px] font-mono text-gray-300 mb-2">
          {t.stat_type}
          {t.df1 != null && t.df2 != null ? `(${t.df1},${t.df2})` : t.df1 != null ? `(${t.df1})` : ''} = {formatNum(t.stat)}
          {'  '}p = <span className={t.p_value < 0.05 ? 'text-emerald-400' : 'text-gray-400'}>{formatNum(t.p_value)}</span>
        </div>
      ) : (
        <div className="text-[11px] text-gray-500 mb-2">No valid statistic</div>
      )}
      <div className="text-[11px] text-gray-300">{t.recommendation}</div>
      {t.note ? <div className="text-[10px] text-gray-500 mt-1">{t.note}</div> : null}
    </div>
  );
}

export function PanelSelectionTestsBlock({ tests }: { tests: PanelSelectionTest[] }) {
  const modelTests = tests.filter((t) => t.group === 'model_choice');
  const effectTests = tests.filter((t) => t.group === 'effect_choice');

  if (tests.length === 0) return null;

  return (
    <div className="mb-6 rounded-xl border border-gray-800/60 bg-gray-900/35 p-4">
      <div className="text-[11px] text-gray-500 uppercase tracking-wider mb-3 font-medium">Model Selection Tests</div>
      {modelTests.length > 0 && (
        <>
          <div className="text-xs text-gray-400 mb-2">Model Type (Mixed / FE / RE)</div>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3 mb-4">
            {modelTests.map((t) => (
              <TestCard key={t.id} t={t} />
            ))}
          </div>
        </>
      )}
      {effectTests.length > 0 && (
        <>
          <div className="text-xs text-gray-400 mb-2">Effect Type (Entity / Time / Two-Way)</div>
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
            {effectTests.map((t) => (
              <TestCard key={t.id} t={t} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

